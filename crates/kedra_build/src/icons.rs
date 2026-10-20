//! Reuse the shared SVG renderer and maintained PNG/ICO/native bundle encoders.
use image::codecs::ico::{IcoEncoder, IcoFrame};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn generate(source: &Path, output: &Path) -> Result<Vec<PathBuf>, String> {
    let bytes = fs::read(source).map_err(|e| format!("Cannot read application icon: {e}"))?;
    fs::create_dir(output).map_err(|e| e.to_string())?;
    let mut pngs = Vec::new();
    let mut frames = Vec::new();
    let mut family = tauri_icns::IconFamily::new();
    for size in [16, 32, 48, 64, 128, 256, 512, 1024] {
        let (_, png) = kedra_import::processors::rasterize(&bytes, Some([size, size]), 1)?;
        let path = output.join(format!("icon-{size}.png"));
        fs::write(&path, &png).map_err(|e| e.to_string())?;
        let native = tauri_icns::Image::read_png(png.as_slice()).map_err(|e| e.to_string())?;
        for density in [1, 2] {
            if let Some(kind) =
                tauri_icns::IconType::from_pixel_size_and_density(size, size, density)
            {
                family
                    .add_icon_with_type(&native, kind)
                    .map_err(|e| e.to_string())?;
            }
        }
        if size <= 256 {
            frames.push(
                IcoFrame::with_encoded(png, size, size, image::ExtendedColorType::Rgba8)
                    .map_err(|e| e.to_string())?,
            );
        }
        pngs.push(path);
    }
    let file = fs::File::create(output.join("application.ico")).map_err(|e| e.to_string())?;
    IcoEncoder::new(file)
        .encode_images(&frames)
        .map_err(|e| format!("Cannot encode application icon: {e}"))?;
    family
        .write(fs::File::create(output.join("application.icns")).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot encode native ICNS family: {e}"))?;
    Ok(pngs)
}
