//! An independent Bevy consumer: no editor, UI, authoring, assets, QA or RPC.
use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use clap::Parser;
use kedra_profiling::{CaptureOptions, CaptureState, Recorder};
use std::{process::ExitCode, time::Duration};

#[derive(Parser)]
#[command(about = "Record a finite independent Bevy application for local Perfetto analysis")]
struct Options {
    #[command(flatten)]
    profiling: CaptureOptions,
}

fn run(options: Options) -> Result<AppExit, Box<dyn std::error::Error>> {
    if options.profiling.profile_output.is_none() || options.profiling.profile_manual {
        return Err(
            "Pass --profile-output DIRECTORY; this independent example records automatically"
                .into(),
        );
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1. / 60.,
        ))),
        bevy::log::LogPlugin {
            custom_layer: kedra_profiling::log_layer,
            ..default()
        },
    ));
    options.profiling.configure(
        &mut app,
        kedra_profiling::Target {
            instance: uuid::Uuid::new_v4(),
            name: "Independent Bevy application".into(),
            build: format!(
                "{}; debug_assertions={}",
                env!("CARGO_PKG_VERSION"),
                cfg!(debug_assertions)
            ),
        },
    )?;
    app.insert_resource(Workload {
        updates: 0,
        counter: kedra_profiling::Counter::new(
            "example.completed_updates",
            kedra_profiling::Unit::Count,
        )?,
    });
    app.add_systems(Update, (workload, finished));
    Ok(app.run())
}

#[derive(Resource)]
struct Workload {
    updates: u32,
    counter: kedra_profiling::Counter,
}

fn workload(mut state: ResMut<Workload>) {
    static COMPUTE: kedra_profiling::Marker = kedra_profiling::Marker::new("example.compute")
        .with_category(kedra_profiling::Category::Application);
    let _scope = COMPUTE.enter();
    let mut value = 0u64;
    for n in 0..20_000 {
        value = std::hint::black_box(value.wrapping_add(n));
    }
    std::hint::black_box(value);
    state.updates = state.updates.saturating_add(1);
    if let Err(error) = state.counter.sample(state.updates) {
        eprintln!("CPU profiling counter FAILED: {error}");
    }
}

fn finished(recorder: Res<Recorder>, mut exit: MessageWriter<AppExit>) {
    match recorder.state() {
        CaptureState::Completed { .. } => {
            exit.write(AppExit::Success);
        }
        CaptureState::Failed { .. } | CaptureState::Cancelled { .. } => {
            exit.write(AppExit::error());
        }
        _ => {}
    }
}

fn main() -> ExitCode {
    match run(Options::parse()) {
        Ok(AppExit::Success) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("CPU profiling FAILED: {error}");
            ExitCode::FAILURE
        }
    }
}
