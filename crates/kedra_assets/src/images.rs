//! Multi-density image delivery shared by editor and game runtimes.
use bevy::{
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
};
use serde::{Deserialize, Serialize};

/// Processed icon package, serialized by RON rather than a bespoke file parser.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageVariantsDelivery {
    /// Contract version: 1 used numeric byte arrays; 2 writes compact Base64 PNGs.
    pub format: u32,
    /// Logical width and height, independent of monitor DPI.
    pub logical_size: [u32; 2],
    /// PNG variants, one per authored density.
    pub variants: Vec<ImageVariant>,
}
/// One straight-alpha sRGBA PNG at an integer pixel density.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageVariant {
    /// Physical pixels per logical pixel.
    pub density: u32,
    /// Lossless encoded PNG, decoded by Bevy at runtime. New deliveries use
    /// Base64; legacy numeric arrays remain readable without changing the bytes.
    #[serde(with = "serde_with::As::<PngBytes>")]
    pub png: Vec<u8>,
}
type PngBytes = serde_with::PickFirst<(serde_with::base64::Base64, Vec<serde_with::Same>)>;
/// Runtime dependency owner for all labeled images in a processed icon.
#[derive(Asset, TypePath)]
pub struct ImageVariants {
    /// Native image subassets labeled `1x`, `2x`, etc.
    #[dependency]
    pub images: Vec<Handle<Image>>,
}
/// Loads processed packages, never authoring SVG sources.
#[derive(Default, TypePath)]
pub struct ImageVariantsLoader;
impl AssetLoader for ImageVariantsLoader {
    type Asset = ImageVariants;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<ImageVariants, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let delivery: ImageVariantsDelivery =
            ron::de::from_bytes(&bytes).map_err(std::io::Error::other)?;
        if !matches!(delivery.format, 1 | 2)
            || delivery.logical_size.contains(&0)
            || delivery.variants.is_empty()
            || delivery.variants.len() > 8
        {
            return Err(std::io::Error::other("Unsupported image variants delivery"));
        }
        let mut images = Vec::new();
        let mut previous = 0;
        for variant in delivery.variants {
            if variant.density <= previous || variant.density > 8 {
                return Err(std::io::Error::other("Invalid image densities"));
            }
            previous = variant.density;
            let image = Image::from_buffer(
                &variant.png,
                ImageType::Format(bevy::image::ImageFormat::Png),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::default(),
                RenderAssetUsages::default(),
            )
            .map_err(std::io::Error::other)?;
            let width = delivery.logical_size[0]
                .checked_mul(variant.density)
                .ok_or_else(|| std::io::Error::other("Image width overflow"))?;
            let height = delivery.logical_size[1]
                .checked_mul(variant.density)
                .ok_or_else(|| std::io::Error::other("Image height overflow"))?;
            if image.width() != width || image.height() != height {
                return Err(std::io::Error::other(
                    "Image density dimensions do not match metadata",
                ));
            }
            images.push(context.add_labeled_asset(format!("{}x", variant.density), image));
        }
        Ok(ImageVariants { images })
    }
    fn extensions(&self) -> &[&str] {
        &["kedra-images"]
    }
}
