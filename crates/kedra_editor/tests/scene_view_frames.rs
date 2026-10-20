//! Opt-in content assertions on actual native Scene View renderer captures.
use serde_json::Value;

#[path = "../examples/scene_view_fixture/model.rs"]
mod fixture_model;

#[test]
fn generated_glb_has_two_opaque_unlit_materials_and_native_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = fixture_model::generate()?;
    kedra_assets::model::validate_glb(&bytes)?;
    let gltf = bevy::gltf::gltf::Gltf::from_slice(&bytes)?;
    assert_eq!(gltf.scenes().len(), 1);
    assert_eq!(gltf.nodes().len(), 2);
    assert_eq!(gltf.meshes().len(), 2);
    let materials: Vec<_> = gltf.materials().collect();
    assert_eq!(materials.len(), 2);
    for (material, expected) in materials.iter().zip([[0., 1., 1., 1.], [1., 1., 0., 1.]]) {
        assert!(material.unlit());
        assert_eq!(
            material.alpha_mode(),
            bevy::gltf::gltf::material::AlphaMode::Opaque
        );
        assert_eq!(
            material.pbr_metallic_roughness().base_color_factor(),
            expected
        );
    }
    let blob = gltf.blob.as_deref().ok_or("Missing native mesh buffer")?;
    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|_| Some(blob));
            assert_eq!(
                reader.read_positions().ok_or("Missing positions")?.count(),
                24
            );
            assert_eq!(reader.read_normals().ok_or("Missing normals")?.count(), 24);
            assert_eq!(
                reader
                    .read_indices()
                    .ok_or("Missing indices")?
                    .into_u32()
                    .count(),
                36
            );
        }
    }
    Ok(())
}

fn model_visible(image: &image::RgbaImage) -> bool {
    // Independent evidence for both generated unlit materials. Reinhard/display
    // conversion and antialiasing can alter intensity, so require saturated color
    // families rather than exact bytes. Neutral grid, RGB axes, white UI, blue
    // checker and the warm scene sphere cannot supply either required pair.
    let mut cyan = 0;
    let mut yellow = 0;
    for pixel in image.pixels() {
        let [r, g, b, _] = pixel.0;
        cyan += usize::from(r < 100 && g > 140 && b > 140);
        yellow += usize::from(r > 140 && g > 140 && b < 100);
    }
    cyan > 50 && yellow > 50
}

fn synthetic_frame(grid: bool, model: bool, other_content: bool) -> image::RgbaImage {
    image::RgbaImage::from_fn(160, 120, |x, y| {
        let rgb = if model && (40..80).contains(&x) && (30..90).contains(&y) {
            [0, 188, 188]
        } else if model && (80..120).contains(&x) && (30..90).contains(&y) {
            [188, 188, 0]
        } else if other_content && y < 20 {
            if x < 80 {
                [55, 100, 160]
            } else {
                [240, 240, 240]
            }
        } else if other_content && (100..140).contains(&x) && (70..100).contains(&y) {
            [220, 100, 30]
        } else if grid && (x % 16 == 0 || y % 16 == 0) {
            [160, 160, 160]
        } else {
            [36, 43, 52]
        };
        image::Rgba([rgb[0], rgb[1], rgb[2], 255])
    })
}

#[test]
fn clear_grid_and_hidden_model_frames_fail_independent_model_evidence() {
    for (name, frame) in [
        ("clear", synthetic_frame(false, false, false)),
        ("grid-only", synthetic_frame(true, false, false)),
        (
            "model-hidden with UI/image content",
            synthetic_frame(true, false, true),
        ),
    ] {
        assert!(!model_visible(&frame), "Accepted {name} without a model");
    }
}

#[test]
fn known_model_colors_pass_with_and_without_grid() {
    for grid in [false, true] {
        assert!(model_visible(&synthetic_frame(grid, true, false)));
    }
}

#[test]
fn either_missing_material_and_uniform_frames_fail() {
    for missing in [[0, 188, 188, 255], [188, 188, 0, 255]] {
        let mut image = synthetic_frame(true, true, false);
        for p in image.pixels_mut() {
            if p.0 == missing {
                *p = image::Rgba([36, 43, 52, 255]);
            }
        }
        assert!(!model_visible(&image));
        assert!(!model_visible(&image::RgbaImage::from_pixel(
            160,
            120,
            image::Rgba(missing)
        )));
    }
}

#[test]
fn old_raw_glb_predicate_accepts_a_grid_without_the_model() {
    let grid = synthetic_frame(true, false, false);
    let foreground = grid
        .pixels()
        .filter(|p| p[0] > 80 || p[1] > 90 || p[2] > 120)
        .count();
    let magenta = grid
        .pixels()
        .filter(|p| p[0] > 200 && p[1] < 60 && p[2] > 200)
        .count();
    assert!(foreground > 100 && magenta == 0);
}

#[test]
fn hidden_model_is_rejected_even_when_all_other_content_checks_pass() {
    let image = synthetic_frame(true, false, true);
    let blue = image
        .pixels()
        .filter(|p| p[2] > p[1].saturating_add(12) && p[1] > p[0].saturating_add(10))
        .count();
    let warm = image
        .pixels()
        .filter(|p| p[0] > 120 && p[0] > p[1].saturating_add(35) && p[1] > p[2].saturating_add(15))
        .count();
    let white = image
        .pixels()
        .filter(|p| p[0] > 180 && p[1] > 180 && p[2] > 180)
        .count();
    assert!(blue > 50 && warm > 50 && white > 50);
    assert!(!model_visible(&image));
}

#[test]
#[ignore = "Requires native Scene View captures; set KEDRA_SCENE_REPORT"]
fn native_targets_contain_the_requested_assets_not_a_clear_or_error_frame()
-> Result<(), Box<dyn std::error::Error>> {
    let report = std::env::var_os("KEDRA_SCENE_REPORT").ok_or("Set KEDRA_SCENE_REPORT")?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report: Value = serde_json::from_slice(&std::fs::read(root.join(report))?)?;
    assert_eq!(report["status"], "passed");
    let frames = report["frames"].as_array().ok_or("Missing frames")?;
    assert!(frames.len() >= 6);
    for required in [
        "checker.png",
        "ship.glb",
        "Hud.prefab.ron",
        "Composed.scene.ron",
        "mixed-float",
        "mixed-restored",
    ] {
        assert!(
            frames.iter().any(|frame| frame["case"] == required),
            "Missing native case: {required}"
        );
    }
    for frame in frames {
        let path = frame["frame"]["path"]
            .as_str()
            .ok_or("Missing image path")?;
        let image = image::open(root.join(path))?.to_rgba8();
        let rect = frame["rect"].as_array().ok_or("Missing rectangle")?;
        let dpr = frame["dpr"].as_f64().ok_or("Missing DPR")?;
        let rect = rect
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|n| (n * dpr).round() as u32)
                    .ok_or("Bad rectangle")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let case = frame["case"].as_str().ok_or("Missing case")?;
        assert_eq!(rect.len(), 4, "Bad rectangle for {case}");
        assert!(rect[0] < rect[2] && rect[1] < rect[3]);
        assert!(rect[2] <= image.width() && rect[3] <= image.height());
        let crop = image::imageops::crop_imm(
            &image,
            rect[0],
            rect[1],
            rect[2] - rect[0],
            rect[3] - rect[1],
        )
        .to_image();
        assert!(
            crop.pixels().any(|p| p != crop.get_pixel(0, 0)),
            "Uniform frame for {case}: {path}"
        );
        let (mut blue, mut warm, mut white, mut foreground, mut magenta) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        for y in rect[1]..rect[3].min(image.height()) {
            for x in rect[0]..rect[2].min(image.width()) {
                let [r, g, b, _] = image.get_pixel(x, y).0;
                blue += usize::from(b > g.saturating_add(12) && g > r.saturating_add(10));
                warm +=
                    usize::from(r > 120 && r > g.saturating_add(35) && g > b.saturating_add(15));
                white += usize::from(r > 180 && g > 180 && b > 180);
                foreground += usize::from(r > 80 || g > 90 || b > 120);
                magenta += usize::from(r > 200 && g < 60 && b > 200);
            }
        }
        assert_eq!(magenta, 0, "Error-tonemapping pixels in {case}: {path}");
        assert!(
            foreground > 100,
            "Only a background was rendered for {case}: {path}"
        );
        if case != "ship.glb" {
            assert!(
                blue > 50 && white > 50,
                "Image/UI content missing in {case}: {path}"
            );
        }
        if case.starts_with("mixed") || case == "Composed.scene.ron" {
            assert!(
                warm > 50,
                "Spatial material missing from mixed scene: {path}"
            );
        }
        if case == "ship.glb" || case.starts_with("mixed") || case == "Composed.scene.ron" {
            assert!(
                model_visible(&crop),
                "Generated cyan/yellow model missing in {case}: {path}"
            );
        }
    }
    Ok(())
}
