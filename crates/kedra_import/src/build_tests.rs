#![allow(
    clippy::unwrap_used,
    reason = "Exact manifest serialization boundary assertions"
)]
use super::*;

#[test]
fn exact_serialized_manifest_boundary_is_shared_with_the_native_reader() {
    let mut manifest = Manifest {
        format: 1,
        profile: String::new(),
        assets: Vec::new(),
        files: BTreeMap::new(),
    };
    let overhead = serde_json::to_vec(&manifest).unwrap().len();
    manifest.profile = "p".repeat(MAX_MANIFEST_BYTES - overhead);
    let bytes = encode_manifest(&manifest).unwrap();
    assert_eq!(bytes.len(), MAX_MANIFEST_BYTES);
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join(MANIFEST), bytes).unwrap();
    assert!(kedra_assets::delivery::read_directory(directory.path(), &manifest.profile).is_ok());
    manifest.profile.push('p');
    assert!(encode_manifest(&manifest).unwrap_err().contains("exceeds"));
    fs::write(
        directory.path().join(MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(kedra_assets::delivery::read_directory(directory.path(), &manifest.profile).is_err());
}
