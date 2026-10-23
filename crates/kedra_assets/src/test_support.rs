//! Binary GLB input construction for Kedra admission contracts.
use serde_json::Value;
pub(crate) fn glb(document: Value) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: std::borrow::Cow::Owned(serde_json::to_vec(&document)?),
        bin: None,
    }
    .to_vec()?)
}
