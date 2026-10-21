//! Native icon conversion against actual maintained packager output.
use super::*;

#[test]
fn generated_icons_produce_a_real_icns_bundle_without_native_sdk_tools()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("icon.svg");
    fs::write(
        &source,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="purple"/></svg>"#,
    )?;
    let icons = crate::icons::generate(&source, &directory.path().join("icons"))?;
    fs::write(
        directory.path().join("probe"),
        "non-native fixture executable",
    )?;
    let mut config: cargo_packager::Config = serde_json::from_value(serde_json::json!({
        "name": "icon-probe", "productName": "Icon Probe", "version": "1.2.3",
        "identifier": "dev.kedra.icon-probe", "binaries": [{"path": "probe", "main": true}]
    }))?;
    config.binaries_dir = Some(directory.path().to_path_buf());
    config.out_dir = directory.path().join("packages");
    config.target_triple = Some("aarch64-apple-darwin".into());
    config.formats = Some(vec![PackageFormat::App]);
    config.icons = Some(native_icons(NativeTarget::MacArm, icons)?);
    cargo_packager::package(&config)?;
    let resources = config.out_dir.join("Icon Probe.app/Contents/Resources");
    let files = fs::read_dir(resources)?.collect::<Result<Vec<_>, _>>()?;
    let icon = files
        .iter()
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "icns"))
        .ok_or("Native ICNS icon missing")?;
    let family = tauri_icns::IconFamily::read(fs::File::open(icon.path())?)?;
    assert!(
        family
            .available_icons()
            .contains(&tauri_icns::IconType::RGBA32_512x512_2x)
    );
    assert!(
        family
            .available_icons()
            .contains(&tauri_icns::IconType::RGBA32_16x16_2x)
    );
    Ok(())
}
