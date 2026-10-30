//! Bootstrap isolation: Cargo must not replace its running Windows orchestrator.

#[test]
fn xtask_bootstrap_cannot_be_overwritten_by_workspace_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let config: toml::Value = toml::from_str(include_str!("../../.cargo/config.toml"))?;
    // Windows cannot replace a running executable: keep orchestration separate.
    let alias = config["alias"]["xtask"]
        .as_str()
        .ok_or("Cargo alias missing")?;
    // This is Cargo's whitespace-separated alias, not a shell command.
    let arguments: Vec<_> = alias.split_whitespace().collect();
    assert_eq!(arguments.first(), Some(&"run"));
    let target = arguments
        .windows(2)
        .find_map(|pair| (pair[0] == "--target-dir").then_some(pair[1]))
        .ok_or("Bootstrap must have an explicit isolated target directory")?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("root")?;
    std::fs::create_dir_all(root.join(target))?;
    assert_ne!(
        root.join(target).canonicalize()?,
        root.join("target").canonicalize()?
    );
    Ok(())
}
