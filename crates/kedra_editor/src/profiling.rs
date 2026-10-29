//! Editor-only authorization and shortcut, consuming the ordinary runtime recorder.
use bevy::prelude::*;

pub(super) fn configure(
    app: &mut App,
    options: kedra_profiling::CaptureOptions,
) -> Result<(), String> {
    #[cfg(feature = "profiling")]
    if options.profile_output.is_some() {
        if app.get_sub_app(bevy::render::RenderApp).is_none() {
            return Err(
                "Editor profiling requires render plugins before recording configuration".into(),
            );
        }
        app.insert_resource(Settings(kedra_profiling::CaptureConfig {
            duration_ms: options.profile_duration_ms,
            buffer_kib: options.profile_buffer_kib,
        }));
        app.add_systems(PostUpdate, shortcut);
        app.insert_resource(Counters::new()?)
            .add_systems(Last, sample);
        eprintln!(
            "CPU profiling authorized: F9 starts/stops a finite recording; output opens locally in Perfetto UI"
        );
    }
    options
        .configure(
            app,
            kedra_profiling::Target {
                instance: uuid::Uuid::new_v4(),
                name: "Kedra Editor".into(),
                build: format!(
                    "{}; debug_assertions={}; CPU scope instrumentation",
                    env!("CARGO_PKG_VERSION"),
                    cfg!(debug_assertions)
                ),
            },
        )
        .map_err(|error| error.to_string())
}

#[cfg(feature = "profiling")]
#[derive(Resource)]
struct Counters {
    nodes: kedra_profiling::Counter,
    text: kedra_profiling::Counter,
    changed_nodes: kedra_profiling::Counter,
}

#[cfg(feature = "profiling")]
impl Counters {
    fn new() -> Result<Self, String> {
        use kedra_profiling::{Counter, Unit};
        Ok(Self {
            nodes: Counter::new("editor.ui.nodes", Unit::Count)
                .map_err(|error| error.to_string())?,
            text: Counter::new("editor.ui.text_nodes", Unit::Count)
                .map_err(|error| error.to_string())?,
            changed_nodes: Counter::new("editor.ui.changed_nodes", Unit::Count)
                .map_err(|error| error.to_string())?,
        })
    }
}

#[cfg(feature = "profiling")]
fn sample(
    recorder: Res<kedra_profiling::Recorder>,
    counters: Res<Counters>,
    nodes: Query<Entity, With<Node>>,
    text: Query<Entity, With<Text>>,
    changed: Query<Entity, Changed<Node>>,
) {
    if !matches!(
        recorder.state(),
        kedra_profiling::CaptureState::Recording { .. }
    ) {
        return;
    }
    static SAMPLE: kedra_profiling::Marker =
        kedra_profiling::Marker::new("editor.profiler.counters")
            .with_category(kedra_profiling::Category::Metadata);
    let _scope = SAMPLE.enter();
    for (counter, count) in [
        (&counters.nodes, nodes.iter().count()),
        (&counters.text, text.iter().count()),
        (&counters.changed_nodes, changed.iter().count()),
    ] {
        match i64::try_from(count)
            .map_err(|error| error.to_string())
            .and_then(|value| counter.sample(value).map_err(|error| error.to_string()))
        {
            Ok(()) => {}
            Err(error) => eprintln!("CPU profiling counter FAILED: {error}"),
        }
    }
}

#[cfg(feature = "profiling")]
#[derive(Resource)]
struct Settings(kedra_profiling::CaptureConfig);

#[cfg(feature = "profiling")]
fn shortcut(
    mut keys: MessageReader<bevy::input::keyboard::KeyboardInput>,
    windows: Query<&Window>,
    settings: Res<Settings>,
    mut recorder: ResMut<kedra_profiling::Recorder>,
) {
    for key in keys.read() {
        if key.key_code != KeyCode::F9
            || !key.state.is_pressed()
            || key.repeat
            || !windows.get(key.window).is_ok_and(|window| window.focused)
        {
            continue;
        }
        let result = if recorder.state().active() {
            recorder.stop()
        } else {
            recorder.start(settings.0).map(|_| ())
        };
        if let Err(error) = result {
            eprintln!("CPU profiling FAILED: {error}");
        }
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::*;

    #[test]
    fn missing_render_host_rejects_authorization_without_installing_a_partial_recorder()
    -> Result<(), Box<dyn std::error::Error>> {
        let output = tempfile::tempdir()?;
        let directory = output.path().join("not-created");
        let mut app = App::new();
        let result = configure(
            &mut app,
            kedra_profiling::CaptureOptions {
                profile_output: Some(directory.clone()),
                profile_manual: true,
                profile_duration_ms: 1000,
                profile_buffer_kib: 1024,
                ..default()
            },
        );
        assert!(matches!(result, Err(error) if error.contains("render plugins")));
        assert!(!app.world().contains_resource::<kedra_profiling::Recorder>());
        assert!(!app.world().contains_resource::<Settings>());
        assert!(!directory.exists());
        Ok(())
    }
}
