//! Shared source intake uses the same bounded decoder, not capture semantics.
use super::{images, parse, report};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    export: String,
    purpose: String,
    frames: BTreeMap<String, Frame>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    image: PathBuf,
    sha256: String,
    size: [u32; 2],
    kind: String,
    review: String,
    source: String,
}

pub(super) fn inspect(manifest_path: &Path, output: &Path) -> crate::Result<()> {
    let manifest: Manifest = parse(manifest_path)?;
    if manifest.schema != 1
        || manifest.export.trim().is_empty()
        || manifest.purpose.trim().is_empty()
        || manifest.frames.is_empty()
        || manifest.frames.len() > 1024
    {
        return Err("invalid independent export manifest".into());
    }
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let mut inspected = BTreeMap::new();
    let result = (|| -> crate::Result<()> {
        for (node, frame) in &manifest.frames {
            if frame.kind != "design_reference"
                || !matches!(frame.review.as_str(), "pending" | "human_reviewed")
                || !frame.source.starts_with("https://www.figma.com/")
                || frame.size.contains(&0)
            {
                return Err(format!("invalid Figma source identity: {node}").into());
            }
            let image_path = images::resolve_image(manifest_path, &frame.image)?;
            let bytes = images::read_bounded(&image_path, images::MAX_FILE_BYTES)?;
            if images::digest(&bytes) != frame.sha256 {
                return Err(format!("source export hash differs: {node}").into());
            }
            // Alpha and uniform fills can be legitimate design material. They
            // are not proof of a valid native capture and never pass its gate.
            images::decode(&bytes, frame.size)?;
            inspected.insert(
                node,
                json!({"image":frame.image,"sha256":frame.sha256,"size":frame.size}),
            );
            println!(
                "Inspected Figma export {node}: {} × {}",
                frame.size[0], frame.size[1]
            );
        }
        Ok(())
    })();
    report::write_json(
        output.join("report.json"),
        &json!({
            "schema":1,"export_integrity":if result.is_ok() {"passed"} else {"failed"},
            "native_capture":false,"design_acceptance":"not_granted","frames":inspected,
            "error":result.as_ref().err().map(ToString::to_string),
        }),
    )?;
    result
}
