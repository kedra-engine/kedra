//! A real independent runtime resource consumer, without editor/import/tool edges.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = kedra_assets::location::bundled_asset_root(
        &std::env::current_exe()?,
        kedra_assets::location::BundlePlatform::current(),
    )?;
    let (_, manifest) = kedra_assets::delivery::read_directory(&root, "game-desktop")?;
    if manifest.assets.len() != 1 {
        return Err("Expected exactly the independent fixture asset".into());
    }
    println!("SUCCESS: independently packaged runtime verified its asset delivery");
    Ok(())
}
