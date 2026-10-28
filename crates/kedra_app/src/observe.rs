//! Renderer-derived observation, shared by all Kedra applications.
use bevy::{
    camera::RenderTarget,
    input_focus::InputFocus,
    prelude::*,
    text::{EditableText, FontSource},
    ui::InteractionDisabled,
    window::PrimaryWindow,
};
use bevy_remote::BrpResult;
use kedra_ui::{ElementId, ScreenId, logical_rect};
use serde_json::{Value, json};

#[cfg(test)]
#[path = "observe_tests.rs"]
mod tests;

/// Observe the actual rendered tree, not an alternate test representation.
/// Reports duplicate visible IDs as errors and marks truncated output explicitly.
pub fn snapshot(world: &mut World) -> Result<Value, String> {
    snapshot_selected(world, None)
}
// Selection limits serialization work, never the inventory/uniqueness checks.
fn snapshot_selected(world: &mut World, selected: Option<&str>) -> Result<Value, String> {
    let frame = world
        .get_resource::<bevy::diagnostic::FrameCount>()
        .map_or(0, |frame| frame.0);
    let session = world.resource::<crate::AppSession>().id.clone();
    let focus = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    let ui_scale = world.get_resource::<UiScale>().map_or(1., |scale| scale.0);
    let primary = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next();
    let windows:Vec<_>=world.query::<(Entity,&Window)>().iter(world).map(|(id,window)|json!({"id":id.to_bits(),"title":window.title,"width":window.width(),"height":window.height(),"physical_width":window.physical_width(),"physical_height":window.physical_height(),"dpr":window.scale_factor(),"focused":window.focused,"primary":Some(id)==primary})).collect();
    let screens: Vec<_> = world
        .query::<&ScreenId>()
        .iter(world)
        .map(|screen| screen.0.clone())
        .collect();
    let mut nodes = Vec::new();
    let mut unique = std::collections::BTreeSet::new();
    let mut truncated = false;
    for (index, (entity, id, node, transform, visible, camera, clip)) in world
        .query::<(
            Entity,
            &ElementId,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &ComputedUiTargetCamera,
            Option<&CalculatedClip>,
        )>()
        .iter(world)
        .enumerate()
    {
        if index == 4096 {
            truncated = true;
            break;
        }
        let camera_entity = camera.get();
        let window = camera_entity
            .and_then(|entity| world.get::<RenderTarget>(entity))
            .and_then(|target| match target {
                RenderTarget::Window(bevy::window::WindowRef::Entity(entity)) => Some(*entity),
                RenderTarget::Window(bevy::window::WindowRef::Primary) => primary,
                _ => None,
            });
        let active = camera_entity
            .and_then(|entity| world.get::<Camera>(entity))
            .is_some_and(|camera| camera.is_active);
        let visible = active
            && visible.get()
            && !node.is_empty()
            && clip.is_none_or(|clip| !clip.is_fully_clipped());
        if visible && !unique.insert((window, id.0.as_str())) {
            return Err(format!("Duplicate visible UI ID: {}", id.0));
        }
        if selected.is_some_and(|selected| id.0 != selected) {
            continue;
        }
        // Bevy logical UI units divide out UiScale as well as target DPR.
        // Automation coordinates are logical window pixels, not UI units.
        let mut rect = logical_rect(node, transform).map(|coordinate| coordinate * ui_scale);
        let origin = camera_entity
            .and_then(|entity| world.get::<Camera>(entity))
            .and_then(Camera::logical_viewport_rect)
            .map_or(Vec2::ZERO, |rect| rect.min);
        rect[0] += origin.x;
        rect[2] += origin.x;
        rect[1] += origin.y;
        rect[3] += origin.y;
        let mut enabled = world.get::<InteractionDisabled>(entity).is_none();
        let mut parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
        while let Some(ancestor) = parent {
            enabled &= world.get::<InteractionDisabled>(ancestor).is_none();
            parent = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
        }
        let text = world.get::<Text>(entity).map(|text| text.0.clone());
        let checked = world
            .get::<bevy::ui::Checkable>(entity)
            .map(|_| world.get::<bevy::ui::Checked>(entity).is_some());
        let value = world
            .get::<EditableText>(entity)
            .map(|text| text.value().to_string());
        let settled = world
            .get::<kedra_ui::fields::TextField>(entity)
            .map(|_| !kedra_ui::fields::TextField::unsettled(world, entity));
        let font=world.get::<TextFont>(entity).map(|font| {
            let source=match &font.font {FontSource::Handle(handle)=>world.get_resource::<AssetServer>().and_then(|server|server.get_path(handle.id())).map(|path|path.to_string()),_=>Some(format!("{:?}",font.font))};
            json!({"source":source,"size":format!("{:?}",font.font_size),"weight":format!("{:?}",font.weight)})
        });
        nodes.push(json!({"id":id.0,"entity":entity.to_bits(),"parent":world.get::<ChildOf>(entity).map(|parent|parent.parent().to_bits()),"window":window.map(Entity::to_bits),"rect":rect,"visible":visible,"enabled":enabled,"center_unclipped":clip.is_none_or(|clip|clip.contains_point(transform.affine().translation)),"focused":focus==Some(entity),"text":text,"value":value,"settled":settled,"checked":checked,"font":font,"background":world.get::<BackgroundColor>(entity).map(|color|color.0.to_srgba().to_u8_array()),"color":world.get::<TextColor>(entity).map(|color|color.0.to_srgba().to_u8_array())}));
    }
    let input_busy = world
        .get_resource::<crate::input::InputQueue>()
        .is_some_and(|queue| queue.busy());
    let cameras:Vec<_>=world.query::<(Entity,&Camera,&RenderTarget)>().iter(world).map(|(entity,camera,target)|json!({"entity":entity.to_bits(),"active":camera.is_active,"target":format!("{target:?}"),"physical_size":camera.physical_viewport_size().map(|size|size.to_array()),"logical_size":camera.logical_viewport_size().map(|size|size.to_array())})).collect();
    let native=bevy::winit::WINIT_WINDOWS.with(|windows|windows.borrow().entity_to_winit.iter().filter_map(|(entity,_)| {
        let windows=windows.borrow();let window=windows.get_window(*entity)?;
        Some(json!({"window":entity.to_bits(),"focused":window.has_focus(),"visible":window.is_visible(),"minimized":window.is_minimized(),"position":window.outer_position().ok().map(|p|[p.x,p.y]),"size":[window.inner_size().width,window.inner_size().height],"monitor":window.current_monitor().and_then(|monitor|monitor.name())}))
    }).collect::<Vec<_>>());
    Ok(
        json!({"session":session,"frame":frame,"windows":windows,"native":native,"cameras":cameras,"screens":screens,"elements":nodes,"input_busy":input_busy,"truncated":truncated}),
    )
}
pub(crate) fn observe(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    #[derive(serde::Deserialize, Default)]
    #[serde(deny_unknown_fields)]
    struct Selection {
        element: Option<String>,
    }
    let selection = params
        .filter(|value| !value.is_null())
        .map(serde_json::from_value::<Selection>)
        .transpose()
        .map_err(|error| crate::invalid(error.to_string()))?
        .unwrap_or_default();
    snapshot_selected(world, selection.element.as_deref()).map_err(crate::invalid)
}

/// Resolve a stable element freshly, in logical coordinates of the target window.
/// Hidden, disabled, clipped, duplicated and stale elements cannot be clicked by ID.
/// Truncated observations are rejected because they cannot establish uniqueness.
pub fn element_center(world: &mut World, window: Entity, id: &str) -> Result<Vec2, String> {
    let snapshot = snapshot_selected(world, Some(id))?;
    if snapshot["truncated"] == true {
        return Err("UI observation was truncated; target uniqueness cannot be established".into());
    }
    let elements = snapshot["elements"]
        .as_array()
        .ok_or("UI observation unavailable")?;
    let matches: Vec<_> = elements
        .iter()
        .filter(|node| {
            node["id"] == id && node["window"] == window.to_bits() && node["visible"] == true
        })
        .collect();
    if matches.len() != 1 {
        return Err(format!(
            "Expected one visible element {id}, found {}",
            matches.len()
        ));
    }
    let node = matches[0];
    if node["enabled"] != true || node["center_unclipped"] != true {
        return Err("Element is disabled or clipped".into());
    }
    let rect: Vec<f32> = serde_json::from_value(node["rect"].clone()).map_err(|e| e.to_string())?;
    Ok(Vec2::new(
        (rect[0] + rect[2]) * 0.5,
        (rect[1] + rect[3]) * 0.5,
    ))
}
