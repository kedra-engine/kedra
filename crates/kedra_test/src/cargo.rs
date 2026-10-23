//! Ordinary Cargo targets are the scenario boundary, without registries or DSLs.
use crate::Result;
use cargo_metadata::{Message, MetadataCommand};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Compile one package's named bin or example, discovering the executable from
/// Cargo messages. Diagnostics stream live. Never guesses target-directory paths.
/// Requires an existing lockfile; compilation failure or ambiguous identity fails.
pub fn build(
    manifest: &Path,
    package: &str,
    name: &str,
    example: bool,
    features: &[&str],
) -> Result<PathBuf> {
    let manifest = manifest.canonicalize()?;
    let metadata = MetadataCommand::new()
        .current_dir(manifest.parent().ok_or("Manifest directory missing")?)
        .manifest_path(&manifest)
        .no_deps()
        .other_options(vec!["--locked".into()])
        .exec()?;
    let packages: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| p.name.as_str() == package)
        .collect();
    if packages.len() != 1 {
        return Err("Missing or ambiguous Cargo package".into());
    }
    let selected = packages[0];
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .current_dir(manifest.parent().ok_or("Manifest parent missing")?)
        .args([
            "build",
            "--locked",
            "--message-format=json-render-diagnostics",
            "--manifest-path",
        ])
        .arg(&manifest)
        .args([
            "--package",
            package,
            if example { "--example" } else { "--bin" },
            name,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if !features.is_empty() {
        command.args(["--features", &features.join(",")]);
    }
    eprintln!("Building {package}/{name} through Cargo…");
    let mut child = command.spawn()?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Cargo stdout unavailable".into());
    };
    let mut executables = std::collections::BTreeSet::new();
    let mut invalid = false;
    for message in Message::parse_stream(BufReader::new(stdout)) {
        match message {
            Ok(Message::CompilerArtifact(artifact))
                if artifact.package_id == selected.id && artifact.target.name == name =>
            {
                if let Some(path) = artifact.executable {
                    executables.insert(path.into_std_path_buf());
                }
            }
            Ok(Message::CompilerMessage(message)) => {
                if let Some(rendered) = message.message.rendered {
                    eprint!("{rendered}");
                }
            }
            Ok(Message::TextLine(line)) => eprintln!("{line}"),
            Err(_) => invalid = true,
            _ => {}
        }
    }
    if !child.wait()?.success() || invalid || executables.len() != 1 {
        return Err("Cargo failed or did not produce exactly one selected executable".into());
    }
    executables
        .into_iter()
        .next()
        .filter(|path| path.is_file())
        .ok_or_else(|| "Cargo executable missing".into())
}

/// Build and execute an ordinary project-owned Rust scenario bin or example. Arguments are
/// passed unchanged, cwd is the scenario manifest's directory, and nonzero exit
/// fails the test. The target owns setup, evidence and graceful native shutdown.
pub fn run(
    manifest: &Path,
    package: &str,
    binary: &str,
    example: bool,
    arguments: &[std::ffi::OsString],
) -> Result<()> {
    let manifest = manifest.canonicalize()?;
    let executable = build(&manifest, package, binary, example, &[])?;
    eprintln!("Running Rust test scenario {package}/{binary}…");
    let status = Command::new(executable)
        .current_dir(manifest.parent().ok_or("Manifest directory missing")?)
        .args(arguments)
        .status()?;
    if !status.success() {
        return Err(format!("Rust test scenario failed: {status}").into());
    }
    eprintln!("SUCCESS: Rust test scenario completed");
    Ok(())
}
