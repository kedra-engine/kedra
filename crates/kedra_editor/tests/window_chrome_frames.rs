//! Owner-requested dark outer frame, checked on a real renderer capture.
use std::path::PathBuf;

#[test]
#[ignore = "Requires a native capture; set KEDRA_CHROME_FRAME to its PNG path"]
fn native_outer_frame_has_no_light_outline_or_top_bevel() -> Result<(), Box<dyn std::error::Error>>
{
    let path =
        PathBuf::from(std::env::var_os("KEDRA_CHROME_FRAME").ok_or("Set KEDRA_CHROME_FRAME")?);
    let path = if path.is_absolute() {
        path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    };
    let frame = image::open(path)?.to_rgba8();
    let (width, height) = frame.dimensions();
    assert!(
        width >= 640 && height >= 360,
        "A full native frame is required"
    );
    for (x, y) in [
        (0, height / 2),
        (width - 1, height / 2),
        (width / 2, 0),
        (width / 2, 1),
        (width / 2, height - 1),
    ] {
        let pixel = frame.get_pixel(x, y).0;
        assert!(
            pixel[..3].iter().all(|channel| *channel <= 64),
            "Unexpected light frame pixel at ({x},{y}): {pixel:?}"
        );
    }
    Ok(())
}
