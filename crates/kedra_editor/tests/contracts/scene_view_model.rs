//! Source-model contract, independent of screenshot coverage.
#[test]
fn selection_fixture_has_distinct_opaque_parts_and_drawable_native_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = include_bytes!("../../../../tests/fixtures/test_project/assets/ship.glb");
    kedra_assets::model::validate_glb(bytes)?;
    let gltf = bevy::gltf::gltf::Gltf::from_slice(bytes)?;
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
            let positions = reader.read_positions().ok_or("Missing positions")?.count();
            assert!(positions >= 3);
            assert_eq!(
                reader.read_normals().ok_or("Missing normals")?.count(),
                positions
            );
            let indices: Vec<_> = reader
                .read_indices()
                .ok_or("Missing indices")?
                .into_u32()
                .collect();
            assert_eq!(primitive.mode(), bevy::gltf::gltf::mesh::Mode::Triangles);
            assert!(!indices.is_empty() && indices.len().is_multiple_of(3));
            assert!(indices.iter().all(|index| (*index as usize) < positions));
        }
    }
    Ok(())
}
