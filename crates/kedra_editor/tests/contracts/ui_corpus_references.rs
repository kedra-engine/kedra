//! Integrity of the only active screenshot scope: kit buttons and text inputs.
use sha2::{Digest, Sha256};
use std::path::Path;

#[test]
fn kit_button_and_input_exports_are_pinned_complete_and_native_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/corpus/text");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("text-manifest.json"))?)?;
    assert_eq!(manifest["schema"], 1);
    let cases = manifest["cases"].as_object().ok_or("Missing kit cases")?;
    assert_eq!(cases.len(), 22);
    for (name, case) in cases {
        assert!(
            name.starts_with("button-") || name.starts_with("field-"),
            "Out-of-scope screenshot case: {name}"
        );
        for layer in ["metadata", "full", "shape"] {
            let file = case[layer]["file"].as_str().ok_or("Missing export file")?;
            assert_eq!(Path::new(file).components().count(), 1);
            let bytes = std::fs::read(root.join(file))?;
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                case[layer]["sha256"]
                    .as_str()
                    .ok_or("Missing export hash")?,
                "{name}/{layer}"
            );
            if layer != "metadata" {
                let image = image::load_from_memory(&bytes)?;
                assert_eq!((image.width(), image.height()), (640, 360));
            }
        }
    }
    Ok(())
}
