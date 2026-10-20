//! Release identities are explicit; local non-Git builds are not publishable.
use crate::config::Application;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Serialize)]
struct Artifact {
    name: String,
    format: &'static str,
    bytes: u64,
    sha256: String,
}

#[derive(Serialize)]
struct Inventory<'a> {
    schema: u32,
    application: &'a str,
    version: String,
    target: &'a str,
    source_revision: Option<String>,
    artifacts: Vec<Artifact>,
}

pub(crate) fn write(
    app: &Application,
    output: &Path,
    artifacts: Vec<(PathBuf, &'static str)>,
) -> Result<(), String> {
    let mut entries = Vec::new();
    let mut sums = String::new();
    for (path, format) in artifacts {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Artifact filename is not Unicode")?
            .to_owned();
        let mut file = fs::File::open(&path).map_err(|e| e.to_string())?;
        let bytes = file.metadata().map_err(|e| e.to_string())?.len();
        if bytes == 0 {
            return Err("Empty distribution artifact".into());
        }
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 65536];
        loop {
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        let sha256 = format!("{:x}", hash.finalize());
        sums.push_str(&format!("{sha256}  {name}\n"));
        entries.push(Artifact {
            name,
            format,
            bytes,
            sha256,
        });
    }
    let source_revision = revision(&app.workspace);
    if source_revision.is_none() {
        eprintln!(
            "NOTE: workspace is not a clean Git checkout; this local package is not eligible for release publication"
        );
    }
    let inventory = Inventory {
        schema: 1,
        application: app.package.name.as_str(),
        version: app.package.version.to_string(),
        target: app.target.triple(),
        source_revision,
        artifacts: entries,
    };
    let bytes = serde_json::to_vec_pretty(&inventory).map_err(|e| e.to_string())?;
    fs::File::create_new(output.join("artifact-inventory.json"))
        .and_then(|mut f| f.write_all(&bytes))
        .map_err(|e| e.to_string())?;
    fs::File::create_new(output.join("SHA256SUMS"))
        .and_then(|mut f| f.write_all(sums.as_bytes()))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn revision(workspace: &Path) -> Option<String> {
    let root = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !root.status.success() {
        return None;
    }
    let root = String::from_utf8(root.stdout).ok()?;
    let root = root.strip_suffix('\n')?;
    let root = if cfg!(windows) {
        root.strip_suffix('\r').unwrap_or(root)
    } else {
        root
    };
    if Path::new(root).canonicalize().ok()? != workspace.canonicalize().ok()? {
        return None;
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["status", "--porcelain=v1", "--untracked-files=normal"])
        .output()
        .ok()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return None;
    }
    let result = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !result.status.success() {
        return None;
    }
    let value = String::from_utf8(result.stdout).ok()?.trim().to_owned();
    (value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit())).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_workspaces_cannot_claim_a_publishable_source_revision()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = tempfile::tempdir()?;
        let project = parent.path().join("workspace space Юникод ");
        fs::create_dir(&project)?;
        let hooks = tempfile::tempdir()?;
        let git = |args: &[&str]| -> Result<(), Box<dyn std::error::Error>> {
            let output = Command::new("git")
                .arg("-C")
                .arg(&project)
                .args(["-c", "commit.gpgsign=false", "-c"])
                .arg(format!("core.hooksPath={}", hooks.path().display()))
                .args(args)
                .output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(())
        };
        git(&["init", "-q"])?;
        fs::write(project.join("source.txt"), "original")?;
        git(&["add", "source.txt"])?;
        git(&[
            "-c",
            "user.name=Distribution fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ])?;
        assert!(revision(&project).is_some());
        fs::write(project.join("source.txt"), "modified")?;
        assert!(revision(&project).is_none());
        Ok(())
    }
}
