//! Property sessions retain native resource ownership/path through indexed exact
//! replacement; final rejection preserves sources and the reusable candidate.
use crate::support::{Result, setup};
use bevy::{
    asset::{AssetApp, AssetPath, LoadFromPath, UntypedHandle},
    prelude::*,
    reflect::ParsedPath,
};
use kedra_authoring::{
    properties::{PropertyEdit, PropertySession, PropertyTarget},
    workspace::WorkingSet,
};
use kedra_scene::{
    assets::compile_catalog, composition::Catalog, document::*, instance::CompiledPrefab,
};
use std::{any::TypeId, collections::BTreeMap, sync::Arc};
#[derive(Asset, Reflect)]
struct Resource;
#[derive(Component, Reflect)]
#[reflect(Component)]
struct References {
    images: Vec<Handle<Resource>>,
}
struct Loader {
    ids: BTreeMap<String, uuid::Uuid>,
}
impl LoadFromPath for Loader {
    fn load_from_path_erased(
        &mut self,
        type_id: TypeId,
        path: AssetPath<'static>,
    ) -> UntypedHandle {
        UntypedHandle::Uuid {
            type_id,
            uuid: self.ids.get(&path.to_string()).copied().unwrap_or_default(),
        }
    }
}
#[test]
fn indexed_native_handle_replacement_retains_its_new_logical_path_across_rejection_and_undo()
-> Result<()> {
    let (mut app, mut policies) = setup()?;
    app.init_asset::<Resource>()
        .register_asset_reflect::<Resource>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    policies.register::<References>(&mut registry.write(), "references", 1, |_| Ok(()))?;
    let types = registry.read();
    let mut document = PrefabDocument::new("Resource owner");
    document.nodes[0].components.insert(
        "references".into(),
        ComponentData::ron(1, "(images:[Path(\"game://first.resource\")])")?,
    );
    let mut working = WorkingSet::default();
    working.open(document.clone())?;
    let mut loader = Loader {
        ids: BTreeMap::from([
            ("game://first.resource".into(), uuid::Uuid::new_v4()),
            ("game://second.resource#exact".into(), uuid::Uuid::new_v4()),
        ]),
    };
    let compiled = Arc::new(CompiledPrefab::compile(
        working.catalog().resolve(document.id)?,
        &types,
        &policies,
        &mut loader,
        &BTreeMap::new(),
    )?);
    let mut session = PropertySession::begin(
        &working,
        compiled,
        PropertyTarget {
            document: document.id,
            node: document.root.into(),
            component: "references".into(),
        },
        &types,
        &policies,
    )?;
    let replacement = app
        .world()
        .resource::<AssetServer>()
        .load::<Resource>("game://second.resource#exact");
    let ids = loader.ids.clone();
    let validate = |catalog: &Catalog| {
        compile_catalog(catalog, &types, &policies, &mut Loader { ids: ids.clone() }).map(|_| ())
    };
    session.propose_batch(
        &working,
        &[PropertyEdit {
            path: ParsedPath::parse("images[0]")?,
            value: &replacement,
        }],
        &types,
        &policies,
        validate,
    )?;
    drop(replacement);
    let before = ron::to_string(&working)?;
    assert!(
        session
            .accept(&mut working, |_| Err(kedra_scene::Error(
                "Host preview refused".into()
            )))
            .is_err()
    );
    assert_eq!(ron::to_string(&working)?, before);
    assert!(!working.can_undo());
    session.accept(&mut working, validate)?;
    let compiled = compile_catalog(&working.catalog(), &types, &policies, &mut loader)?;
    let current = &compiled[&document.id];
    let value = current
        .authoring_world()
        .get::<References>(current.source_entities()[&document.root.into()])
        .ok_or("references")?;
    assert_eq!(
        current
            .dependency_path(&value.images[0].clone().untyped())
            .map(ToString::to_string),
        Some("game://second.resource#exact".into())
    );
    working.undo_checked(validate)?;
    assert_eq!(working.documents()[&document.id].document, document);
    assert!(!working.can_undo());
    Ok(())
}
