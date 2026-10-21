//! Explicit standalone launch; request acceptance is not application/GUI readiness.
use crate::{
    application::BuiltApplication,
    config::{Application, NativeTarget},
};
use std::{
    ffi::OsString,
    fs,
    path::Path,
    process::{Command, Stdio},
};

pub(crate) fn request(
    app: &Application,
    built: &BuiltApplication,
    arguments: &[OsString],
) -> Result<(), String> {
    let root = crate::verify::member(&built.output, &built.result.application_root)?;
    let binary = crate::verify::member(&built.output, &built.result.executable)?;
    let mut command = command(app.target, &root, &binary, arguments);
    command
        .current_dir(&built.output)
        .env_remove("KEDRA_ASSETS")
        .stdin(Stdio::null());
    if matches!(app.target, NativeTarget::MacArm | NativeTarget::MacIntel) {
        let result = command
            .output()
            .map_err(|e| format!("Application built, but launch request failed: {e}"))?;
        if !result.status.success() {
            return Err(format!(
                "Application built, but launch request failed: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
    } else {
        let stdout = fs::File::create_new(built.output.join("application-stdout.log"))
            .map_err(|e| e.to_string())?;
        let stderr = fs::File::create_new(built.output.join("application-stderr.log"))
            .map_err(|e| e.to_string())?;
        command.stdout(stdout).stderr(stderr);
        independent(&mut command);
        let child = command
            .spawn()
            .map_err(|e| format!("Application built, but launch failed: {e}"))?;
        eprintln!(
            "Application process {} requested; logs: {}",
            child.id(),
            built.output.display()
        );
        // Reap eventual exit without making application lifetime a build operation.
        std::thread::spawn(move || {
            let mut child = child;
            let _ = child.wait();
        });
    }
    eprintln!("SUCCESS: launch request accepted; runtime exit and GUI readiness are not verified");
    Ok(())
}

fn command(target: NativeTarget, root: &Path, binary: &Path, arguments: &[OsString]) -> Command {
    if matches!(target, NativeTarget::MacArm | NativeTarget::MacIntel) {
        let mut command = Command::new("/usr/bin/open");
        command.arg("-n").arg(root).arg("--args").args(arguments);
        command
    } else {
        let mut command = Command::new(binary);
        command.args(arguments);
        command
    }
}

fn independent(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Windows process creation flags, via the safe standard-library API.
        command.creation_flags(0x0000_0200 | 0x0000_0008);
    }
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
