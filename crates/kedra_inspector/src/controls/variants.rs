//! Native enum construction from the existing Bevy registry, never parsed UI labels.
use bevy::{
    prelude::*,
    reflect::{
        TypeInfo, TypeRegistry,
        enums::{DynamicEnum, DynamicVariant, VariantInfo},
        std_traits::ReflectDefault,
        structs::DynamicStruct,
        tuple::DynamicTuple,
    },
};
use std::sync::Arc;

/// Build complete variants with registered native field defaults. Current payload
/// values are retained exactly. A missing constructor refuses the whole menu;
/// no guessed source schema or partially initialized payload is exposed.
pub fn enum_choices(
    value: &dyn PartialReflect,
    types: &TypeRegistry,
) -> Option<Vec<(String, Arc<dyn PartialReflect>)>> {
    let TypeInfo::Enum(schema) = value.get_represented_type_info()? else {
        return None;
    };
    let bevy::reflect::ReflectRef::Enum(current) = value.reflect_ref() else {
        return None;
    };
    if schema.variant_len() > 128 {
        return None;
    }
    schema
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let name = variant.name();
            if current.variant_name() == name {
                return Some((name.to_owned(), Arc::from(value.to_dynamic().ok()?)));
            }
            let payload = match variant {
                VariantInfo::Unit(_) => DynamicVariant::Unit,
                VariantInfo::Tuple(fields) => {
                    let mut data = DynamicTuple::default();
                    for field in fields.iter() {
                        if types
                            .get_type_data::<bevy::asset::ReflectHandle>(field.type_id())
                            .is_some()
                        {
                            return None;
                        }
                        data.insert_boxed(
                            types
                                .get_type_data::<ReflectDefault>(field.type_id())?
                                .default()
                                .into_partial_reflect(),
                        );
                    }
                    DynamicVariant::Tuple(data)
                }
                VariantInfo::Struct(fields) => {
                    let mut data = DynamicStruct::default();
                    for field in fields.iter() {
                        if types
                            .get_type_data::<bevy::asset::ReflectHandle>(field.type_id())
                            .is_some()
                        {
                            return None;
                        }
                        data.insert_boxed(
                            field.name(),
                            types
                                .get_type_data::<ReflectDefault>(field.type_id())?
                                .default()
                                .into_partial_reflect(),
                        );
                    }
                    DynamicVariant::Struct(data)
                }
            };
            let mut native = DynamicEnum::new_with_index(index, name, payload);
            native.set_represented_type(value.get_represented_type_info());
            Some((name.to_owned(), Arc::new(native) as Arc<dyn PartialReflect>))
        })
        .collect()
}
