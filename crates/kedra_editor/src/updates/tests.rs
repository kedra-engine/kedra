//! Filesystem-backed adapter regressions; no live network or installed application.
use super::*;
use kedra_update::MacInstallation;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn release() -> Result<Release, String> {
    let mut assets = Vec::new();
    let mut artifacts = Vec::new();
    for (target, format, ext) in [
        ("x86_64-pc-windows-msvc", "nsis", "exe"),
        ("x86_64-pc-windows-msvc", "zip", "zip"),
        ("x86_64-unknown-linux-gnu", "appimage", "AppImage"),
        ("x86_64-unknown-linux-gnu", "tar.gz", "tar.gz"),
        ("aarch64-apple-darwin", "dmg", "dmg"),
    ] {
        let name = format!("kedra-99.0.0-{target}.{ext}");
        artifacts.push(serde_json::json!({"name":name,"format":format,"target":target,"bytes":3,"sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}));
        assets.push(serde_json::json!({"name":name,"browser_download_url":format!("https://github.com/kedra-engine/kedra/releases/download/v99.0.0/{name}"),"size":3,"state":"uploaded"}));
    }
    for name in ["artifact-inventory.json", "SHA256SUMS"] {
        assets.push(serde_json::json!({"name":name,"browser_download_url":format!("https://github.com/kedra-engine/kedra/releases/download/v99.0.0/{name}"),"size":3,"state":"uploaded"}));
    }
    let revision = "1".repeat(40);
    let listing = serde_json::to_vec(&serde_json::json!([{"tag_name":"v99.0.0","draft":false,"prerelease":true,"html_url":"https://github.com/kedra-engine/kedra/releases/tag/v99.0.0","target_commitish":revision,"assets":assets}])).map_err(|e| e.to_string())?;
    let inventory = serde_json::to_vec(&serde_json::json!({"schema":1,"application":"kedra","version":"99.0.0","source_revision":revision,"artifacts":artifacts})).map_err(|e| e.to_string())?;
    backend::select_release(&host::config()?, &listing, &inventory, Kind::MacApp)?
        .ok_or("Fixture release missing".into())
}

#[test]
fn removed_or_mutated_ready_packages_allow_fresh_download_on_both_paths() -> TestResult {
    for automatic in [false, true] {
        for remove in [false, true] {
            let dir = tempfile::tempdir()?;
            let release = release()?;
            let package = backend::stage(&b"abc"[..], &release, dir.path(), |_| {}, || {})?;
            if remove {
                std::fs::remove_file(package.path())?;
            } else {
                std::fs::write(package.path(), b"bad")?;
            }
            let macos = if automatic {
                let root = dir.path().join("Kedra.app");
                std::fs::create_dir_all(root.join("Contents/MacOS"))?;
                std::fs::create_dir_all(root.join("Contents/Resources/assets"))?;
                std::fs::create_dir_all(root.join("Contents/Resources/licenses"))?;
                std::fs::write(root.join("Contents/MacOS/kedra"), b"fixture")?;
                std::fs::write(
                    root.join("Contents/Info.plist"),
                    format!(
                        r#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>dev.kedra.editor</string><key>CFBundleExecutable</key><string>kedra</string><key>CFBundleShortVersionString</key><string>{}</string></dict></plist>"#,
                        env!("CARGO_PKG_VERSION")
                    ),
                )?;
                Some(MacInstallation::open(
                    &host::config()?,
                    root.canonicalize()?,
                )?)
            } else {
                None
            };
            let mut app = crate::workspace::browser_test_app()?;
            app.init_resource::<crate::model::ProjectManager>()
                .add_message::<AppExit>();
            app.insert_resource(Updates {
                model: Model {
                    phase: Phase::Ready,
                    ..Default::default()
                },
                release: Some(release),
                package: Some(package),
                installation: Some(host::Installation {
                    kind: if automatic { Kind::MacApp } else { Kind::Nsis },
                    macos,
                }),
                ..Default::default()
            });
            activated(app.world_mut());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while app.world().resource::<Updates>().task.is_some() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "Worker did not complete"
                );
                poll(app.world_mut());
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let mut updates = app.world_mut().resource_mut::<Updates>();
            assert_eq!(
                updates.model.phase,
                Phase::Error,
                "automatic={automatic}, remove={remove}"
            );
            assert!(updates.package.is_none());
            assert!(matches!(
                updates.model.activate(),
                Some(Effect::Download(_))
            ));
            assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        }
    }
    Ok(())
}
