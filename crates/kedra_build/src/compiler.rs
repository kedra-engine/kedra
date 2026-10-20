//! Cargo owns builds and artifact identities; diagnostics remain live.
use crate::config::Application;
use cargo_metadata::Message;
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) fn build(app: &Application, release: bool, icon: &Path) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.current_dir(
        app.package
            .manifest_path
            .parent()
            .ok_or("Application manifest directory missing")?,
    );
    command
        .args([
            "build",
            "--locked",
            "--message-format=json-render-diagnostics",
            "--manifest-path",
        ])
        .arg(&app.package.manifest_path)
        .args([
            "--package",
            app.package.name.as_str(),
            "--bin",
            &app.binary,
            "--target",
            app.target.triple(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if release {
        command.arg("--release");
    }
    if app.target == crate::config::NativeTarget::Windows {
        command.env("KEDRA_WINDOWS_ICON", icon);
    }
    if let Some(version) = app
        .native
        .macos
        .as_ref()
        .and_then(|config| config.minimum_system_version.as_ref())
    {
        command.env("MACOSX_DEPLOYMENT_TARGET", version);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start Cargo: {e}"))?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Cargo diagnostic stream missing".into());
    };
    let mut executable = None;
    let mut parse_error = None;
    for message in Message::parse_stream(BufReader::new(stdout)) {
        match message {
            Ok(Message::CompilerArtifact(artifact))
                if artifact.package_id == app.package.id && artifact.target.name == app.binary =>
            {
                if let Some(path) = artifact.executable {
                    executable = Some(path.into_std_path_buf());
                }
            }
            Ok(Message::CompilerMessage(message)) => {
                if let Some(rendered) = message.message.rendered {
                    eprint!("{rendered}");
                }
            }
            Ok(Message::TextLine(line)) => eprintln!("{line}"),
            Err(error) => {
                parse_error = Some(error.to_string());
            }
            _ => {}
        }
    }
    let status = child
        .wait()
        .map_err(|e| format!("Cannot wait for Cargo: {e}"))?;
    if !status.success() {
        return Err(format!("Cargo compilation failed: {status}"));
    }
    if let Some(error) = parse_error {
        return Err(format!("Invalid Cargo artifact stream: {error}"));
    }
    let executable =
        executable.ok_or("Cargo succeeded without the selected application executable")?;
    if !executable.is_file() {
        return Err("Cargo application executable is missing".into());
    }
    Ok(executable)
}
