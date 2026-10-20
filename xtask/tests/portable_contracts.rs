//! Provider-independent bootstrap and native-runtime dependency contracts.

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
