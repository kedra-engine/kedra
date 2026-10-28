//! Exact typed local replacement and bounded list operations, using Bevy reflection.
use super::*;
use bevy::reflect::{
    ReflectFromReflect, ReflectMut, TypeInfo, TypeRegistry, std_traits::ReflectDefault,
};

pub(super) fn native(
    value: &dyn PartialReflect,
    types: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    let info = value
        .get_represented_type_info()
        .ok_or("Collection type unavailable")?;
    types
        .get_type_data::<ReflectFromReflect>(info.type_id())
        .and_then(|factory| factory.from_reflect(value))
        .map(|value| value.into_partial_reflect())
        .ok_or("Collection cannot be constructed natively".into())
}
pub(super) fn count(value: &dyn PartialReflect) -> Result<usize, String> {
    match value.reflect_ref() {
        bevy::reflect::ReflectRef::List(list) => Ok(list.len()),
        _ => Err("Native list required".into()),
    }
}
pub(super) fn new_item(
    state: &State,
    types: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    let Some(TypeInfo::List(info)) = state.value.as_ref().get_represented_type_info() else {
        return Err("Native list schema unavailable".into());
    };
    if info.item_ty().id()
        == std::any::TypeId::of::<kedra_ui::authored::components::UiImageDensity>()
    {
        use kedra_ui::authored::components::UiImageDensity;
        let bevy::reflect::ReflectRef::List(list) = state.value.as_ref().reflect_ref() else {
            return Err("Native density list required".into());
        };
        let prior = list.len().checked_sub(1).and_then(|index| list.get(index));
        let mut item = prior
            .and_then(UiImageDensity::from_reflect)
            .ok_or("Choose an admitted image before adding a density")?;
        item.density = if prior.is_some() {
            item.density * 2.
        } else {
            1.
        };
        if !item.density.is_finite() {
            return Err("New density exceeds the native finite range".into());
        }
        return Ok(Box::new(item));
    }
    if state.classes {
        let bevy::reflect::ReflectRef::List(list) = state.value.as_ref().reflect_ref() else {
            return Err("Native classes required".into());
        };
        for index in 1..=129 {
            let name = if index == 1 {
                "class".to_owned()
            } else {
                format!("class-{index}")
            };
            if !list
                .iter()
                .any(|item| item.try_downcast_ref::<String>() == Some(&name))
            {
                return Ok(Box::new(name));
            }
        }
        return Err("No unique class default available".into());
    }
    types
        .get_type_data::<ReflectDefault>(info.item_ty().id())
        .map(|factory| factory.default().into_partial_reflect())
        .ok_or("No registered native item default; existing entries remain editable".into())
}
pub(super) fn structural(
    state: &State,
    operation: Operation,
    types: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>, String> {
    let item = if matches!(operation, Operation::Add) {
        Some(new_item(state, types)?)
    } else {
        None
    };
    let mut candidate = state
        .value
        .as_ref()
        .to_dynamic()
        .map_err(|e| e.to_string())?;
    let ReflectMut::List(list) = candidate.as_mut().reflect_mut() else {
        return Err("Native list required".into());
    };
    match operation {
        Operation::Add => {
            if list.len() >= 128 {
                return Err("Collection draft exceeds 128 entries".into());
            }
            list.push(item.ok_or("Native item unavailable")?);
        }
        Operation::Remove(index) => {
            if index >= list.len() {
                return Err("Collection entry is stale".into());
            }
            list.remove(index);
        }
        Operation::Move(from, to) => {
            if from >= list.len() || to >= list.len() {
                return Err("Collection reorder is stale".into());
            }
            let item = list.remove(from);
            list.insert(to, item);
        }
        _ => return Err("Structural collection action required".into()),
    }
    // Native nested editors require actual Val/Color values, not DynamicEnum
    // fields introduced by structural list reflection. Reconstruct the complete
    // native list exactly; try_apply would not implement removal/reordering.
    native(candidate.as_ref(), types)
}
