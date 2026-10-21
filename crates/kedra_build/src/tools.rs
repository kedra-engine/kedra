//! Explicit user-local tooling setup; builds never install system prerequisites.
use crate::config::{Application, NativeTarget};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::Path, process::Command};

const ABOUT_VERSION: &str = "0.9.2";
const ABOUT_CHECKSUM: &str = "0cd19d99696eb83f0a2d6ab7a347b14968d2980416c8cca827ded220e6e9c4bb";

pub(crate) fn preflight(app: &Application, package: bool) -> Result<(), String> {
    eprintln!("Checking native build prerequisites");
    check(
        Command::new(cargo()).arg("--version"),
        "Install Rust with rustup",
    )?;
    if app.distribution.license_config.is_some() {
        check_about()?;
    }
    for (tool, argument) in required_tools(app.target, package) {
        probe(tool, argument)?;
    }
    Ok(())
}

fn required_tools(target: NativeTarget, package: bool) -> Vec<(&'static str, &'static str)> {
    let mut tools = match target {
        NativeTarget::MacArm | NativeTarget::MacIntel => {
            vec![("codesign", "--help"), ("plutil", "-help")]
        }
        NativeTarget::Linux => vec![("ldd", "--version")],
        NativeTarget::Windows => Vec::new(),
    };
    if package {
        match target {
            NativeTarget::MacArm | NativeTarget::MacIntel => {
                tools.extend([("hdiutil", "help"), ("ditto", "--help")])
            }
            NativeTarget::Linux => tools.extend([
                ("lddtree", "--version"),
                ("dpkg-query", "--version"),
                ("tar", "--version"),
            ]),
            NativeTarget::Windows => tools.push(("7z", "i")),
        }
    }
    tools
}

pub(crate) fn check_about() -> Result<(), String> {
    let result = Command::new("cargo-about").arg("--version").output()
        .map_err(|e| format!("Configured notices require audited cargo-about {ABOUT_VERSION}: {e}. Use cargo kedra tools install-about --output PATH outside the workspace, then add PATH/bin to PATH"))?;
    if !result.status.success()
        || String::from_utf8_lossy(&result.stdout).trim() != format!("cargo-about {ABOUT_VERSION}")
    {
        return Err(format!(
            "Configured notices require cargo-about {ABOUT_VERSION}"
        ));
    }
    Ok(())
}

pub(crate) fn install_about(output: &Path, archive: Option<&Path>) -> Result<(), String> {
    let output = std::path::absolute(output).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(&output).is_ok() {
        return Err(
            "Tool output must be a new directory; previous installations are preserved".into(),
        );
    }
    eprintln!("Preparing checksum-verified cargo-about {ABOUT_VERSION} source");
    let bytes = if let Some(path) = archive {
        fs::read(path).map_err(|e| format!("Tool source archive: {e}"))?
    } else {
        ureq::get(&format!(
            "https://crates.io/api/v1/crates/cargo-about/{ABOUT_VERSION}/download"
        ))
        .header("User-Agent", "Kedra distribution tooling")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?
    };
    install_verified(&output, &bytes, ABOUT_CHECKSUM, &cargo())
}

fn install_verified(
    output: &Path,
    bytes: &[u8],
    checksum: &str,
    cargo: &std::ffi::OsStr,
) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(bytes)) != checksum {
        return Err("cargo-about source checksum mismatch; nothing extracted or executed".into());
    }
    let ancestor = output
        .ancestors()
        .skip(1)
        .find(|p| p.is_dir())
        .ok_or("Tool output ancestor is unavailable")?;
    let location = Command::new(cargo)
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .current_dir(ancestor)
        .output()
        .map_err(|e| format!("Cannot check tool workspace isolation: {e}"))?;
    if location.status.success() {
        return Err(
            "Tool source must be outside any Cargo workspace; choose an OS tool-cache directory"
                .into(),
        );
    }
    fs::create_dir_all(output.parent().ok_or("Tool output parent missing")?)
        .map_err(|e| e.to_string())?;
    fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(Cursor::new(bytes)));
    archive
        .unpack(output.join("source"))
        .map_err(|e| format!("Tool source extraction: {e}"))?;
    let manifest = output
        .join("source")
        .join(format!("cargo-about-{ABOUT_VERSION}/Cargo.toml"));
    check(
        Command::new(cargo)
            .arg("update")
            .arg("--manifest-path")
            .arg(&manifest),
        "Tool dependency resolution failed",
    )?;
    check(
        Command::new(cargo)
            .arg("deny")
            .arg("--manifest-path")
            .arg(&manifest)
            .args(["--locked", "check", "advisories"]),
        "Tool advisory audit failed; nothing compiled",
    )?;
    check(
        Command::new(cargo)
            .arg("install")
            .arg("--path")
            .arg(manifest.parent().ok_or("Tool source missing")?)
            .args(["--features", "cli", "--locked", "--root"])
            .arg(output)
            .arg("--target-dir")
            .arg(output.join("build")),
        "Tool installation failed",
    )?;
    let executable = output
        .join("bin")
        .join(format!("cargo-about{}", std::env::consts::EXE_SUFFIX));
    check(
        Command::new(executable).arg("--version"),
        "Installed collector is not executable",
    )?;
    eprintln!(
        "SUCCESS: audited isolated tool installed at {}; add its bin directory to PATH",
        output.display()
    );
    Ok(())
}

fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn prerequisite_command(tool: &str) -> Command {
    if tool == "lddtree" {
        lddtree_command()
    } else {
        Command::new(tool)
    }
}

pub(crate) fn lddtree_command() -> Command {
    // apt's pyelftools belongs to the distro Python, not setup-python or a venv.
    let mut command = Command::new("/usr/bin/python3");
    command.arg("/usr/bin/lddtree");
    command
}

fn probe(tool: &str, argument: &str) -> Result<(), String> {
    let result = prerequisite_command(tool)
        .arg(argument)
        .output()
        .map_err(|e| format!("Install the native prerequisite {tool}: {e}"))?;
    // Apple's command-line tools use nonzero help statuses.
    if result.status.success()
        || matches!(
            (tool, result.status.code()),
            ("codesign", Some(2)) | ("ditto", Some(1))
        )
    {
        return Ok(());
    }
    Err(format!(
        "Native prerequisite {tool} failed: {}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    ))
}

fn check(command: &mut Command, context: &str) -> Result<(), String> {
    eprintln!("Running {command:?}");
    let status = command.status().map_err(|e| format!("{context}: {e}"))?;
    if !status.success() {
        return Err(format!("{context}: {status}"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
