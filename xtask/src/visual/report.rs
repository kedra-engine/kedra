//! Passive local evidence, preserving originals and making exclusions conspicuous.
use super::{compare::Compared, config::Config};
use image::{RgbaImage, imageops};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn write_json(path: PathBuf, value: &impl Serialize) -> crate::Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn artifacts(
    output: &Path,
    config: &Config,
    reference: &RgbaImage,
    actual: &RgbaImage,
    compared: &Compared,
) -> crate::Result<()> {
    for (name, image) in [
        ("reference.png", reference),
        ("actual.png", actual),
        ("difference.png", &compared.difference),
        ("exact-difference.png", &compared.exact_difference),
        ("overlay.png", &compared.overlay),
        ("exclusions.png", &compared.exclusions),
    ] {
        image.save(output.join(name))?;
    }
    let mut pair = RgbaImage::new(reference.width() * 2, reference.height());
    imageops::replace(&mut pair, reference, 0, 0);
    imageops::replace(&mut pair, actual, i64::from(reference.width()), 0);
    pair.save(output.join("side-by-side.png"))?;
    for (index, region) in config.critical_regions.iter().enumerate() {
        let super::config::Bounds(x, y, end_x, end_y) = config.profile.resolve(region.rect)?;
        for (kind, image) in [("reference", reference), ("actual", actual)] {
            imageops::crop_imm(image, x, y, end_x - x, end_y - y)
                .to_image()
                .save(output.join(format!("region-{index}-{kind}.png")))?;
        }
    }
    let matched = compared.domains.iter().all(|d| d.matched);
    let mut sections = String::new();
    for domain in &compared.domains {
        sections.push_str(&format!(
            "<li>{}: {} / {} eligible pixels ({:.6}%), {} excluded. {}</li>",
            escape(&domain.name),
            domain.perceptual_changed_pixels,
            domain.eligible_pixels,
            domain.changed_percent,
            domain.excluded_pixels,
            if domain.matched {
                "MATCHED"
            } else {
                "MISMATCH"
            }
        ));
    }
    let mut exclusions = String::new();
    for exclusion in &config.exclusions {
        exclusions.push_str(&format!(
            "<li>{}: {}</li>",
            escape(&exclusion.name),
            escape(&exclusion.reason)
        ));
    }
    fs::write(
        output.join("index.html"),
        format!(
            r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src 'self'; style-src 'unsafe-inline'">
<title>Visual regression evidence</title><style>
body{{background:#181818;color:#eee;font:14px/1.5 system-ui;margin:24px}}
a{{color:#aecfff}} img{{display:block;max-width:none}} figure{{margin:12px 0;overflow:auto}}
</style><h1>{}</h1><p>Automated result: <strong>{}</strong>. Design acceptance: awaiting human review.</p>
<p>Baseline approval: {:?}. Single-frame comparison does not verify motion, timing or native operations.</p>
<p><a href="report.json">Metrics/provenance</a> · <a href="policy.json">Complete policy</a> ·
<a href="capture.json">Capture declaration</a></p><ul>{}</ul>
<figure><figcaption>Reference / actual, original scale</figcaption><img src="side-by-side.png" alt="Reference and actual"></figure>
<figure><figcaption>50% overlay (no alignment correction)</figcaption><img src="overlay.png" alt="Overlay"></figure>
<figure><figcaption>Perceptual differences: red marks counted mismatches</figcaption><img src="difference.png" alt="Perceptual differences"></figure>
<figure><figcaption>Exact RGB differences ×4, INCLUDING excluded pixels</figcaption><img src="exact-difference.png" alt="Exact differences"></figure>
<h2>Exclusions (magenta, original evidence remains intact)</h2><ul>{}</ul>
<img src="exclusions.png" alt="Excluded areas"><p>Verification never promotes or updates references.</p></html>"#,
            escape(&config.scenario),
            if matched { "MATCHED" } else { "MISMATCH" },
            config.reference.approval,
            sections,
            exclusions
        ),
    )?;
    Ok(())
}
