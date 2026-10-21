//! Native icon conversion against actual maintained packager output.
use super::*;

#[test]
fn installation_identity_is_native_only_and_does_not_mark_other_applications()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    assert!(
        installation_marker(
            directory.path(),
            "independent-app",
            "0.1.0",
            NativeTarget::Linux
        )?
        .is_none()
    );
    let resource = installation_marker(directory.path(), "kedra", "0.1.0", NativeTarget::Linux)?
        .ok_or("Marker missing")?;
    let Resource::Mapped { src, target } = resource else {
        return Err("Mapped marker expected".into());
    };
    assert_eq!(target, PathBuf::from("kedra-installation.json"));
    let data: serde_json::Value = serde_json::from_slice(&fs::read(src)?)?;
    assert_eq!(data["kind"], "AppImage");
    assert_eq!(data["target"], "x86_64-unknown-linux-gnu");
    assert_eq!(data["version"], "0.1.0");
    assert!(
        !directory
            .path()
            .join("assets/kedra-installation.json")
            .exists()
    );
    Ok(())
}

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

#[cfg(target_os = "macos")]
#[test]
fn native_macos_bundle_signature_seals_resources_and_preserves_explicit_identity()
-> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::create_dir(root.path().join("assets"))?;
    fs::write(root.path().join("src/main.rs"), "fn main() {}\n")?;
    let manifest = root.path().join("Cargo.toml");
    let metadata = r#"[package]
name = "signature-probe"
version = "1.2.3"
edition = "2024"
[workspace]
[package.metadata.packager]
productName = "Signature Probe"
identifier = "dev.kedra.signature-probe"
[package.metadata.kedra.distribution]
assets = "assets"
profile = "editor-desktop"
namespace = "probe"
icon = "icon.svg"
notices = ["NOTICE.txt"]
"#;
    fs::write(&manifest, metadata)?;
    fs::write(root.path().join("NOTICE.txt"), "Original resource\n")?;
    fs::write(
        root.path().join("icon.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="purple"/></svg>"#,
    )?;
    let application = Application::load(&manifest, None, None)?;
    let target = application.target;
    let binary = root.path().join(&application.binary);
    fs::write(
        root.path().join("probe.c"),
        "int main(void) { return 0; }\n",
    )?;
    assert!(
        Command::new("cc")
            .arg(root.path().join("probe.c"))
            .arg("-o")
            .arg(&binary)
            .status()?
            .success()
    );
    let icons = crate::icons::generate(&root.path().join("icon.svg"), &root.path().join("icons"))?;
    let mut config = application.native;
    config.binaries = vec![Binary::new("signature-probe").main(true)];
    config.binaries_dir = Some(root.path().to_path_buf());
    config.out_dir = root.path().join("packages");
    config.target_triple = Some(target.triple().into());
    config.formats = Some(vec![PackageFormat::App]);
    config.icons = Some(native_icons(target, icons)?);
    config.resources = Some(vec![Resource::Mapped {
        src: root
            .path()
            .join("NOTICE.txt")
            .to_string_lossy()
            .into_owned(),
        target: PathBuf::from("NOTICE.txt"),
    }]);
    cargo_packager::package(&config)?;
    let app = config.out_dir.join("Signature Probe.app");
    let verify = |path: &Path| {
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict", "--verbose=2"])
            .arg(path)
            .output()
    };
    let valid = verify(&app)?;
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    assert_eq!(
        config
            .macos
            .as_ref()
            .and_then(|m| m.signing_identity.as_deref()),
        Some("-")
    );
    assert!(
        Command::new(app.join("Contents/MacOS/signature-probe"))
            .status()?
            .success()
    );
    fs::write(
        app.join("Contents/Resources/NOTICE.txt"),
        "Changed resource\n",
    )?;
    assert!(
        !verify(&app)?.status.success(),
        "Changing resources must invalidate the signature"
    );
    fs::write(
        &manifest,
        format!(
            "{metadata}\n[package.metadata.packager.macos]\nsigningIdentity = \"Developer ID Application: Explicit\"\n"
        ),
    )?;
    let explicit = Application::load(&manifest, None, None)?;
    assert_eq!(
        explicit
            .native
            .macos
            .as_ref()
            .and_then(|m| m.signing_identity.as_deref()),
        Some("Developer ID Application: Explicit")
    );
    Ok(())
}
