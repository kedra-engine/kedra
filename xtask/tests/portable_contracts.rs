//! Provider-independent bootstrap and native-runtime dependency contracts.

#[test]
fn winit_fork_retains_exact_revision_and_reviewed_source() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    let manifest: toml::Value = toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml"))?)?;
    let fork = &manifest["patch"]["crates-io"]["winit"];
    assert_eq!(
        fork["git"].as_str(),
        Some("https://github.com/kedra-engine/winit.git")
    );
    assert_eq!(
        fork["rev"].as_str(),
        Some("948d27f0007602d2536a8974fd483c6fd33d19d3")
    );
    let lock = cargo_lock::Lockfile::load(root.join("Cargo.lock"))?;
    let winit: Vec<_> = lock
        .packages
        .iter()
        .filter(|package| package.name.as_str() == "winit")
        .collect();
    assert_eq!(winit.len(), 1);
    assert_eq!(winit[0].version.to_string(), "0.30.13");
    assert_eq!(
        winit[0]
            .source
            .as_ref()
            .ok_or("Winit source missing")?
            .to_string(),
        "git+https://github.com/kedra-engine/winit.git?rev=948d27f0007602d2536a8974fd483c6fd33d19d3#948d27f0007602d2536a8974fd483c6fd33d19d3"
    );
    let policy: toml::Value = toml::from_str(&std::fs::read_to_string(root.join("deny.toml"))?)?;
    assert_eq!(policy["sources"]["required-git-spec"].as_str(), Some("rev"));
    assert_eq!(
        policy["sources"]["allow-git"]
            .as_array()
            .ok_or("source allowlist missing")?,
        &[
            toml::Value::String("https://github.com/kedra-engine/winit.git".into()),
            toml::Value::String("https://github.com/crabnebula-dev/cargo-packager.git".into()),
        ]
    );
    Ok(())
}

#[test]
fn native_packager_source_is_reviewed_and_fixed() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    let manifest: toml::Value = toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml"))?)?;
    let packager = &manifest["workspace"]["dependencies"]["cargo-packager"];
    assert_eq!(
        packager["git"].as_str(),
        Some("https://github.com/crabnebula-dev/cargo-packager.git")
    );
    assert_eq!(
        packager["rev"].as_str(),
        Some("5c1824ba8022401e9ef0de36ff1a12c5acf60237")
    );
    assert_eq!(packager["default-features"].as_bool(), Some(false));
    Ok(())
}

#[test]
fn xtask_bootstrap_cannot_be_overwritten_by_workspace_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let config: toml::Value = toml::from_str(include_str!("../../.cargo/config.toml"))?;
    // Windows cannot replace a running executable: keep orchestration separate.
    assert_eq!(
        config["alias"]["xtask"].as_str(),
        Some("run --locked --package xtask --target-dir target/xtask-bootstrap --")
    );
    Ok(())
}

#[test]
fn independent_native_service_does_not_rely_on_dev_linux_backends()
-> Result<(), Box<dyn std::error::Error>> {
    let manifest: toml::Value = toml::from_str(include_str!("../../crates/kedra_app/Cargo.toml"))?;
    let features = manifest["dependencies"]["bevy"]["features"]
        .as_array()
        .ok_or("production Bevy features missing")?;
    // Dev/example feature unification must not supply the isolated service's backend.
    for backend in ["x11", "wayland"] {
        assert!(
            features
                .iter()
                .any(|feature| feature.as_str() == Some(backend)),
            "the isolated native service must enable {backend} in production"
        );
    }
    Ok(())
}
