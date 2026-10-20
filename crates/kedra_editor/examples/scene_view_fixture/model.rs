//! Generated QA geometry only: two opaque unlit cubes, cyan and yellow.
//! Native Bevy mesh generation and glTF serialization own their file formats.
use bevy::{
    gltf::gltf::binary::{Glb, Header},
    mesh::VertexAttributeValues,
    prelude::*,
};
use std::borrow::Cow;

pub fn generate() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mesh = Mesh::from(Cuboid::new(0.9, 1.8, 0.9));
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return Err("Native cuboid positions missing".into());
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        return Err("Native cuboid normals missing".into());
    };
    let indices = mesh.indices().ok_or("Native cuboid indices missing")?;
    let mut bin = Vec::new();
    for values in [positions, normals] {
        for value in values.iter().flatten() {
            bin.extend(value.to_le_bytes());
        }
    }
    let index_offset = bin.len();
    for index in indices.iter() {
        bin.extend(u32::try_from(index)?.to_le_bytes());
    }
    let materials = [[0.0, 1.0, 1.0, 1.0], [1.0, 1.0, 0.0, 1.0]].map(|color| serde_json::json!({
        "pbrMetallicRoughness": {"baseColorFactor": color, "metallicFactor": 0.0, "roughnessFactor": 1.0},
        "extensions": {"KHR_materials_unlit": {}}
    }));
    let json = serde_json::json!({
        "asset": {"version": "2.0", "generator": "Kedra generated native QA fixture"},
        "extensionsUsed": ["KHR_materials_unlit"],
        "scene": 0,
        "scenes": [{"nodes": [0, 1]}],
        "nodes": [
            {"name": "Cyan fixture cube", "mesh": 0, "translation": [-0.55, 0.0, 0.0]},
            {"name": "Yellow fixture cube", "mesh": 1, "translation": [0.55, 0.0, 0.0]}
        ],
        "meshes": [
            {"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2, "material": 0}]},
            {"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2, "material": 1}]}
        ],
        "materials": materials,
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": positions.len() * 12},
            {"buffer": 0, "byteOffset": positions.len() * 12, "byteLength": normals.len() * 12},
            {"buffer": 0, "byteOffset": index_offset, "byteLength": indices.len() * 4}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": positions.len(), "type": "VEC3",
             "min": [-0.45, -0.9, -0.45], "max": [0.45, 0.9, 0.45]},
            {"bufferView": 1, "componentType": 5126, "count": normals.len(), "type": "VEC3"},
            {"bufferView": 2, "componentType": 5125, "count": indices.len(), "type": "SCALAR"}
        ]
    });
    let mut bytes = Vec::new();
    Glb {
        header: Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(serde_json::to_vec(&json)?),
        bin: Some(Cow::Owned(bin)),
    }
    .to_writer(&mut bytes)?;
    Ok(bytes)
}
