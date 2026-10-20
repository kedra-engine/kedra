//! Dify adapter plus exact diagnostics, eligible-area budgets and critical regions.
use super::config::{Bounds, Budget, Config};
use image::{Rgba, RgbaImage, imageops};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct Domain {
    pub name: String,
    pub bounds_physical: Bounds,
    pub eligible_pixels: u64,
    pub excluded_pixels: u64,
    pub exact_changed_pixels: u64,
    pub perceptual_changed_pixels: u64,
    pub changed_percent: f64,
    pub budget: Budget,
    pub matched: bool,
}

pub(super) struct Compared {
    pub domains: Vec<Domain>,
    pub difference: RgbaImage,
    pub exact_difference: RgbaImage,
    pub overlay: RgbaImage,
    pub exclusions: RgbaImage,
}

/// Dify's memory API takes squared YIQ distance, unlike its normalized CLI API.
pub(super) fn threshold(normalized: f32) -> f32 {
    35215. * normalized * normalized
}

fn diff(left: RgbaImage, right: RgbaImage, tolerance: f32) -> crate::Result<(u64, RgbaImage)> {
    let size = left.dimensions();
    if size != right.dimensions() || size.0 == 0 || size.1 == 0 {
        return Err("comparator requires equal nonempty dimensions".into());
    }
    // No AA suppression: small glyphs/borders are meaningful. No masks here;
    // excluded pixels have already been replaced in the COMPARISON COPY only.
    match dify::diff::get_results(
        left,
        right,
        threshold(tolerance),
        false,
        Some(0.2),
        &None,
        &None,
    ) {
        Some((count, image)) if count >= 0 => Ok((count as u64, image)),
        Some(_) => Err("comparator returned a negative mismatch count".into()),
        None => Ok((0, RgbaImage::from_pixel(size.0, size.1, Rgba([255; 4])))),
    }
}

fn domain(
    name: &str,
    bounds: Bounds,
    budget: &Budget,
    left: &RgbaImage,
    right: &RgbaImage,
    mask: &[bool],
    tolerance: f32,
) -> crate::Result<(Domain, RgbaImage)> {
    let Bounds(x, y, end_x, end_y) = bounds;
    let a = imageops::crop_imm(left, x, y, end_x - x, end_y - y).to_image();
    let mut b = imageops::crop_imm(right, x, y, end_x - x, end_y - y).to_image();
    let mut excluded = 0;
    let mut exact = 0;
    for (cx, cy, pixel) in b.enumerate_pixels_mut() {
        let original = a.get_pixel(cx, cy);
        if mask[((cy + y) * left.width() + cx + x) as usize] {
            *pixel = *original;
            excluded += 1;
        } else if pixel != original {
            exact += 1;
        }
    }
    let eligible = u64::from(b.width()) * u64::from(b.height()) - excluded;
    if eligible == 0 {
        return Err("comparison domain has no eligible pixels".into());
    }
    let (changed, difference) = diff(a, b, tolerance)?;
    Ok((
        Domain {
            name: name.to_owned(),
            bounds_physical: bounds,
            eligible_pixels: eligible,
            excluded_pixels: excluded,
            exact_changed_pixels: exact,
            perceptual_changed_pixels: changed,
            changed_percent: changed as f64 * 100. / eligible as f64,
            budget: budget.clone(),
            matched: budget.passes(changed, eligible),
        },
        difference,
    ))
}

pub(super) fn compare(
    config: &Config,
    left: &RgbaImage,
    right: &RgbaImage,
) -> crate::Result<Compared> {
    let (width, height) = left.dimensions();
    if left.dimensions() != right.dimensions() {
        return Err("image dimensions differ".into());
    }
    let mut mask = vec![false; width as usize * height as usize];
    let mut exclusions = right.clone();
    for exclusion in &config.exclusions {
        let Bounds(x, y, end_x, end_y) = config.profile.resolve(exclusion.rect)?;
        for cy in y..end_y {
            for cx in x..end_x {
                mask[(cy * width + cx) as usize] = true;
                exclusions.put_pixel(cx, cy, Rgba([255, 0, 255, 255]));
            }
        }
    }
    let excluded = mask.iter().filter(|b| **b).count() as u64;
    let area = u64::from(width) * u64::from(height);
    if excluded == area
        || excluded * 1_000_000 > area * u64::from(config.comparison.max_excluded_ppm)
    {
        return Err("excluded union area exceeds configured cap or removes all evidence".into());
    }
    let (global, difference) = domain(
        "whole_frame",
        Bounds(0, 0, width, height),
        &config.comparison.budget,
        left,
        right,
        &mask,
        config.comparison.normalized_yiq_tolerance,
    )?;
    let mut domains = vec![global];
    for region in &config.critical_regions {
        domains.push(
            domain(
                &region.name,
                config.profile.resolve(region.rect)?,
                &region.budget,
                left,
                right,
                &mask,
                config.comparison.normalized_yiq_tolerance,
            )?
            .0,
        );
    }
    // These diagnostics intentionally show excluded pixels too. Masks never erase
    // original evidence, and the exact image is not interpreted as a count mask.
    let exact_difference = RgbaImage::from_fn(width, height, |x, y| {
        let a = left.get_pixel(x, y);
        let b = right.get_pixel(x, y);
        Rgba([
            a[0].abs_diff(b[0]).saturating_mul(4),
            a[1].abs_diff(b[1]).saturating_mul(4),
            a[2].abs_diff(b[2]).saturating_mul(4),
            255,
        ])
    });
    let overlay = RgbaImage::from_fn(width, height, |x, y| {
        let a = left.get_pixel(x, y);
        let b = right.get_pixel(x, y);
        Rgba([
            ((u16::from(a[0]) + u16::from(b[0])) / 2) as u8,
            ((u16::from(a[1]) + u16::from(b[1])) / 2) as u8,
            ((u16::from(a[2]) + u16::from(b[2])) / 2) as u8,
            255,
        ])
    });
    Ok(Compared {
        domains,
        difference,
        exact_difference,
        overlay,
        exclusions,
    })
}
