//! Caption ellipsis reuses the same Bevy-shaped cluster fitting as dock labels.
use super::{BoundHeader, under};
use crate::{native_input::owner, text_fit::Title};
use bevy::prelude::*;

#[derive(Component, Clone)]
struct FittedCaption {
    measure: Title,
    bounds: Entity,
    source: String,
}

/// Update and fit a caption within a registered header. Measurements retain the
/// complete Unicode source and share their parent's lifetime. Call after layout
/// updates as needed; unmeasured text keeps its full source until shaping is ready.
/// Reject foreign/stale text, non-UI bounds and changes of measurement parent.
pub fn set_title(
    world: &mut World,
    header: Entity,
    title: Entity,
    bounds: Entity,
    source: &str,
) -> Result<(), String> {
    let window = world
        .get::<BoundHeader>(header)
        .ok_or("Caption header is unbound")?
        .window;
    if owner(world, header) != Some(window)
        || !under(world, title, header)
        || !under(world, bounds, header)
        || world.get::<Text>(title).is_none()
        || world.get::<Node>(bounds).is_none()
    {
        return Err("Caption requires live text and bounds in its header".into());
    }
    let existing = world.get::<FittedCaption>(title).cloned();
    if existing.as_ref().is_some_and(|f| f.bounds != bounds) {
        return Err("Caption measurement parent cannot change while bound".into());
    }
    let changed = existing.as_ref().is_none_or(|f| f.source != source);
    let mut fitted = existing.unwrap_or_else(|| FittedCaption {
        measure: Title::new(world, bounds),
        bounds,
        source: String::new(),
    });
    if changed {
        fitted.source = source.into();
        if let Some(mut text) = world.get_mut::<Text>(title) {
            text.0 = source.into();
        }
    }
    fitted.measure.set(world, source);
    fitted.measure.fit(world, title, bounds);
    world.entity_mut(title).insert(fitted);
    Ok(())
}
