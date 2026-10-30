//! Native Task readiness has no RPC/sequence polling that could mask a missing wake.
use kedra_test::{Result, evidence, process::NativeProcess};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn frames(path: &Path) -> Result<u64> {
    Ok(std::str::from_utf8(&evidence::read(&path.join("frames.txt"), 64)?)?.parse()?)
}

fn sleeping(process: &mut NativeProcess, artifacts: &Path) -> Result<(u64, String)> {
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        process.require_running()?;
        if artifacts.join("armed").is_file() {
            let wait = fs::read_to_string(format!("/proc/{}/wchan", process.id()))?;
            if ["ep_poll", "epoll", "poll_schedule"]
                .iter()
                .any(|name| wait.contains(name))
            {
                return Ok((frames(artifacts)?, wait.trim().into()));
            }
        }
        if Instant::now() >= deadline {
            return Err("Native event loop did not enter an observed poll wait".into());
        }
        // Poll actual native OS readiness, not a screenshot settling delay.
        thread::sleep(Duration::from_millis(10));
    }
}

fn frame(path: &Path, color: [u8; 4]) -> Result<Vec<u8>> {
    let bytes = evidence::read(path, 64 * 1024 * 1024)?;
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?.to_rgba8();
    if image.dimensions() != (320, 240) || image.pixels().any(|pixel| pixel.0 != color) {
        return Err("Native diagnostic readback differs from its owned clear-color state".into());
    }
    Ok(image.into_raw())
}

fn exercise(
    executable: &Path,
    private: &Path,
    name: &str,
    failure: bool,
    control: bool,
    timed: bool,
) -> Result<Value> {
    let artifacts = private.join("evidence");
    let mut command = Command::new(executable);
    command
        .arg(&artifacts)
        .stdin(Stdio::piped())
        .env_remove("WAYLAND_DISPLAY");
    if control {
        command.arg("--without-wakeup");
    }
    if timed {
        command.arg("--deadline");
    }
    let mut process = NativeProcess::spawn(&mut command, private)?;
    let (before, wait_channel) = sleeping(&mut process, &artifacts)?;
    if !timed {
        process.write_stdin(if failure { b"fail\n" } else { b"complete\n" })?;
    }
    let deadline = Instant::now() + Duration::from_secs(if control { 3 } else { 12 });
    loop {
        if let Some(status) = process.try_wait()? {
            if control {
                return Err(
                    "No-wakeup control completed without a native readiness notification".into(),
                );
            }
            if !status.success() {
                return Err(format!("Native readiness host failed: {status}").into());
            }
            let result: Value =
                serde_json::from_slice(&evidence::read(&artifacts.join("result.json"), 4096)?)?;
            let outcome = if timed {
                json!({"Ok":"deadline"})
            } else if failure {
                json!({"Err":"worker refused"})
            } else {
                json!({"Ok":"owned result"})
            };
            if result["outcome"] != outcome
                || result["frames"]
                    .as_u64()
                    .is_none_or(|frame| frame <= before)
                || !artifacts.join("closed-loop").is_file()
                || (!timed && !artifacts.join("worker-returning").is_file())
                || (timed
                    && result["deadline_elapsed_ms"]
                        .as_u64()
                        .is_none_or(|elapsed| elapsed < 350))
            {
                return Err(
                    "Native readiness result/ownership/closed-loop evidence is incomplete".into(),
                );
            }
            let original = frame(&artifacts.join("before.png"), [0, 0, 255, 255])?;
            let after = frame(
                &artifacts.join("after.png"),
                if failure {
                    [0, 0, 255, 255]
                } else {
                    [0, 255, 0, 255]
                },
            )?;
            if failure && original != after {
                return Err("Worker failure changed the rendered state".into());
            }
            return Ok(
                json!({"case":name,"status":"passed","outcome":outcome,"native_wait_channel":wait_channel,"frames_before_release":before,"frames_after":result["frames"],"readback_size":[320,240],"deadline_elapsed_ms":result["deadline_elapsed_ms"],"input":if timed { "native deadline only; no worker release/RPC/Luau/window input" } else { "worker pipe; no RPC/Luau/window input" }}),
            );
        }
        if Instant::now() >= deadline {
            if !control {
                return Err(
                    "Native worker result did not wake and render before the external deadline"
                        .into(),
                );
            }
            if !artifacts.join("worker-returning").is_file()
                || artifacts.join("result.json").exists()
                || artifacts.join("after.png").exists()
                || frames(&artifacts)? != before
            {
                return Err(
                    "No-wakeup control did not remain asleep with its worker output unconsumed"
                        .into(),
                );
            }
            // Drop kills/reaps this owned negative control. Forced cleanup is an
            // expected control outcome, never a successful application shutdown.
            return Ok(
                json!({"case":name,"status":"expected_external_deadline_no_wake","native_wait_channel":wait_channel,"frames_before_release":before,"frames_after":before,"worker_input_consumed":true,"forced_control_cleanup":true}),
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn run(root: &Path, output: &Path) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Err("Native readiness OS-wait observation is currently Linux-only".into());
    }
    kedra_test::process::ensure_desktop()?;
    let executable = kedra_test::cargo::build(
        &root.join("Cargo.toml"),
        "kedra_wakeup",
        "native_readiness",
        true,
        &["native"],
    )?;
    fs::create_dir(output.join("public"))?;
    let mut cases = Vec::new();
    let mut failed = false;
    for (name, failure, control, timed) in [
        ("success", false, false, false),
        ("worker-failure", true, false, false),
        ("no-wakeup-control", false, true, false),
        ("deadline", false, false, true),
    ] {
        eprintln!("Native readiness: {name}");
        let result = exercise(
            &executable,
            &output.join("private").join(name),
            name,
            failure,
            control,
            timed,
        );
        let record = match result {
            Ok(record) => record,
            Err(error) => {
                failed = true;
                eprintln!("FAILED {name}: {error}");
                json!({"case":name,"status":"failed","error":error.to_string()})
            }
        };
        cases.push(record);
        fs::write(
            output.join("public/report.json"),
            serde_json::to_vec_pretty(
                &json!({"cases":cases,"scope":"native task readiness and renderer delivery; not editor idle pacing or physical input latency"}),
            )?,
        )?;
    }
    eprintln!(
        "Native readiness: {} — {}",
        if failed { "failed" } else { "passed" },
        output.join("public/report.json").display()
    );
    if failed {
        Err("Native readiness contract failed; retained evidence was not retried".into())
    } else {
        Ok(())
    }
}
