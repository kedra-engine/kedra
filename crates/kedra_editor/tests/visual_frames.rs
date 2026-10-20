//! Opt-in verification of actual native frames against independent Figma samples.
//! No implementation-generated baseline or automatic reference update exists.
#![allow(clippy::unwrap_used, reason = "Explicit native acceptance assertions")]
use serde_json::Value;
use std::path::Path;

#[test]
#[ignore = "Requires actual renderer captures: run tools/qa/startup.py first"]
fn native_frame_dimensions_and_figma_colors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = std::env::var_os("KEDRA_QA_REPORT")
        .unwrap_or_else(|| panic!("Set KEDRA_QA_REPORT to the native QA JSON report"));
    let report: Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(report["status"], "passed");
    let runs = report["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2, "Both DPR runs must be present");
    assert_eq!(
        runs.iter()
            .map(|run| run["dpr"].as_u64().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([1, 2])
    );
    let reference: Value = serde_json::from_slice(
        &std::fs::read(root.join("docs/ui/startup-reference.json")).unwrap(),
    )
    .unwrap();
    for run in report["runs"].as_array().unwrap() {
        let dpr = run["dpr"].as_u64().unwrap() as u32;
        for (state, samples) in reference["pixels"].as_object().unwrap() {
            let frame = &run["frames"][state];
            let path = root.join(frame["path"].as_str().unwrap());
            let image = image::open(&path).unwrap().to_rgba8();
            assert_eq!(
                image.dimensions(),
                (1120 * dpr, 720 * dpr),
                "{}",
                path.display()
            );
            for sample in samples.as_array().unwrap() {
                let values: Vec<_> = sample
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap() as u32)
                    .collect();
                let actual = image.get_pixel(values[0] * dpr, values[1] * dpr);
                for channel in 0..3 {
                    assert!(
                        actual[channel].abs_diff(values[channel + 2] as u8) <= 2,
                        "{state} DPR {dpr}, sample {sample}, observed {actual:?}, file {}",
                        path.display()
                    );
                }
                assert_eq!(actual[3], 255);
            }
        }
    }
}
