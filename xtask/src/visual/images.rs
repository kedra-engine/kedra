//! Bounded lossless decoding and unchanged source retention; no resampling or alignment.
use super::config::MAX_PIXELS;
use image::{DynamicImage, ImageDecoder, Limits, RgbaImage, codecs::png::PngDecoder};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};

pub(super) const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn read_bounded(path: &Path, max: u64) -> crate::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err("input exceeds byte limit".into());
    }
    Ok(bytes)
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn resolve_image(manifest: &Path, image: &Path) -> crate::Result<PathBuf> {
    if image.as_os_str().is_empty()
        || image
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || image.extension().and_then(|s| s.to_str()) != Some("png")
    {
        return Err("image must be a relative .png path without traversal".into());
    }
    let base = manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let resolved = base.join(image).canonicalize()?;
    if !resolved.starts_with(&base) {
        return Err("image symlink escapes manifest directory".into());
    }
    Ok(resolved)
}

pub(super) fn decode(bytes: &[u8], size: [u32; 2]) -> crate::Result<RgbaImage> {
    // Use the existing PNG parser, not a home-grown chunk walker. image's PNG
    // adapter does not expose cICP/HDR/chromaticity metadata.
    let mut metadata_decoder = png::Decoder::new(Cursor::new(bytes));
    metadata_decoder.set_limits(png::Limits {
        bytes: 64 * 1024 * 1024,
    });
    let reader = metadata_decoder.read_info()?;
    let info = reader.info();
    if info.coding_independent_code_points.is_some()
        || info.mastering_display_color_volume.is_some()
        || info.content_light_level.is_some()
        || (info.chrm_chunk.is_some() && info.srgb.is_none())
    {
        return Err(
            "HDR/cICP or unqualified chromaticities are unsupported; no implicit color conversion"
                .into(),
        );
    }
    drop(reader);
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let mut decoder = PngDecoder::with_limits(Cursor::new(bytes), limits)?;
    let (width, height) = decoder.dimensions();
    if [width, height] != size || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("PNG dimensions differ from profile; resizing is forbidden".into());
    }
    if decoder.is_apng()?
        || decoder.icc_profile()?.is_some()
        || decoder.original_color_type().channel_count() == 0
        || decoder.color_type().bits_per_pixel() / u16::from(decoder.color_type().channel_count())
            != 8
    {
        return Err("only static 8-bit SDR PNG without ICC is supported".into());
    }
    if decoder
        .gamma_value()?
        .is_some_and(|g| (g - 0.45455).abs() > 0.00001)
    {
        return Err("PNG gamma differs from declared sRGB assumption".into());
    }
    Ok(DynamicImage::from_decoder(decoder)?.to_rgba8())
}

pub(super) fn validate_content(image: &RgbaImage, allow_uniform: bool) -> crate::Result<()> {
    if image.pixels().any(|p| p[3] != 255) {
        return Err("unexpected transparent/nonopaque client pixels; capture is invalid".into());
    }
    if !allow_uniform && image.pixels().all(|p| p == image.get_pixel(0, 0)) {
        return Err("unexpected uniform frame; capture is invalid".into());
    }
    Ok(())
}
