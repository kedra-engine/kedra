//! Bounded, opt-in main-world timing over Bevy's diagnostic history.
//! Update intervals include pacing/waits; they are not GPU or input-to-photon timings.
use bevy::{
    diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, DiagnosticsStore, RegisterDiagnostic},
    prelude::*,
};
use bevy_remote::BrpResult;
use serde::Deserialize;
use serde_json::{Value, json};

const UPDATE_INTERVAL: DiagnosticPath = DiagnosticPath::const_new("kedra/app/update_interval_ms");
const HISTORY: usize = 4096;
const NODE_CHANGES: DiagnosticPath = DiagnosticPath::const_new("kedra/ui/node_changes");
const TEXT_CHANGES: DiagnosticPath = DiagnosticPath::const_new("kedra/ui/text_changes");
const FONT_CHANGES: DiagnosticPath = DiagnosticPath::const_new("kedra/ui/font_changes");
const TRANSFORM_CHANGES: DiagnosticPath = DiagnosticPath::const_new("kedra/ui/transform_changes");

#[derive(Resource)]
pub(crate) struct Enabled;

#[derive(Resource, Default)]
struct MutationSample {
    nodes: Vec<Entity>,
    focus_events: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default)]
    include_changed_nodes: bool,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<MutationSample>()
        .register_diagnostic(
            Diagnostic::new(UPDATE_INTERVAL)
                .with_max_history_length(HISTORY)
                .with_suffix("ms"),
        )
        .add_systems(Update, measure.run_if(resource_exists::<Enabled>));
    for path in [NODE_CHANGES, TEXT_CHANGES, FONT_CHANGES, TRANSFORM_CHANGES] {
        app.register_diagnostic(Diagnostic::new(path).with_max_history_length(HISTORY));
    }
    app.add_systems(Last, ui_changes.run_if(resource_exists::<Enabled>));
    app.add_message::<bevy::window::WindowFocused>();
    use crate::RemoteAppExt;
    app.register_remote(
        "app.performance",
        "Read bounded update-interval statistics and aggregate UI inventory; not GPU or physical-input latency",
        json!({"include_changed_nodes":"optional boolean; at most 32 nodes from the latest sampled update, without text values"}),
        snapshot,
    );
}

fn ui_changes(
    nodes: Query<Entity, Changed<Node>>,
    text: Query<Entity, Changed<Text>>,
    fonts: Query<Entity, Changed<TextFont>>,
    transforms: Query<Entity, Changed<UiTransform>>,
    mut diagnostics: Diagnostics,
    mut sample: ResMut<MutationSample>,
    mut focus: MessageReader<bevy::window::WindowFocused>,
) {
    sample.nodes.clear();
    sample.nodes.extend(nodes.iter().take(32));
    sample.focus_events = sample
        .focus_events
        .saturating_add(focus.read().count() as u64);
    for (path, count) in [
        (NODE_CHANGES, nodes.iter().count()),
        (TEXT_CHANGES, text.iter().count()),
        (FONT_CHANGES, fonts.iter().count()),
        (TRANSFORM_CHANGES, transforms.iter().count()),
    ] {
        diagnostics.add_measurement(&path, || count as f64);
    }
}

fn measure(time: Res<Time<Real>>, mut diagnostics: Diagnostics) {
    if !time.delta().is_zero() {
        diagnostics.add_measurement(&UPDATE_INTERVAL, || time.delta_secs_f64() * 1000.);
    }
}

fn snapshot(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    let request: Option<Request> = crate::params(value)?;
    let mut values: Vec<_> = world
        .resource::<DiagnosticsStore>()
        .get(&UPDATE_INTERVAL)
        .into_iter()
        .flat_map(Diagnostic::values)
        .copied()
        .collect();
    values.sort_by(f64::total_cmp);
    let percentile = |p: f64| -> Option<f64> {
        (!values.is_empty()).then(|| values[((values.len() as f64 * p).ceil() as usize) - 1])
    };
    let intervals = json!({
        "samples": values.len(), "capacity": HISTORY,
        "mean_ms": (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        "min_ms": values.first(), "p50_ms": percentile(0.5),
        "p95_ms": percentile(0.95), "p99_ms": percentile(0.99), "max_ms": values.last(),
    });
    let nodes = world
        .query_filtered::<Entity, With<Node>>()
        .iter(world)
        .count();
    let text = world
        .query_filtered::<Entity, With<Text>>()
        .iter(world)
        .count();
    let editable = world
        .query_filtered::<Entity, With<bevy::text::EditableText>>()
        .iter(world)
        .count();
    let windows = world.query::<&Window>().iter(world).count();
    let changed_nodes = request.filter(|request| request.include_changed_nodes).map(|_| {
        world.resource::<MutationSample>().nodes.iter().map(|entity| {
            let parent = world.get::<ChildOf>(*entity).map(ChildOf::parent);
            json!({"entity":entity.to_bits(),
                "id":world.get::<kedra_ui::ElementId>(*entity).map(|id| &id.0),
                "parent":parent.map(Entity::to_bits),
                "parent_id":parent.and_then(|parent|world.get::<kedra_ui::ElementId>(parent)).map(|id|&id.0),
                "node":world.get::<Node>(*entity).map(|node|format!(
                    "display={:?} position={:?} width={:?} height={:?} left={:?} top={:?} min_width={:?} min_height={:?}",
                    node.display, node.position_type, node.width, node.height,
                    node.left, node.top, node.min_width, node.min_height))})
        }).collect::<Vec<_>>()
    });
    let changes = [
        ("nodes", NODE_CHANGES),
        ("text", TEXT_CHANGES),
        ("fonts", FONT_CHANGES),
        ("transforms", TRANSFORM_CHANGES),
    ]
    .into_iter()
    .map(|(name, path)| {
        let store = world.resource::<DiagnosticsStore>();
        let diagnostic = store.get(&path);
        (
            name.to_owned(),
            json!({"latest": diagnostic.and_then(Diagnostic::value),
            "mean_per_update": diagnostic.and_then(Diagnostic::average)}),
        )
    })
    .collect::<serde_json::Map<_, _>>();
    Ok(json!({
        "enabled": world.contains_resource::<Enabled>(),
        "frame": world.get_resource::<bevy::diagnostic::FrameCount>().map(|frame| frame.0),
        "update_intervals": intervals,
        "ui_changes": changes,
        "changed_ui_nodes": changed_nodes,
        "focus_events": world.resource::<MutationSample>().focus_events,
        "inventory": {"ui_nodes": nodes, "text_nodes": text, "editable_nodes": editable, "windows": windows},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::diagnostic::DiagnosticMeasurement;

    fn host() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<crate::CommandCatalog>()
            .insert_resource(bevy_remote::RemoteMethods::new());
        install(&mut app);
        app
    }

    #[test]
    fn empty_history_does_not_invent_timings_and_snapshot_does_not_mutate_ui()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = host();
        let node = app
            .world_mut()
            .spawn((Node::default(), Text::new("Retained 日本語")))
            .id();
        let report = snapshot(In(None), app.world_mut()).map_err(|error| error.message)?;
        assert_eq!(report["enabled"], false);
        assert_eq!(report["update_intervals"]["samples"], 0);
        assert!(report["update_intervals"]["p99_ms"].is_null());
        assert_eq!(
            app.world().get::<Text>(node).map(|text| text.0.as_str()),
            Some("Retained 日本語")
        );
        Ok(())
    }

    #[test]
    fn collection_requires_opt_in_and_details_exclude_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = host();
        app.world_mut()
            .spawn((Node::default(), Text::new("Private 日本語")));
        app.update();
        let report = snapshot(In(None), app.world_mut()).map_err(|error| error.message)?;
        assert_eq!(report["update_intervals"]["samples"], 0);
        assert!(report["changed_ui_nodes"].is_null());
        app.insert_resource(Enabled);
        app.world_mut()
            .spawn((Node::default(), Text::new("Private 日本語")));
        app.update();
        let report = snapshot(
            In(Some(json!({"include_changed_nodes":true}))),
            app.world_mut(),
        )
        .map_err(|error| error.message)?;
        assert_eq!(report["enabled"], true);
        assert!(
            !report["changed_ui_nodes"]
                .as_array()
                .ok_or("node details")?
                .is_empty()
        );
        assert!(!report.to_string().contains("Private"));
        assert!(
            snapshot(
                In(Some(json!({"include_changed_nodes":"yes"}))),
                app.world_mut()
            )
            .is_err()
        );
        assert!(snapshot(In(Some(json!({"reset":true}))), app.world_mut()).is_err());
        assert_eq!(
            app.world_mut().query::<&Text>().iter(app.world()).count(),
            2
        );
        Ok(())
    }

    #[test]
    fn timing_snapshot_reports_tail_latency_without_serializing_raw_history()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = host();
        let diagnostic = app
            .world_mut()
            .resource_mut::<DiagnosticsStore>()
            .into_inner()
            .get_mut(&UPDATE_INTERVAL)
            .ok_or("diagnostic")?;
        for value in [1., 2., 3., 4., 100.] {
            diagnostic.add_measurement(DiagnosticMeasurement {
                time: std::time::Instant::now(),
                value,
            });
        }
        let report = snapshot(In(None), app.world_mut()).map_err(|error| error.message)?;
        assert_eq!(report["update_intervals"]["p50_ms"], 3.);
        assert_eq!(report["update_intervals"]["p95_ms"], 100.);
        assert_eq!(report["update_intervals"]["mean_ms"], 22.);
        assert!(report["update_intervals"].get("values").is_none());
        Ok(())
    }
}
