//! Independent Figma reference integrity; never regenerates expected pixels.
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn figma_docking_state_exports_are_pinned_complete_and_native_resolution() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/docking/figma");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json"))?)?;
    assert_eq!(manifest["schema"], 1);
    assert_eq!(manifest["file"], "2EZlK40cfx2Mub7GmDyI70");
    assert_eq!(manifest["revision"], "flat-icon-buttons-common-dock-menu");
    let frames = manifest["frames"]
        .as_array()
        .ok_or("Missing Figma frames")?;
    assert_eq!(frames.len(), 20);
    let mut names = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    for frame in frames {
        let name = frame["name"].as_str().ok_or("Missing frame name")?;
        assert!(names.insert(name));
        assert!(nodes.insert(frame["node"].as_str().ok_or("Missing source node")?));
        let bytes = std::fs::read(root.join(format!("{name}.png")))?;
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            frame["sha256"].as_str().ok_or("Missing hash")?,
            "Independent Figma export changed: {name}"
        );
        let image = image::load_from_memory(&bytes)?;
        assert_eq!(
            u64::from(image.width()),
            frame["size"][0].as_u64().ok_or("Missing width")?
        );
        assert_eq!(
            u64::from(image.height()),
            frame["size"][1].as_u64().ok_or("Missing height")?
        );
    }
    let policy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("../policy.json"))?)?;
    assert_eq!(policy["fixture"], "docking.fixture.v1");
    assert_eq!(policy["comparison"]["max_excluded_ppm"], 0);
    assert_eq!(policy["exclusions"], serde_json::json!([]));
    let scenarios = policy["critical_regions"]
        .as_object()
        .ok_or("Missing critical regions")?;
    assert_eq!(
        scenarios
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        names
    );
    Ok(())
}
