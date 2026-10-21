use super::*;
use serde_json::json;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn listing(version: &str) -> serde_json::Value {
    let entries = [
        ("x86_64-pc-windows-msvc", "exe"),
        ("x86_64-pc-windows-msvc", "zip"),
        ("x86_64-unknown-linux-gnu", "AppImage"),
        ("x86_64-unknown-linux-gnu", "tar.gz"),
        ("aarch64-apple-darwin", "dmg"),
    ];
    let mut assets: Vec<_> = entries.iter().map(|(target, ext)| {
        let name = format!("kedra-{version}-{target}.{ext}");
        json!({"name":name,"size":3,"state":"uploaded","browser_download_url":format!("https://github.com/kedra-engine/kedra/releases/download/v{version}/{name}")})
    }).collect();
    for name in ["artifact-inventory.json", "SHA256SUMS"] {
        assets.push(json!({"name":name,"size":1024,"state":"uploaded","browser_download_url":format!("https://github.com/kedra-engine/kedra/releases/download/v{version}/{name}")}));
    }
    json!({"tag_name":format!("v{version}"),"draft":false,"prerelease":true,
        "target_commitish":"1234567890123456789012345678901234567890",
        "html_url":format!("https://github.com/kedra-engine/kedra/releases/tag/v{version}"),
        "body":"Small improvements", "assets":assets})
}
fn inventory() -> serde_json::Value {
    let entries = [
        ("x86_64-pc-windows-msvc", "nsis", "exe"),
        ("x86_64-pc-windows-msvc", "zip", "zip"),
        ("x86_64-unknown-linux-gnu", "appimage", "AppImage"),
        ("x86_64-unknown-linux-gnu", "tar.gz", "tar.gz"),
        ("aarch64-apple-darwin", "dmg", "dmg"),
    ];
    let artifacts: Vec<_> = entries
        .iter()
        .map(|(target, format, ext)| {
            json!({
        "name":format!("kedra-0.2.0-{target}.{ext}"),"target":target,"format":format,"bytes":3,
        "sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"})
        })
        .collect();
    json!({"schema":1,"application":"kedra","version":"0.2.0",
        "source_revision":"1234567890123456789012345678901234567890", "artifacts":artifacts})
}

#[test]
fn prerelease_is_discovered_and_semver_not_api_order_selects_newest() -> TestResult {
    let items = json!([listing("0.2.0"), listing("0.10.0")]);
    let found =
        candidate(&crate::config::fixture(), &serde_json::to_vec(&items)?)?.ok_or("No update")?;
    assert_eq!(found.tag_name, "v0.10.0");
    Ok(())
}
#[test]
fn drafts_foreign_releases_and_non_newer_versions_are_ignored() -> TestResult {
    let mut draft = listing("0.3.0");
    draft["draft"] = json!(true);
    let mut foreign = listing("0.4.0");
    foreign["html_url"] = json!("https://github.com/other/repo/releases/tag/v0.4.0");
    assert!(
        candidate(
            &crate::config::fixture(),
            &serde_json::to_vec(&json!([draft, foreign, listing("0.1.0")]))?,
        )?
        .is_none()
    );
    Ok(())
}
#[test]
fn current_published_inventory_produces_an_exact_platform_payload() -> TestResult {
    let c: Candidate = serde_json::from_value(listing("0.2.0"))?;
    let release = validate(
        &crate::config::fixture(),
        &c,
        &serde_json::to_vec(&inventory())?,
        Kind::AppImage,
    )?;
    assert_eq!(release.version, Version::new(0, 2, 0));
    assert_eq!(release.bytes, 3);
    assert_eq!(
        release.name,
        "kedra-0.2.0-x86_64-unknown-linux-gnu.AppImage"
    );
    Ok(())
}
#[test]
fn malformed_wrong_target_duplicate_and_foreign_assets_are_rejected() -> TestResult {
    let c: Candidate = serde_json::from_value(listing("0.2.0"))?;
    for field in ["schema", "application", "version", "source_revision"] {
        let mut wire = inventory();
        wire[field] = json!("wrong");
        assert!(
            validate(
                &crate::config::fixture(),
                &c,
                &serde_json::to_vec(&wire)?,
                Kind::AppImage
            )
            .is_err(),
            "{field}"
        );
    }
    for field in ["target", "format", "sha256", "bytes", "name"] {
        let mut wire = inventory();
        wire["artifacts"][0][field] = json!("wrong");
        assert!(
            validate(
                &crate::config::fixture(),
                &c,
                &serde_json::to_vec(&wire)?,
                Kind::AppImage
            )
            .is_err(),
            "{field}"
        );
    }
    let mut wire = inventory();
    let entry = wire["artifacts"][0].clone();
    wire["artifacts"]
        .as_array_mut()
        .ok_or("Artifacts")?
        .push(entry);
    assert!(
        validate(
            &crate::config::fixture(),
            &c,
            &serde_json::to_vec(&wire)?,
            Kind::AppImage
        )
        .is_err()
    );
    let mut bad = c.clone();
    bad.assets[1].browser_download_url = Url::parse("https://evil.example/payload")?;
    assert!(
        validate(
            &crate::config::fixture(),
            &bad,
            &serde_json::to_vec(&inventory())?,
            Kind::AppImage
        )
        .is_err()
    );
    let mut missing = inventory();
    missing["artifacts"]
        .as_array_mut()
        .ok_or("Artifacts")?
        .pop();
    assert!(
        validate(
            &crate::config::fixture(),
            &c,
            &serde_json::to_vec(&missing)?,
            Kind::Nsis
        )
        .is_err()
    );
    Ok(())
}
