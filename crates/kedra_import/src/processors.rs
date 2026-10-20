//! Generic SVG processing shared by editor resources and game projects.
use bevy::{
    asset::{
        io::Writer,
        processor::{Process, ProcessContext, ProcessError},
    },
    image::{ImageFormat, ImageFormatSetting, ImageLoader, ImageLoaderSettings},
    prelude::*,
    tasks::futures_lite::AsyncWriteExt,
};
use kedra_assets::images::{ImageVariant, ImageVariantsDelivery, ImageVariantsLoader};
use serde::{Deserialize, Serialize};

/// Native SVG processing settings. No editor palette, paths or fixed icon size.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SvgSettings {
    /// Optional target size in logical pixels, otherwise the authored SVG size.
    pub size: Option<[u32; 2]>,
    /// Output densities, strictly increasing integers in 1..=8.
    pub densities: Vec<u32>,
}
impl Default for SvgSettings {
    fn default() -> Self {
        Self {
            size: None,
            densities: vec![1],
        }
    }
}
/// SVG to one native Bevy image. Suitable for ordinary game image assets.
#[derive(TypePath)]
pub struct SvgToImage;
/// SVG to a multi-density package with native Bevy labeled image subassets.
#[derive(TypePath)]
pub struct SvgToImageVariants;
fn failure(error: impl std::fmt::Display) -> ProcessError {
    ProcessError::AssetSaveError(std::io::Error::other(error.to_string()).into())
}
async fn source(context: &mut ProcessContext<'_>) -> Result<Vec<u8>, ProcessError> {
    let mut bytes = Vec::new();
    context
        .asset_reader()
        .read_to_end(&mut bytes)
        .await
        .map_err(failure)?;
    Ok(bytes)
}
impl Process for SvgToImage {
    type Settings = SvgSettings;
    type OutputLoader = ImageLoader;
    async fn process(
        &self,
        context: &mut ProcessContext<'_>,
        settings: &SvgSettings,
        writer: &mut Writer,
    ) -> Result<ImageLoaderSettings, ProcessError> {
        if settings.densities != [1] {
            return Err(failure(
                "Use SvgToImageVariants for multiple output densities",
            ));
        }
        let bytes = source(context).await?;
        let (_, png) = rasterize(&bytes, settings.size, 1).map_err(failure)?;
        writer.write_all(&png).await.map_err(failure)?;
        Ok(ImageLoaderSettings {
            format: ImageFormatSetting::Format(ImageFormat::Png),
            is_srgb: true,
            ..default()
        })
    }
}
impl Process for SvgToImageVariants {
    type Settings = SvgSettings;
    type OutputLoader = ImageVariantsLoader;
    async fn process(
        &self,
        context: &mut ProcessContext<'_>,
        settings: &SvgSettings,
        writer: &mut Writer,
    ) -> Result<(), ProcessError> {
        let bytes = source(context).await?;
        let delivery = variants(&bytes, settings).map_err(failure)?;
        writer
            .write_all(ron::ser::to_string(&delivery).map_err(failure)?.as_bytes())
            .await
            .map_err(failure)?;
        Ok(())
    }
}
fn variants(bytes: &[u8], settings: &SvgSettings) -> Result<ImageVariantsDelivery, String> {
    if settings.densities.is_empty()
        || settings.densities.len() > 8
        || settings.densities.windows(2).any(|v| v[0] >= v[1])
    {
        return Err("Densities must be nonempty and strictly increasing".into());
    }
    let mut logical_size = [0, 0];
    let mut variants = Vec::new();
    for &density in &settings.densities {
        let (size, png) = rasterize(bytes, settings.size, density)?;
        logical_size = size;
        variants.push(ImageVariant { density, png });
    }
    Ok(ImageVariantsDelivery {
        format: 2,
        logical_size,
        variants,
    })
}

/// Render a self-contained SVG to a straight-alpha PNG. Allocation is bounded
/// to 16 megapixels and density 1..=8. External image/font files are not loaded.
pub fn rasterize(
    bytes: &[u8],
    size: Option<[u32; 2]>,
    density: u32,
) -> Result<([u32; 2], Vec<u8>), String> {
    if !(1..=8).contains(&density) {
        return Err("SVG density must be 1..=8".into());
    }
    let options = resvg::usvg::Options {
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            ..Default::default()
        },
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_data(bytes, &options).map_err(|e| e.to_string())?;
    let native = tree.size().to_int_size();
    let logical = size.unwrap_or([native.width(), native.height()]);
    let width = logical[0]
        .checked_mul(density)
        .ok_or("SVG width overflow")?;
    let height = logical[1]
        .checked_mul(density)
        .ok_or("SVG height overflow")?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 16_777_216 {
        return Err("SVG output must contain 1..16777216 pixels".into());
    }
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or("Cannot allocate SVG image")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(
            width as f32 / tree.size().width(),
            height as f32 / tree.size().height(),
        ),
        &mut pixmap.as_mut(),
    );
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for pixel in pixmap.pixels() {
        let pixel = pixel.demultiply();
        rgba.extend([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    }
    let mut png = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(
        &mut png,
        &rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())?;
    Ok((logical, png.into_inner()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "SVG processing assertions")]
    use super::*;
    const SVG:&[u8]=br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="12"><path d="M1 1 L22 10" stroke="red" stroke-width="1.5"/></svg>"#;
    #[test]
    fn arbitrary_svg_sizes_and_density_variants_are_shared_not_editor_specific() {
        let package = variants(
            SVG,
            &SvgSettings {
                size: None,
                densities: vec![1, 2, 3, 4],
            },
        )
        .unwrap();
        assert_eq!(package.logical_size, [24, 12]);
        for variant in package.variants {
            let image = image::load_from_memory(&variant.png).unwrap().to_rgba8();
            assert_eq!(
                image.dimensions(),
                (24 * variant.density, 12 * variant.density)
            );
            assert!(image.pixels().any(|p| p.0[3] > 0 && p.0[3] < 255));
        }
        assert!(rasterize(b"invalid", None, 1).is_err());
        assert!(rasterize(SVG, Some([0, 1]), 1).is_err());
        assert!(rasterize(SVG, Some([u32::MAX, 1]), 2).is_err());
        assert!(rasterize(SVG, None, 9).is_err());
        assert!(
            variants(
                SVG,
                &SvgSettings {
                    size: None,
                    densities: vec![2, 1]
                }
            )
            .is_err()
        );
    }
}
