//! A component/schema allowlist layered on Bevy's registry and reference processors.
use crate::{
    Result,
    document::{ComponentData, InstanceId, NodeAddress, Reference},
    failure,
};
use bevy::{
    asset::{
        EphemeralHandleBehavior, HandleDeserializeProcessor, HandleSerializeProcessor, LoadFromPath,
    },
    ecs::reflect::ReflectComponent,
    prelude::*,
    reflect::{
        PartialReflect, TypeRegistration, TypeRegistry,
        serde::{
            ReflectDeserializerProcessor, ReflectSerializerProcessor, TypedReflectDeserializer,
            TypedReflectSerializer,
        },
    },
};
use serde::{Deserialize, Serialize, de::DeserializeSeed};
use std::{any::TypeId, cell::RefCell, collections::BTreeMap};

#[derive(Clone)]
/// An allowed Bevy component's source schema and side-effect-free validator.
pub struct ComponentPolicy {
    /// Canonical authored component key.
    pub key: String,
    /// Exact accepted schema version; no compatibility readers are installed.
    pub version: u32,
    /// Component registration in Bevy's TypeRegistry, not a second reflection identity.
    pub type_id: TypeId,
    /// Source validation before world insertion/publication.
    pub validate: fn(&dyn PartialReflect) -> Result<()>,
    verify_mapping: fn(&dyn PartialReflect, &BTreeMap<Entity, usize>) -> Result<()>,
}

#[derive(Resource, Default, Clone)]
/// Explicit schema allowlist; all construction, reflection and mapping remain Bevy-owned.
pub struct ComponentPolicies(pub(crate) BTreeMap<String, ComponentPolicy>);
impl ComponentPolicies {
    /// Encode a reflected source value with symbolic references, rejecting ephemeral handles.
    pub fn encode_value(
        &self,
        value: &dyn PartialReflect,
        types: &TypeRegistry,
        entities: &BTreeMap<Entity, Reference>,
    ) -> Result<Box<ron::value::RawValue>> {
        let processor = EncodeReferences {
            entities,
            visited: RefCell::default(),
        };
        let serializer = TypedReflectSerializer::with_processor(value, types, &processor);
        crate::value::parse(&ron::to_string(&serializer)?)
    }
    /// Register one hook-free component and its required component policies.
    /// Duplicate keys/types and side-effectful lifecycle requirements fail.
    pub fn register<T: Component + Reflect + FromReflect + bevy::reflect::GetTypeRegistration>(
        &mut self,
        types: &mut TypeRegistry,
        key: &str,
        version: u32,
        validate: fn(&dyn PartialReflect) -> Result<()>,
    ) -> Result<()> {
        if T::on_add().is_some()
            || T::on_insert().is_some()
            || T::on_discard().is_some()
            || T::on_remove().is_some()
            || T::on_despawn().is_some()
        {
            return Err(failure(
                "authorable components must not have insertion/lifecycle hooks; activate gameplay after PrefabReady",
            ));
        }
        let mut probe = World::new();
        let component = probe.register_component::<T>();
        if let Some(info) = probe.components().get_info(component) {
            for required in info.required_components().iter_ids() {
                let Some(info) = probe.components().get_info(required) else {
                    continue;
                };
                let engine_presentation = [
                    TypeId::of::<Transform>(),
                    TypeId::of::<GlobalTransform>(),
                    TypeId::of::<bevy::transform::components::TransformTreeChanged>(),
                    TypeId::of::<Visibility>(),
                    TypeId::of::<InheritedVisibility>(),
                    TypeId::of::<ViewVisibility>(),
                ];
                if !info
                    .type_id()
                    .is_some_and(|id| engine_presentation.contains(&id))
                    && !self
                        .0
                        .values()
                        .any(|policy| Some(policy.type_id) == info.type_id())
                {
                    return Err(failure(format!(
                        "required component {} needs its own side-effect-free authoring registration",
                        info.name()
                    )));
                }
            }
        }
        if self.0.contains_key(key) || self.0.values().any(|p| p.type_id == TypeId::of::<T>()) {
            return Err(failure("duplicate authored component key/type"));
        }
        types.register::<T>();
        types.register::<Name>();
        types.register::<ChildOf>();
        types.register::<Children>();
        let registration = types
            .get(TypeId::of::<T>())
            .ok_or_else(|| failure("component registration failed"))?;
        if registration.data::<ReflectComponent>().is_none() {
            return Err(failure("authorable type must reflect Component"));
        }
        self.0.insert(
            key.into(),
            ComponentPolicy {
                key: key.into(),
                version,
                type_id: TypeId::of::<T>(),
                validate,
                verify_mapping: verify_mapping::<T>,
            },
        );
        Ok(())
    }
    /// Require an exact registered key/version. Unknown data is never silently skipped.
    pub fn policy(&self, key: &str, version: u32) -> Result<&ComponentPolicy> {
        let policy = self.0.get(key).ok_or_else(|| {
            failure(format!(
                "unknown authored component {key}; read-only until migration"
            ))
        })?;
        if policy.version != version {
            return Err(failure(format!(
                "{key}: schema version {version} requires migration"
            )));
        }
        Ok(policy)
    }
    /// Validate and encode one component, verifying that Bevy maps every persisted entity field.
    pub fn encode(
        &self,
        key: &str,
        value: &dyn PartialReflect,
        types: &TypeRegistry,
        entities: &BTreeMap<Entity, Reference>,
    ) -> Result<ComponentData> {
        let policy = self
            .0
            .get(key)
            .ok_or_else(|| failure("unregistered authored component"))?;
        if value
            .get_represented_type_info()
            .is_none_or(|info| info.type_id() != policy.type_id)
        {
            return Err(failure("component type/key mismatch"));
        }
        (policy.validate)(value)?;
        let processor = EncodeReferences {
            entities,
            visited: RefCell::default(),
        };
        let serializer = TypedReflectSerializer::with_processor(value, types, &processor);
        let text = ron::to_string(&serializer)?;
        (policy.verify_mapping)(value, &processor.visited.into_inner())?;
        ComponentData::ron(policy.version, &text)
    }
    /// Decode an allowed typed value with explicit reference/asset context; no live writes occur.
    pub fn decode(
        &self,
        key: &str,
        data: &ComponentData,
        types: &TypeRegistry,
        context: &mut ReferenceContext<'_>,
    ) -> Result<Box<dyn PartialReflect>> {
        let policy = self.policy(key, data.version)?;
        let registration = types
            .get(policy.type_id)
            .ok_or_else(|| failure("type missing from registry"))?;
        let mut de = ron::Deserializer::from_str(data.value.get_ron())?;
        let value = TypedReflectDeserializer::with_processor(registration, types, context)
            .deserialize(&mut de)?;
        de.end()?;
        (policy.validate)(value.as_ref())?;
        Ok(value)
    }
}

struct EncodeReferences<'a> {
    entities: &'a BTreeMap<Entity, Reference>,
    visited: RefCell<BTreeMap<Entity, usize>>,
}
impl ReflectSerializerProcessor for EncodeReferences<'_> {
    fn try_serialize<S: serde::Serializer>(
        &self,
        value: &dyn PartialReflect,
        registry: &TypeRegistry,
        serializer: S,
    ) -> std::result::Result<std::result::Result<S::Ok, S>, S::Error> {
        if let Some(entity) = value.try_downcast_ref::<Entity>() {
            *self.visited.borrow_mut().entry(*entity).or_default() += 1;
            let reference = self.entities.get(entity).ok_or_else(|| {
                serde::ser::Error::custom("reference points outside authored graph/ports")
            })?;
            return Ok(Ok(reference.serialize(serializer)?));
        }
        HandleSerializeProcessor {
            ephemeral_handle_behavior: EphemeralHandleBehavior::Error,
        }
        .try_serialize(value, registry, serializer)
    }
}

fn verify_mapping<T: Component + FromReflect>(
    value: &dyn PartialReflect,
    expected: &BTreeMap<Entity, usize>,
) -> Result<()> {
    #[derive(Default)]
    struct Probe(BTreeMap<Entity, usize>);
    impl bevy::ecs::entity::EntityMapper for Probe {
        fn get_mapped(&mut self, source: Entity) -> Entity {
            *self.0.entry(source).or_default() += 1;
            source
        }
        fn set_mapped(&mut self, _: Entity, _: Entity) {}
    }
    let mut value =
        T::from_reflect(value).ok_or_else(|| failure("cannot construct authored component"))?;
    let mut probe = Probe::default();
    T::map_entities(&mut value, &mut probe);
    if probe.0 != *expected {
        return Err(failure(
            "Entity mapping does not match persisted references; annotate fields with #[entities]",
        ));
    }
    Ok(())
}

/// Compilation-scoped symbolic references and Bevy asset loader.
pub struct ReferenceContext<'a> {
    /// Known source addresses in this isolated compilation.
    pub entities: &'a BTreeMap<NodeAddress, Entity>,
    /// Explicitly present/absent root ports.
    pub ports: &'a BTreeMap<String, Option<Entity>>,
    /// Scope prepended to local addresses.
    pub prefix: &'a [InstanceId],
    /// Native asset-reference loader; no independent filesystem reader is used.
    pub loader: &'a mut dyn LoadFromPath,
}
impl ReferenceContext<'_> {
    fn resolve(&self, reference: Reference) -> Result<Option<Entity>> {
        match reference {
            Reference::Local(address) => self
                .entities
                .get(&address.scoped(self.prefix))
                .copied()
                .map(Some)
                .ok_or_else(|| failure("dangling prefab node reference")),
            Reference::Port(name) => self
                .ports
                .get(&name)
                .copied()
                .ok_or_else(|| failure(format!("unbound port {name}"))),
        }
    }
}
impl ReflectDeserializerProcessor for ReferenceContext<'_> {
    fn try_deserialize<'de, D: serde::Deserializer<'de>>(
        &mut self,
        registration: &TypeRegistration,
        registry: &TypeRegistry,
        de: D,
    ) -> std::result::Result<std::result::Result<Box<dyn PartialReflect>, D>, D::Error> {
        if registration.type_id() == TypeId::of::<Entity>() {
            let reference = Reference::deserialize(de)?;
            let entity = self
                .resolve(reference)
                .map_err(serde::de::Error::custom)?
                .ok_or_else(|| serde::de::Error::custom("required entity port is None"))?;
            return Ok(Ok(Box::new(entity)));
        }
        if registration.type_id() == TypeId::of::<Option<Entity>>() {
            let reference = Option::<Reference>::deserialize(de)?;
            let entity = reference
                .map(|r| self.resolve(r))
                .transpose()
                .map_err(serde::de::Error::custom)?
                .flatten();
            return Ok(Ok(Box::new(entity)));
        }
        HandleDeserializeProcessor {
            load_from_path: self.loader,
        }
        .try_deserialize(registration, registry, de)
    }
}
