//! Opt-in checks of the approved border-only scope on actual native frames.
use std::path::Path;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "Requires native dock captures; set KEDRA_DOCK_CAPTURE to the suite directory"]
fn rounded_tab_rims_and_flat_action_edges_match_independent_figma_pixels() -> Result<()> {
    let captures = std::env::var_os("KEDRA_DOCK_CAPTURE")
        .ok_or("Set KEDRA_DOCK_CAPTURE to the native suite directory")?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let captures = root.join(captures);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(captures.join("report.json"))?)?;
    assert_eq!(report["stability"], "passed");
    let reference = image::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/docking/figma/i00-baseline.png"),
    )?
    .to_rgba8();
    for run in ["run-1", "run-2"] {
        let actual = image::open(captures.join(run).join("i00-baseline.actual.png"))?.to_rgba8();
        assert_eq!(actual.dimensions(), reference.dimensions());
        let same = |x: u32, y: u32, tolerance: u8| {
            let expected = reference.get_pixel(x, y);
            let observed = actual.get_pixel(x, y);
            assert!(
                observed
                    .0
                    .iter()
                    .zip(expected.0)
                    .all(|(a, b)| a.abs_diff(b) <= tolerance),
                "{run}, border ({x},{y}): {observed:?}, Figma {expected:?}"
            );
        };
        for (x, y) in [(5, 37), (309, 37), (309, 507)] {
            // Rounded corners and the full straight rim, without text or content.
            for dy in 0..5 {
                for dx in 0..5 {
                    // The existing 0.05 normalized YIQ tolerance permits about
                    // 13 levels on grayscale AA edges; straight pixels stay strict.
                    same(x + dx, y + dy, 13);
                    same(x + 99 - dx, y + dy, 13);
                }
            }
            for dx in 5..95 {
                same(x + dx, y, 3);
                same(x + dx, y + 1, 3);
            }
        }
        for (x, y) in [(271, 36), (1169, 36), (1169, 506)] {
            // Flat menu hit areas are transparent at rest: no raised top stroke
            // or bevel may reappear. The 16px glyph is below these sample rows.
            for dx in 0..26 {
                same(x + dx, y, 3);
                same(x + dx, y + 1, 3);
            }
        }
    }
    Ok(())
}
