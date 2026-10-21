//! Dependency license collection belongs to cargo-about, not a custom scanner.
use crate::config::Application;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

pub(crate) fn generate(app: &Application, config: &Path, output: &Path) -> Result<(), String> {
    let data = collect(app, config)?;
    let html = render(&data)?;
    let lock =
        cargo_lock::Lockfile::load(app.workspace.join("Cargo.lock")).map_err(|e| e.to_string())?;
    let sources = output.join("covered-sources");
    fs::create_dir(&sources).map_err(|e| e.to_string())?;
    let licenses = data["licenses"]
        .as_array()
        .ok_or("cargo-about license inventory missing")?;
    let mut seen = std::collections::BTreeSet::new();
    for license in licenses.iter().filter(|license| license["id"] == "MPL-2.0") {
        for consumer in license["used_by"]
            .as_array()
            .ok_or("License consumers missing")?
        {
            let package = &consumer["crate"];
            let (name, version) = covered_identity(package)?;
            if !seen.insert((name, version)) {
                continue;
            }
            let locked = lock
                .packages
                .iter()
                .find(|p| {
                    p.name.as_str() == name
                        && p.version.to_string() == version
                        && p.source
                            .as_ref()
                            .is_some_and(|source| source.is_default_registry())
                })
                .ok_or("Covered source package is absent from Cargo.lock")?;
            if !locked
                .source
                .as_ref()
                .is_some_and(|source| source.is_default_registry())
            {
                return Err(
                    "Covered non-crates.io sources require explicit source distribution support"
                        .into(),
                );
            }
            let checksum = locked
                .checksum
                .as_ref()
                .ok_or("Covered source checksum missing")?
                .to_string();
            let url = format!("https://crates.io/api/v1/crates/{name}/{version}/download");
            eprintln!("Archiving checksum-verified covered source: {name} {version}");
            let mut response = ureq::get(&url)
                .header("User-Agent", "Kedra distribution tooling")
                .call()
                .map_err(|e| format!("Covered source download: {e}"))?;
            let bytes = response
                .body_mut()
                .with_config()
                .limit(32 * 1024 * 1024)
                .read_to_vec()
                .map_err(|e| e.to_string())?;
            publish_source(
                &bytes,
                &sources.join(format!("{name}-{version}.crate")),
                &checksum,
            )?;
        }
    }
    fs::File::create_new(output.join("dependency-notices.html"))
        .and_then(|mut file| file.write_all(html.as_bytes()))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn collect(app: &Application, config: &Path) -> Result<Value, String> {
    let version = Command::new("cargo-about")
        .arg("--version")
        .output()
        .map_err(|e| {
            format!("Install audited cargo-about 0.9.2 for configured package notices: {e}")
        })?;
    if !version.status.success()
        || String::from_utf8_lossy(&version.stdout).trim() != "cargo-about 0.9.2"
    {
        return Err("Configured package notices require cargo-about 0.9.2".into());
    }
    let result = Command::new("cargo-about")
        .args([
            "generate",
            "--locked",
            "--fail",
            "--threshold",
            "0.95",
            "--format",
            "json",
            "--manifest-path",
        ])
        .arg(&app.package.manifest_path)
        .arg("--config")
        .arg(config)
        .args(["--target", app.target.triple()])
        .current_dir(
            app.package
                .manifest_path
                .parent()
                .ok_or("Application directory missing")?,
        )
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "Dependency license collection failed: {}",
            result.status
        ));
    }
    serde_json::from_slice(&result.stdout).map_err(|e| e.to_string())
}

fn render(data: &Value) -> Result<String, String> {
    if data["licenses"].as_array().is_none_or(Vec::is_empty) {
        return Err("Dependency license inventory is empty".into());
    }
    handlebars::Handlebars::new()
        .render_template(include_str!("license-notices.hbs"), data)
        .map_err(|e| e.to_string())
}

fn covered_identity(package: &Value) -> Result<(&str, &str), String> {
    if package["source"].as_str() != Some("registry+https://github.com/rust-lang/crates.io-index") {
        return Err(
            "Covered non-crates.io sources require explicit source distribution support".into(),
        );
    }
    Ok((
        package["name"]
            .as_str()
            .ok_or("Licensed package name missing")?,
        package["version"]
            .as_str()
            .ok_or("Licensed package version missing")?,
    ))
}

pub(crate) fn publish_source(bytes: &[u8], path: &Path, expected: &str) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        return Err("Covered source archive differs from its Cargo.lock checksum".into());
    }
    fs::File::create_new(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_preserve_copyright_without_leaking_local_source_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::json!({"licenses": [{"id": "MIT", "name": "MIT License",
            "text": "Copyright Example <authors>", "source_path": "/private/build/source/LICENSE",
            "used_by": [{"crate": {"name": "fixture", "version": "1.2.3",
                "source": "registry+https://github.com/rust-lang/crates.io-index"}}]}]});
        let html = render(&data)?;
        assert!(html.contains("Copyright Example &lt;authors&gt;"));
        assert!(html.contains("fixture 1.2.3"));
        assert!(!html.contains("/private/build"));
        Ok(())
    }

    #[test]
    fn source_archives_require_the_locked_checksum_and_never_overwrite()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("fixture-1.2.3.crate");
        assert!(publish_source(b"source archive", &path, &"0".repeat(64)).is_err());
        assert!(!path.exists());
        let hash = format!("{:x}", sha2::Sha256::digest(b"source archive"));
        publish_source(b"source archive", &path, &hash)?;
        assert!(publish_source(b"source archive", &path, &hash).is_err());
        assert_eq!(std::fs::read(path)?, b"source archive");
        Ok(())
    }

    #[test]
    fn covered_git_sources_cannot_be_replaced_with_same_version_registry_archives() {
        let package = serde_json::json!({"name": "fixture", "version": "1.2.3",
            "source": "git+https://example.invalid/source#0123456789"});
        assert!(covered_identity(&package).is_err());
    }
}
