//! Native readiness contract: stdin releases a worker, not a window/RPC event.
//! The externally supervised diagnostic owns its output directory exclusively.
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::PipelineCache,
        view::{
            ViewTarget,
            screenshot::{Screenshot, ScreenshotCaptured},
        },
    },
    tasks::IoTaskPool,
};
use kedra_wakeup::{ReactivePlugin, Task, WakeDemand, Wakeup};
use std::{
    io::BufRead,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Resource, Clone)]
struct RenderReady(Arc<AtomicBool>);

#[derive(Resource)]
struct Probe {
    output: PathBuf,
    phase: Phase,
    task: Option<Worker>,
    control: bool,
    timed: bool,
    deadline: Option<Instant>,
    deadline_elapsed_ms: Option<u128>,
    frames: u64,
    outcome: Option<Result<String, String>>,
}
enum Worker {
    Waking(Task<Result<String, String>>),
    Control(bevy::tasks::Task<Result<String, String>>),
}
#[derive(PartialEq)]
enum Phase {
    Preparing,
    Before,
    Armed,
    After,
    Done,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(debug_assertions) {
        return Err("Native readiness probe requires a debug build".into());
    }
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected new output directory")?);
    let (control, timed) = match args.next().as_deref().and_then(|v| v.to_str()) {
        None => (false, false),
        Some("--without-wakeup") => (true, false),
        Some("--deadline") => (false, true),
        _ => return Err("Expected optional --without-wakeup or --deadline".into()),
    };
    if args.next().is_some() {
        return Err("Unexpected argument".into());
    }
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&output)?;
    eprintln!(
        "Native readiness: capturing initial frame; output {}",
        output.display()
    );
    let mut app = App::new();
    // Cover late native setup after an initially headless resource as well.
    app.init_resource::<Wakeup>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Kedra native task readiness".into(),
            resolution: (320, 240).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(ReactivePlugin {
        idle: Duration::MAX,
        focused_active: Duration::from_millis(16),
        unfocused_active: Duration::from_millis(16),
    });
    let wakeup = Wakeup::for_world(app.world());
    let retired_wakeup = wakeup.clone();
    let closed_marker = output.join("closed-loop");
    let rendered = RenderReady(Arc::new(AtomicBool::new(false)));
    app.sub_app_mut(RenderApp)
        .insert_resource(rendered.clone())
        .add_systems(Render, render_ready.after(RenderSystems::Render));
    app.insert_resource(wakeup)
        .insert_resource(rendered)
        .insert_resource(Probe {
            output,
            phase: Phase::Preparing,
            task: None,
            control,
            timed,
            deadline: None,
            deadline_elapsed_ms: None,
            frames: 0,
            outcome: None,
        })
        .insert_resource(ClearColor(Color::srgb(0., 0., 1.)))
        .add_systems(Startup, setup)
        .add_systems(Update, poll)
        .add_systems(Last, keep_readback_running);
    if app.run().is_error() {
        return Err("Native task readiness probe failed".into());
    }
    retired_wakeup.notify();
    std::fs::write(
        closed_marker,
        "closed-loop notification discarded without replay",
    )?;
    eprintln!("SUCCESS: native task result, rendered frame and closed-loop notification");
    Ok(())
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
}

fn render_ready(
    views: Query<(), With<ViewTarget>>,
    cache: Res<PipelineCache>,
    ready: Res<RenderReady>,
) {
    // Observe renderer-owned readiness after submission, not a fixed frame count
    // or successful screenshot. Do not sleep/exit while pipelines are compiling.
    if !views.is_empty()
        && cache.pipelines().next().is_some()
        && cache.waiting_pipelines().next().is_none()
    {
        ready.0.store(true, Ordering::Release);
    }
}

fn screenshot(world: &mut World) {
    world.spawn(Screenshot::primary_window()).observe(captured);
}

fn captured(
    event: On<ScreenshotCaptured>,
    wakeup: Res<Wakeup>,
    mut demand: ResMut<WakeDemand>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    let name = if probe.phase == Phase::Before {
        "before.png"
    } else {
        "after.png"
    };
    let saved = event
        .image
        .clone()
        .try_into_dynamic()
        .map_err(|e| e.to_string())
        .and_then(|image| {
            image
                .save(probe.output.join(name))
                .map_err(|e| e.to_string())
        });
    if let Err(error) = saved {
        eprintln!("FAILED readback: {error}");
        exit.write(AppExit::error());
        return;
    }
    if probe.phase == Phase::Before {
        eprintln!("Native readiness: initial readback complete; waiting for worker input");
        if probe.timed {
            let deadline = Instant::now() + Duration::from_millis(350);
            probe.deadline = Some(deadline);
            demand.at(deadline);
            probe.phase = Phase::Armed;
            return;
        }
        let worker_marker = probe.output.join("worker-returning");
        let task = IoTaskPool::get().spawn(async move {
            let mut line = String::new();
            std::io::stdin()
                .lock()
                .read_line(&mut line)
                .map_err(|e| e.to_string())?;
            std::fs::write(worker_marker, "stdin consumed; returning owned Task output")
                .map_err(|e| e.to_string())?;
            match line.trim() {
                "complete" => Ok("owned result".into()),
                "fail" => Err("worker refused".into()),
                _ => Err("Unexpected worker release".into()),
            }
        });
        probe.task = Some(if probe.control {
            Worker::Control(task)
        } else {
            Worker::Waking(wakeup.track(task))
        });
        probe.phase = Phase::Armed;
    } else {
        probe.phase = Phase::Done;
        let result = serde_json::json!({"frames":probe.frames,"outcome":probe.outcome,"without_wakeup":probe.control,"native_readbacks":true,"deadline_elapsed_ms":probe.deadline_elapsed_ms});
        match serde_json::to_vec_pretty(&result)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                std::fs::write(probe.output.join("result.json"), bytes).map_err(|e| e.to_string())
            }) {
            Ok(()) => {
                exit.write(AppExit::Success);
            }
            Err(error) => {
                eprintln!("FAILED evidence: {error}");
                exit.write(AppExit::error());
            }
        }
    }
}

fn poll(world: &mut World) {
    let ready = world.resource_scope(|world, mut probe: Mut<Probe>| {
        probe.frames += 1;
        if let Err(error) =
            std::fs::write(probe.output.join("frames.txt"), probe.frames.to_string())
        {
            eprintln!("FAILED frame evidence: {error}");
            world.write_message(AppExit::error());
        }
        if probe.phase == Phase::Preparing
            && world.resource::<RenderReady>().0.load(Ordering::Acquire)
        {
            probe.phase = Phase::Before;
            screenshot(world);
        }
        if probe.phase != Phase::Armed {
            return None;
        }
        // Deliberately omit all owner polling before worker release. Readiness
        // must have been registered automatically at task construction, even
        // though admission happened late in the readback observer.
        let outcome = if let Some(deadline) = probe.deadline {
            if Instant::now() >= deadline {
                probe.deadline_elapsed_ms =
                    Some((Instant::now() - deadline + Duration::from_millis(350)).as_millis());
                probe.deadline = None;
                Some(Ok("deadline".into()))
            } else {
                world.resource_mut::<WakeDemand>().at(deadline);
                None
            }
        } else if probe.output.join("worker-returning").is_file() {
            probe.task.as_mut().and_then(|task| match task {
                Worker::Control(task) => bevy::tasks::block_on(bevy::tasks::poll_once(task)),
                Worker::Waking(task) => task.poll(),
            })
        } else {
            None
        };
        if let Some(outcome) = outcome {
            eprintln!("Native readiness: owner received {outcome:?}; capturing result frame");
            probe.task = None;
            let success = outcome.is_ok();
            probe.outcome = Some(outcome);
            probe.phase = Phase::After;
            Some(success)
        } else {
            if let Err(error) = std::fs::write(
                probe.output.join("armed"),
                "task admitted; owner polling deferred until worker release; no RPC or sequence",
            ) {
                eprintln!("FAILED arming evidence: {error}");
                world.write_message(AppExit::error());
            }
            None
        }
    });
    if let Some(success) = ready {
        if success {
            world.resource_mut::<ClearColor>().0 = Color::srgb(0., 1., 0.);
        }
        screenshot(world);
    }
}

fn keep_readback_running(
    probe: Res<Probe>,
    mut redraw: MessageWriter<bevy::window::RequestRedraw>,
) {
    if matches!(probe.phase, Phase::Preparing | Phase::Before | Phase::After) {
        redraw.write(bevy::window::RequestRedraw);
    }
}
