//! Debug catalog bindings on authored canvases in the ordinary editor application.
use bevy::reflect::PartialReflect;
use bevy::{prelude::*, text::EditableText};
use bevy_remote::{BrpError, BrpResult};
use kedra_app::RemoteAppExt;
use kedra_scene::{
    assets::PrefabSources,
    document::DocumentId,
    startup::{StartupScene, StartupState},
};
use kedra_ui::authored::view::UiView;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Authored specimen values/calibration, separate from unchanged production prefabs.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub(crate) struct Canvas {
    empty_caption: String,
    field_text: String,
    cursor_blink_seconds: f32,
    round_layout: bool,
    hinting: bevy::text::FontHinting,
    density: f32,
}

pub(crate) fn validate_canvas(value: &dyn PartialReflect) -> kedra_scene::Result<()> {
    let canvas = Canvas::from_reflect(value)
        .ok_or_else(|| kedra_scene::Error("Invalid catalog calibration".into()))?;
    if canvas.empty_caption.chars().count() > 4096
        || canvas.field_text.chars().count() > 4096
        || !canvas.cursor_blink_seconds.is_finite()
        || !(0.1..=86_400.).contains(&canvas.cursor_blink_seconds)
        || !canvas.density.is_finite()
        || !(0.5..=4.).contains(&canvas.density)
    {
        return Err(kedra_scene::Error(
            "Catalog text must be bounded, caret period 0.1..=86400 seconds and density 0.5..=4"
                .into(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    source: String,
    disabled: bool,
}

#[derive(Resource, Default)]
struct Catalog {
    canvas: Option<Entity>,
    slots: BTreeMap<&'static str, Entity>,
    view: Option<Entity>,
    selected: Option<String>,
    document: Option<DocumentId>,
}

fn specimen_slot(source: &str) -> Result<&'static str, String> {
    match source {
        "editor://prefabs/button.prefab.ron"
        | "editor://prefabs/raised-button.prefab.ron"
        | "editor://prefabs/primary-button.prefab.ron" => Ok("button-slot"),
        "editor://prefabs/field.prefab.ron" => Ok("field-slot"),
        "editor://prefabs/browser-icon.prefab.ron" => Ok("icon-slot"),
        _ => Err("Only the catalog's button and text-field sources are supported".into()),
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Catalog>();
    kedra_app::scripting::register(
        app,
        kedra_script::Program::sequence(
            "catalog.prepare",
            "crates/kedra_editor/src/catalog/prepare.luau",
            include_str!("catalog/prepare.luau"),
            kedra_app::scripting::install_app_api,
        ),
    );
    app.register_remote(
        "catalog.state",
        "Read fixture readiness and renderer",
        json!({}),
        describe,
    );
    app.register_remote(
        "catalog.show",
        "Mount a production button or field specimen; not product input",
        json!({"source":"editor://prefabs/<catalog member>.prefab.ron", "disabled":"boolean"}),
        show,
    );
    app.register_remote(
        "catalog.text_capture",
        "Scoped full/shape paint with retained Text and EditableText metrics",
        json!({"layer":"full | shape"}),
        text_capture,
    );
}

fn text_capture(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        layer: String,
    }
    let request: Request = kedra_app::params(value)?;
    let catalog = world.resource::<Catalog>();
    let root = catalog
        .view
        .ok_or_else(|| kedra_app::invalid("No mounted UI text scope"))?;
    let document = catalog.document;
    let nodes = world
        .get::<kedra_scene::instance::PrefabInstance>(root)
        .ok_or_else(|| kedra_app::invalid("Mounted UI scope was retired"))?
        .nodes
        .iter()
        .map(|(address, entity)| (address.clone(), *entity))
        .collect::<Vec<_>>();
    let mut snapshot = kedra_ui::text_capture::capture(world, root, &nodes, &request.layer)
        .map_err(kedra_app::invalid)?;
    if snapshot["ready"] == true {
        snapshot["scope"] = json!(document);
    }
    Ok(snapshot)
}

fn describe(_: In<Option<Value>>, world: &mut World) -> BrpResult {
    let sources = world.resource::<PrefabSources>();
    let state = world.resource::<Catalog>();
    let renderer = world.get_resource::<bevy::render::renderer::RenderAdapterInfo>()
        .map(|adapter| json!({"name":adapter.name,"backend":format!("{:?}",adapter.backend),"driver":adapter.driver,"driver_info":adapter.driver_info}));
    Ok(
        json!({"ready":sources.ready() && state.canvas.is_some_and(|root| world.entities().contains(root)) && world.get_resource::<StartupScene>().is_some_and(|startup| matches!(startup.state(), StartupState::Mounted { .. })),"error":sources.error,"selected":state.selected,"document":state.document,
        "mounted":state.view.is_some(),"renderer":renderer,
        "rendered_state":state.view.and_then(|root|rendered_state(world,root))}),
    )
}

fn show(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    let selected: Selection = serde_json::from_value(value.unwrap_or(Value::Null))
        .map_err(|error| kedra_app::invalid(error.to_string()))?;
    mount(world, selected).map_err(BrpError::internal)?;
    Ok(json!({"mounted":true}))
}

pub(crate) fn bind(world: &mut World, view: &UiView) -> kedra_scene::Result<()> {
    if world
        .resource::<Catalog>()
        .canvas
        .is_some_and(|root| world.entities().contains(root))
    {
        return Err(kedra_scene::Error("Catalog canvas is already bound".into()));
    }
    let calibration = world
        .get::<Canvas>(view.root)
        .ok_or_else(|| kedra_scene::Error("Catalog calibration missing".into()))?
        .clone();
    validate_canvas(&calibration)?;
    world
        .resource_mut::<kedra_ui::authored::components::UiMotion>()
        .reduced = true;
    let slots = ["button-slot", "field-slot", "icon-slot"]
        .into_iter()
        .map(|key| view.element(key).map(|entity| (key, entity)))
        .collect::<kedra_scene::Result<BTreeMap<_, _>>>();
    let slots = match slots {
        Ok(slots) => slots,
        Err(error) => {
            world.despawn(view.root);
            return Err(error);
        }
    };
    world.entity_mut(view.root).insert((
        LayoutConfig {
            use_rounding: calibration.round_layout,
        },
        bevy::input_focus::tab_navigation::TabGroup::new(0),
    ));
    world.resource_mut::<Catalog>().slots = slots;
    world.resource_mut::<Catalog>().canvas = Some(view.root);
    Ok(())
}

pub(crate) fn retire_binding(world: &mut World, root: Entity) {
    if world.resource::<Catalog>().canvas == Some(root) {
        *world.resource_mut::<Catalog>() = Catalog::default();
    }
}

#[cfg(test)]
pub(crate) fn has_binding(world: &World) -> bool {
    let state = world.resource::<Catalog>();
    state.canvas.is_some() || state.view.is_some() || !state.slots.is_empty()
}

fn mount(world: &mut World, selected: Selection) -> Result<(), String> {
    let slot = specimen_slot(&selected.source)?;
    if !world
        .resource::<Catalog>()
        .canvas
        .is_some_and(|root| world.entities().contains(root))
    {
        return Err("No catalog canvas is bound".into());
    }
    let canvas = world
        .resource::<Catalog>()
        .canvas
        .ok_or("Catalog canvas missing")?;
    let calibration = world
        .get::<Canvas>(canvas)
        .ok_or("Catalog calibration missing")?
        .clone();
    validate_canvas(&calibration).map_err(|error| error.to_string())?;
    if !world
        .get_resource::<StartupScene>()
        .is_some_and(|startup| matches!(startup.state(), StartupState::Mounted { .. }))
    {
        return Err("Component scene is not mounted".into());
    }
    let sources = world.resource::<PrefabSources>();
    if !sources.ready() {
        return Err("Production sources are not ready".into());
    }
    let template = crate::views::compiled(world, &selected.source)?;
    let document = template.graph().document;
    if let Some(root) = world.resource_mut::<Catalog>().view.take() {
        world.despawn(root);
    }
    world
        .resource_mut::<bevy::input_focus::InputFocus>()
        .clear();
    let slots = world.resource::<Catalog>().slots.clone();
    for (key, entity) in &slots {
        world
            .get_mut::<Node>(*entity)
            .ok_or("Retired specimen slot")?
            .display = if *key == slot {
            Display::Flex
        } else {
            Display::None
        };
        world
            .entity_mut(*entity)
            .insert(kedra_ui::ElementId::new(*key));
    }
    let parent = *slots.get(slot).ok_or("Missing authored specimen slot")?;
    world
        .entity_mut(parent)
        .insert(kedra_ui::ElementId::new("specimen.viewport"));
    let view = UiView::mount(
        world,
        &template,
        Some(parent),
        &BTreeMap::new(),
        "specimen",
        calibration.density,
    )
    .map_err(|error| error.to_string())?
    .root;
    world
        .entity_mut(view)
        .insert(kedra_ui::ElementId::new("specimen.target"));
    let nodes = world
        .get::<kedra_scene::instance::PrefabInstance>(view)
        .ok_or("Missing source instance")?
        .nodes
        .values()
        .copied()
        .collect::<Vec<_>>();
    for entity in nodes {
        if let Some(mut text) = world.get_mut::<Text>(entity) {
            if text.0.is_empty() {
                text.0 = calibration.empty_caption.clone();
            }
            world.entity_mut(entity).insert((
                calibration.hinting,
                kedra_ui::ElementId::new("specimen.caption"),
            ));
        }
        if let Some(mut text) = world.get_mut::<EditableText>(entity) {
            text.editor_mut().set_text(&calibration.field_text);
            text.cursor_blink_period =
                std::time::Duration::from_secs_f32(calibration.cursor_blink_seconds);
            world
                .entity_mut(entity)
                .insert(kedra_ui::ElementId::new("specimen.input"));
        }
        if selected.disabled && world.get::<bevy::ui_widgets::Button>(entity).is_some() {
            world
                .entity_mut(entity)
                .insert(bevy::ui::InteractionDisabled);
        }
    }
    let mut state = world.resource_mut::<Catalog>();
    state.view = Some(view);
    state.selected = Some(selected.source);
    state.document = Some(document);
    Ok(())
}

fn rendered_state(world: &World, root: Entity) -> Option<Value> {
    use bevy_flair::style::components::StyleData;
    let root_state = world.get::<StyleData>(root)?.get_pseudo_state();
    let input = world
        .get::<kedra_scene::instance::PrefabInstance>(root)
        .and_then(|instance| {
            instance
                .nodes
                .values()
                .copied()
                .find(|entity| world.get::<EditableText>(*entity).is_some())
        });
    let focus_state = input
        .and_then(|entity| world.get::<StyleData>(entity))
        .map_or(root_state, StyleData::get_pseudo_state);
    let text_selected = input
        .and_then(|entity| world.get::<EditableText>(entity))
        .map(|text| !text.editor().raw_selection().is_collapsed());
    Some(
        json!({"hovered":root_state.hovered,"pressed":root_state.pressed,
        "focused":focus_state.focused,"focus_visible":focus_state.focused_and_visible,
        "disabled":root_state.disabled,"text_selected":text_selected}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_calibration_preserves_values_and_rejects_invalid_periods_and_text() {
        let app = crate::test_support::diagnostic_app();
        let scene = crate::views::compiled(app.world(), "editor://fixtures/catalog.scene.ron")
            .unwrap_or_else(|error| panic!("{error}"));
        let root = scene
            .source_entities()
            .get(&scene.graph().root)
            .unwrap_or_else(|| panic!("Missing canvas source root"));
        let mut canvas = scene
            .authoring_world()
            .get::<Canvas>(*root)
            .unwrap_or_else(|| panic!("Missing authored calibration"))
            .clone();
        assert_eq!(canvas.empty_caption, "Save changes");
        assert_eq!(canvas.field_text, "Main Camera");
        assert_eq!(canvas.cursor_blink_seconds, 86400.);
        assert!(canvas.round_layout);
        assert_eq!(canvas.hinting, bevy::text::FontHinting::Enabled);
        assert_eq!(canvas.density, 1.);
        assert!(validate_canvas(&canvas).is_ok());
        for density in [0., f32::NAN, f32::INFINITY, 4.1] {
            canvas.density = density;
            assert!(validate_canvas(&canvas).is_err());
        }
        canvas.density = 1.;
        for period in [0., -1., f32::NAN, f32::INFINITY, 86401.] {
            canvas.cursor_blink_seconds = period;
            assert!(validate_canvas(&canvas).is_err());
        }
        canvas.cursor_blink_seconds = 86400.;
        canvas.field_text = "名".repeat(4097);
        assert!(validate_canvas(&canvas).is_err());
    }
    #[test]
    fn unbound_catalog_rejects_mount_before_touching_native_focus_or_sources() {
        let mut world = World::new();
        world.init_resource::<Catalog>();
        world.init_resource::<bevy::input_focus::InputFocus>();
        let focused = world.spawn_empty().id();
        world
            .resource_mut::<bevy::input_focus::InputFocus>()
            .set(focused, bevy::input_focus::FocusCause::Navigated);
        let result = mount(
            &mut world,
            Selection {
                source: "editor://prefabs/button.prefab.ron".into(),
                disabled: false,
            },
        );
        assert!(result.is_err_and(|e| e.contains("No catalog canvas")));
        assert_eq!(
            world.resource::<bevy::input_focus::InputFocus>().get(),
            Some(focused)
        );
        assert!(specimen_slot("editor://prefabs/unlisted.prefab.ron").is_err());
    }
}
