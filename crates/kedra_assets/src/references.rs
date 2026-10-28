//! Native reflected asset references, independent of property widgets and authoring.
use bevy::{
    asset::{AssetPath, LoadFromPath, ReflectHandle, UntypedHandle},
    reflect::{
        PartialReflect, ReflectFromReflect, ReflectRef, TypeInfo, TypeRegistry,
        enums::{DynamicEnum, DynamicVariant, VariantInfo},
        tuple::DynamicTuple,
    },
};
/// Borrowed native schema and current dependency for `Handle<T>`/`Option<Handle<T>>`.
pub struct NativeReference<'a> {
    /// Bevy's registered native handle constructor/type metadata.
    pub handle: &'a ReflectHandle,
    /// Represented Option type, absent for required references.
    pub optional: Option<&'static TypeInfo>,
    /// Current dependency; None represents an empty optional field.
    pub current: Option<UntypedHandle>,
}
/// Inspect only exact native handle shapes; unrelated types refuse without loading.
pub fn inspect<'a>(
    value: &dyn PartialReflect,
    types: &'a TypeRegistry,
) -> Result<NativeReference<'a>, String> {
    let info = value
        .get_represented_type_info()
        .ok_or("Native asset-reference type unavailable")?;
    if let Some(handle) = types.get_type_data::<ReflectHandle>(info.type_id()) {
        let value = value
            .try_as_reflect()
            .ok_or("Native asset handle required")?;
        let current = handle
            .downcast_handle_untyped(value.as_any())
            .ok_or("Native asset handle required")?;
        return Ok(NativeReference {
            handle,
            optional: None,
            current: Some(current),
        });
    }
    let TypeInfo::Enum(schema) = info else {
        return Err("Native asset handle required".into());
    };
    let Some(VariantInfo::Tuple(some)) = schema.variant("Some") else {
        return Err("Optional native asset handle required".into());
    };
    let handle = some
        .field_at(0)
        .and_then(|field| types.get_type_data::<ReflectHandle>(field.type_id()))
        .ok_or("Optional native asset handle required")?;
    let ReflectRef::Enum(current) = value.reflect_ref() else {
        return Err("Optional native asset handle required".into());
    };
    let current = if current.variant_name() == "None" {
        None
    } else {
        Some(
            current
                .field_at(0)
                .and_then(PartialReflect::try_as_reflect)
                .and_then(|value| handle.downcast_handle_untyped(value.as_any()))
                .ok_or("Native asset handle required")?,
        )
    };
    Ok(NativeReference {
        handle,
        optional: Some(info),
        current,
    })
}
/// Resolve the native asset type without inferring it from a filename.
pub fn asset_type(
    value: &dyn PartialReflect,
    types: &TypeRegistry,
) -> Result<std::any::TypeId, String> {
    Ok(inspect(value, types)?.handle.asset_type_id())
}
/// Return the current native dependency, not source/delivery authorization.
pub fn referenced_handle(
    value: &dyn PartialReflect,
    types: &TypeRegistry,
) -> Result<Option<UntypedHandle>, String> {
    Ok(inspect(value, types)?.current)
}
/// Construct an exactly typed native value through Bevy reflection and the supplied
/// loader. The host must authorize the path first. Required Clear and unsupported
/// reconstruction refuse; this never publishes authoring sources.
pub fn proposal(
    value: &dyn PartialReflect,
    path: Option<AssetPath<'static>>,
    types: &TypeRegistry,
    loader: &mut dyn LoadFromPath,
) -> Result<Box<dyn PartialReflect>, String> {
    let reference = inspect(value, types)?;
    let handle = path.map(|path| {
        reference
            .handle
            .typed(loader.load_from_path_erased(reference.handle.asset_type_id(), path))
    });
    if let Some(info) = reference.optional {
        let mut candidate = if let Some(handle) = handle {
            let mut tuple = DynamicTuple::default();
            tuple.insert_boxed(handle.into_partial_reflect());
            DynamicEnum::new("Some", DynamicVariant::Tuple(tuple))
        } else {
            DynamicEnum::new("None", DynamicVariant::Unit)
        };
        candidate.set_represented_type(Some(info));
        types
            .get_type_data::<ReflectFromReflect>(info.type_id())
            .and_then(|factory| factory.from_reflect(&candidate))
            .map(|value| value.into_partial_reflect())
            .ok_or("Optional asset-reference reconstruction failed".into())
    } else {
        handle
            .map(|handle| handle.into_partial_reflect())
            .ok_or("This resource field is required".into())
    }
}
