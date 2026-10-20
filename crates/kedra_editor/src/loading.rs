//! Loading source bindings and task feedback, independent of its external skin.
use crate::{
    model::*,
    views::{self, Kind},
};
use bevy::{diagnostic::FrameCount, prelude::*};
use bevy_flair::style::components::ClassList;
use kedra_ui::{
    ScreenId,
    authored::{components::UiSpin, view::UiView},
};

#[derive(Component)]
pub(crate) struct HydratedAt(pub u32);
#[derive(Component)]
pub(crate) struct WorkingActions;
#[derive(Component)]
struct LogHost;
#[derive(Component)]
pub(crate) struct LogText;
#[derive(Component)]
struct LogToggle;
#[derive(Resource)]
pub(crate) struct Trace {
    last: String,
    entries: Vec<String>,
}

pub(crate) fn enter(world: &mut World) {
    let manager = world.resource::<ProjectManager>();
    let name = manager
        .intent
        .as_ref()
        .map(Intent::display_name)
        .unwrap_or_default();
    let path = manager
        .intent
        .as_ref()
        .map(Intent::root)
        .unwrap_or_default()
        .display()
        .to_string();
    let root = kedra_desktop::replace(
        world,
        kedra_desktop::SurfaceKind::Loading,
        format!("{name} — Kedra Engine"),
    );
    let mut manager = world.resource_mut::<ProjectManager>();
    manager.native_created_at = None;
    manager.loading_started = None;
    world.insert_resource(Trace {
        last: String::new(),
        entries: vec![format!("Project preparation requested: {path}")],
    });
    if let Err(error) = views::request(
        world,
        root,
        Kind::Frame(kedra_desktop::SurfaceKind::Loading),
    ) {
        views::fail(world, error);
    }
}

pub(crate) fn bind(world: &mut World, view: &UiView) -> Result<(), String> {
    let keys = [
        "loading.version",
        "loading.name",
        "loading.path",
        "loading.phase",
        "loading.actions",
        "loading.failure_actions",
        "loading.retry",
        "loading.back",
        "loading.log_toggle",
        "loading.failure_log_toggle",
        "loading.log",
        "loading.log_text",
    ];
    let elements = keys
        .into_iter()
        .map(|key| view.element(key).map(|entity| (key, entity)))
        .collect::<kedra_scene::Result<std::collections::BTreeMap<_, _>>>()
        .map_err(|e| e.to_string())?;
    let labels = [
        view.text(world, "loading.log_toggle"),
        view.text(world, "loading.failure_log_toggle"),
    ]
    .into_iter()
    .collect::<kedra_scene::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;
    let manager = world.resource::<ProjectManager>();
    let name = manager
        .intent
        .as_ref()
        .map(Intent::display_name)
        .unwrap_or_default();
    let path = manager
        .intent
        .as_ref()
        .map(Intent::root)
        .map(|path| crate::recent::display_path(&path))
        .unwrap_or_default();
    for (key, value) in [
        ("loading.version", env!("CARGO_PKG_VERSION").to_string()),
        ("loading.name", name),
        ("loading.path", path),
    ] {
        world
            .get_mut::<Text>(elements[key])
            .ok_or("loading text binding is not native Text")?
            .0 = value;
    }
    world
        .entity_mut(elements["loading.phase"])
        .insert(StatusLabel::Loading);
    world
        .entity_mut(elements["loading.actions"])
        .insert(WorkingActions);
    world
        .entity_mut(elements["loading.failure_actions"])
        .insert(FailureActions);
    world
        .entity_mut(elements["loading.retry"])
        .insert((RetryAction, Action::Retry));
    world
        .entity_mut(elements["loading.back"])
        .insert(Action::Back);
    for key in ["loading.log_toggle", "loading.failure_log_toggle"] {
        world.entity_mut(elements[key]).insert(Action::LoadingLog);
    }
    for entity in labels {
        world.entity_mut(entity).insert(LogToggle);
    }
    world.entity_mut(elements["loading.log"]).insert(LogHost);
    world
        .entity_mut(elements["loading.log_text"])
        .insert(LogText);
    let frame = world.resource::<FrameCount>().0;
    world
        .entity_mut(view.root)
        .insert((ScreenId("loading".into()), HydratedAt(frame)));
    Ok(())
}

pub(crate) fn toggle_log(world: &mut World) {
    let mut shown = false;
    for mut classes in world
        .query_filtered::<&mut ClassList, With<LogHost>>()
        .iter_mut(world)
    {
        shown = classes.contains("hidden");
        classes.toggle("hidden");
    }
    for mut text in world
        .query_filtered::<&mut Text, With<LogToggle>>()
        .iter_mut(world)
    {
        text.0 = if shown {
            "Hide startup log"
        } else {
            "Show startup log"
        }
        .into();
    }
}

pub(crate) fn phase(manager: &ProjectManager) -> String {
    manager
        .loading_error()
        .map(str::to_string)
        .unwrap_or_else(|| {
            if manager.is_ready() {
                "Preparing editor window…".into()
            } else {
                "Checking project sources…".into()
            }
        })
}

pub(crate) fn trace(
    time: Res<Time<Real>>,
    manager: Res<ProjectManager>,
    trace: Option<ResMut<Trace>>,
    mut logs: Query<&mut Text, With<LogText>>,
    mut spinners: Query<&mut UiSpin>,
) {
    if manager.phase() != Phase::Loading {
        return;
    }
    if let Some(mut trace) = trace {
        let phase = phase(&manager);
        if trace.last != phase {
            trace.entries.push(format!(
                "{:.2}s  {phase}",
                time.elapsed()
                    .saturating_sub(manager.loading_started.unwrap_or(time.elapsed()))
                    .as_secs_f32()
            ));
            trace.last = phase;
        }
        let text = trace.entries.join("\n");
        for mut log in &mut logs {
            if log.0 != text {
                log.0 = text.clone();
            }
        }
    }
    for mut spinner in &mut spinners {
        if spinner.paused != manager.loading_error().is_some() {
            spinner.paused = manager.loading_error().is_some();
        }
    }
}
