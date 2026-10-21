//! Public consumer contract, independent of editor and library package version.
use kedra_update::{Config, Kind, Version, detect_installation, select_release, stage};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn config(version: &str) -> Result<Config, String> {
    Config::new(
        "example/studio",
        "studio",
        "org.example.studio",
        "studio",
        "Studio.app",
        Version::parse(version).map_err(|e| e.to_string())?,
    )
}

fn fixture() -> (Vec<u8>, Vec<u8>) {
    let formats = [
        ("x86_64-pc-windows-msvc", "nsis", "exe"),
        ("x86_64-pc-windows-msvc", "zip", "zip"),
        ("x86_64-unknown-linux-gnu", "appimage", "AppImage"),
        ("x86_64-unknown-linux-gnu", "tar.gz", "tar.gz"),
        ("aarch64-apple-darwin", "dmg", "dmg"),
    ];
    let mut artifacts = Vec::new();
    let mut assets = Vec::new();
    for (target, format, extension) in formats {
        let name = format!("studio-4.0.0-{target}.{extension}");
        artifacts.push(serde_json::json!({"name":name,"format":format,"target":target,"bytes":3,"sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}));
        assets.push(serde_json::json!({"name":name,"browser_download_url":format!("https://github.com/example/studio/releases/download/v4.0.0/{name}"),"size":3,"state":"uploaded"}));
    }
    for name in ["artifact-inventory.json", "SHA256SUMS"] {
        assets.push(serde_json::json!({"name":name,"browser_download_url":format!("https://github.com/example/studio/releases/download/v4.0.0/{name}"),"size":3,"state":"uploaded"}));
    }
    let revision = "1".repeat(40);
    (serde_json::to_vec(&serde_json::json!([{"tag_name":"v4.0.0","draft":false,"prerelease":true,"html_url":"https://github.com/example/studio/releases/tag/v4.0.0","target_commitish":revision,"body":"Updates","assets":assets}])).unwrap_or_default(),
     serde_json::to_vec(&serde_json::json!({"schema":1,"application":"studio","version":"4.0.0","source_revision":revision,"artifacts":artifacts})).unwrap_or_default())
}

#[test]
fn configured_identity_and_consumer_version_control_selection() -> TestResult {
    let (listing, inventory) = fixture();
    let release = select_release(&config("3.0.0")?, &listing, &inventory, Kind::MacApp)?
        .ok_or("Missing release")?;
    assert_eq!(release.version(), &Version::new(4, 0, 0));
    assert!(select_release(&config("5.0.0")?, &listing, &inventory, Kind::MacApp)?.is_none());
    let foreign = Config::new(
        "other/studio",
        "studio",
        "org.example.studio",
        "studio",
        "Studio.app",
        Version::new(3, 0, 0),
    )?;
    assert!(select_release(&foreign, &listing, &inventory, Kind::MacApp)?.is_none());
    let foreign = Config::new(
        "example/studio",
        "other",
        "org.example.studio",
        "studio",
        "Studio.app",
        Version::new(3, 0, 0),
    )?;
    assert!(select_release(&foreign, &listing, &inventory, Kind::MacApp).is_err());
    Ok(())
}

#[test]
fn verified_package_ownership_and_mutation_checks_are_public_contracts() -> TestResult {
    let (listing, inventory) = fixture();
    let release = select_release(&config("3.0.0")?, &listing, &inventory, Kind::MacApp)?
        .ok_or("Missing release")?;
    let cache = tempfile::tempdir()?;
    let mut received = 0;
    let mut verifying = false;
    let package = stage(
        &b"abc"[..],
        &release,
        cache.path(),
        |bytes| received = bytes,
        || verifying = true,
    )?;
    let path = package.path().to_owned();
    assert_eq!(received, 3);
    assert!(verifying);
    package.reverify(&release)?;
    std::fs::write(&path, b"bad")?;
    assert!(package.reverify(&release).is_err());
    drop(package);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn configuration_rejects_url_and_path_injection() {
    for repository in [
        "../studio",
        "example/../studio",
        "https://github.com/example/studio",
        "example/studio?token=x",
    ] {
        assert!(
            Config::new(
                repository,
                "studio",
                "org.example.studio",
                "studio",
                "Studio.app",
                Version::new(3, 0, 0)
            )
            .is_err()
        );
    }
    for name in ["../studio", "a/b", "a\\b", ".", "..", ""] {
        assert!(
            Config::new(
                "example/studio",
                name,
                "org.example.studio",
                "studio",
                "Studio.app",
                Version::new(3, 0, 0)
            )
            .is_err()
        );
        assert!(
            Config::new(
                "example/studio",
                "studio",
                "org.example.studio",
                name,
                "Studio.app",
                Version::new(3, 0, 0)
            )
            .is_err()
        );
        assert!(
            Config::new(
                "example/studio",
                "studio",
                "org.example.studio",
                "studio",
                name,
                Version::new(3, 0, 0)
            )
            .is_err()
        );
    }
}

#[test]
fn consumer_version_and_identity_control_installation_markers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let executable = dir.path().join("studio.exe");
    std::fs::write(&executable, b"isolated executable")?;
    let marker = dir.path().join("studio-installation.json");
    std::fs::write(
        &marker,
        serde_json::to_vec(&serde_json::json!({
            "schema":1,"application":"studio","version":"3.0.0","kind":"Nsis","target":"x86_64-pc-windows-msvc"
        }))?,
    )?;
    assert_eq!(
        detect_installation(&config("3.0.0")?, &executable, dir.path(), Kind::Nsis, None)?,
        Some(Kind::Nsis)
    );
    let foreign_executable = dir.path().join("foreign.exe");
    std::fs::write(&foreign_executable, b"foreign executable")?;
    assert!(
        detect_installation(
            &config("3.0.0")?,
            &foreign_executable,
            dir.path(),
            Kind::Nsis,
            None
        )
        .is_err()
    );
    assert!(
        detect_installation(&config("5.0.0")?, &executable, dir.path(), Kind::Nsis, None).is_err()
    );
    std::fs::write(
        &marker,
        serde_json::to_vec(&serde_json::json!({
            "schema":1,"application":"other","version":"3.0.0","kind":"Nsis","target":"x86_64-pc-windows-msvc"
        }))?,
    )?;
    assert!(
        detect_installation(&config("3.0.0")?, &executable, dir.path(), Kind::Nsis, None).is_err()
    );
    Ok(())
}

#[test]
fn macos_destination_requires_configured_installed_bundle_identity() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path().canonicalize()?.join("Studio.app");
    let resources = root.join("Contents/Resources");
    std::fs::create_dir_all(root.join("Contents/MacOS"))?;
    std::fs::create_dir_all(&resources)?;
    let executable = root.join("Contents/MacOS/studio");
    std::fs::write(&executable, b"fixture")?;
    std::fs::write(
        resources.join("studio-installation.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema":1,"application":"studio","version":"3.0.0","kind":"MacApp","target":"aarch64-apple-darwin"
        }))?,
    )?;
    for identifier in ["org.example.studio", "org.foreign.app"] {
        std::fs::write(
            root.join("Contents/Info.plist"),
            format!(
                r#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>{identifier}</string><key>CFBundleExecutable</key><string>studio</string><key>CFBundleShortVersionString</key><string>3.0.0</string></dict></plist>"#
            ),
        )?;
        let result = detect_installation(
            &config("3.0.0")?,
            &executable,
            &resources,
            Kind::MacApp,
            None,
        );
        if identifier == "org.example.studio" {
            assert_eq!(result?, Some(Kind::MacApp));
        } else {
            assert!(result.is_err());
        }
    }
    Ok(())
}
