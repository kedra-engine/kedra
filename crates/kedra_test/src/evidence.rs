//! Immutable capture evidence and stability checks, without reference promotion.
use crate::{Client, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

/// Read a bounded regular file, rejecting growth while reading.
pub fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err("Evidence is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("Evidence exceeds size limit".into());
    }
    Ok(bytes)
}

/// SHA-256 identity of bytes, never a visual acceptance decision.
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hash a bounded native PNG or source asset (maximum 64 MiB).
pub fn file_digest(path: &Path) -> Result<String> {
    Ok(digest(&read(path, 64 * 1024 * 1024)?))
}

/// An untrusted renderer receipt, validated by stable/consume_fence before use.
/// Local paths must not be serialized into public reports.
#[derive(Deserialize)]
pub struct Frame {
    /// Private renderer-published file location.
    pub path: PathBuf,
    /// Physical width from the readback receipt.
    pub width: u32,
    /// Physical height from the readback receipt.
    pub height: u32,
    /// Producer receipt frame, not a presentation timestamp.
    pub frame: u64,
    /// Renderer uniform-image diagnostic.
    pub uniform: bool,
}

/// Public portable frame evidence; its image path is relative to the report root.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Capture {
    /// Relative image location chosen by the scenario.
    pub image: PathBuf,
    /// Byte identity of the original PNG.
    pub sha256: String,
    /// Receipt frame of the final stable sample.
    pub receipt_frame: u64,
    /// Physical readback dimensions.
    pub size: [u32; 2],
    /// True only for explicitly allowed uniform diagnostic layers.
    pub uniform: bool,
}

/// Completed GPU readback for behavior synchronization, not visual acceptance.
/// No image, pixel hash, reference or comparison verdict is published.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct RenderFence {
    /// Native renderer frame which completed the readback.
    pub receipt_frame: u64,
    /// Verified physical readback dimensions.
    pub size: [u32; 2],
}

/// Fence real rendering without adding screenshot coverage. Uses the existing
/// authenticated readback, validates its session-owned PNG and dimensions, then
/// retires that temporary file. Invalid readbacks remain private for diagnostics.
/// This checks delivery only: it does not compare pixels or certify visual quality.
pub fn render_fence(
    client: &Client,
    window: u64,
    artifacts: &Path,
    size: [u32; 2],
) -> Result<RenderFence> {
    let frame = client.capture(window)?;
    consume_fence(frame, artifacts, size)
}

/// Validate/retire a real readback returned by an in-application sequence. Expected
/// dimensions come from the independent scenario, not the receipt under test.
pub fn consume_fence(frame: Frame, artifacts: &Path, size: [u32; 2]) -> Result<RenderFence> {
    if !frame
        .path
        .canonicalize()?
        .starts_with(artifacts.canonicalize()?)
        || [frame.width, frame.height] != size
    {
        return Err("Renderer fence differs from its session path or dimensions".into());
    }
    let bytes = read(&frame.path, 64 * 1024 * 1024)?;
    let _ = crate::visual::decode(&bytes, size)?;
    fs::remove_file(&frame.path)?;
    Ok(RenderFence {
        receipt_frame: frame.frame,
        size,
    })
}

/// Require three consecutive byte-identical captures within sixteen attempts.
/// Every frame is decoded with the maintained image library and checked against
/// expected dimensions. Uniform frames require a nonempty diagnostic rationale.
/// Published files must resolve inside the session's artifact directory.
pub fn stable(
    client: &Client,
    window: u64,
    artifacts: &Path,
    size: [u32; 2],
    uniform_reason: Option<&str>,
) -> Result<(Frame, String)> {
    if uniform_reason.is_some_and(|reason| reason.trim().is_empty()) {
        return Err("Uniform diagnostics require an explicit rationale".into());
    }
    let mut previous = String::new();
    let mut count = 0;
    for _ in 0..16 {
        let frame = client.capture(window)?;
        // The runtime publishes its artifact directory lazily with the first
        // completed renderer capture, not when transport is enabled.
        let artifacts = artifacts.canonicalize()?;
        if !frame.path.canonicalize()?.starts_with(&artifacts)
            || [frame.width, frame.height] != size
        {
            return Err("Capture path or dimensions differ from session contract".into());
        }
        let bytes = read(&frame.path, 64 * 1024 * 1024)?;
        let decoded = crate::visual::decode(&bytes, size)?;
        let uniform = decoded.pixels().all(|pixel| *pixel == decoded[(0, 0)]);
        if decoded.pixels().any(|pixel| pixel[3] != 255) {
            return Err("Nonopaque native capture".into());
        }
        if uniform != frame.uniform || (uniform && uniform_reason.is_none()) {
            return Err("Uniform full frame or inconsistent renderer diagnostic".into());
        }
        let hash = digest(&bytes);
        count = if hash == previous { count + 1 } else { 1 };
        previous = hash;
        if count == 3 {
            return Ok((frame, previous));
        }
    }
    Err("Renderer did not settle to three identical consecutive frames".into())
}

/// Copy a stable frame without overwriting; recheck its identity during publication.
pub fn publish(frame: &Frame, hash: &str, root: &Path, relative: &Path) -> Result<Capture> {
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Public image must have a safe relative path".into());
    }
    let bytes = read(&frame.path, 64 * 1024 * 1024)?;
    if digest(&bytes) != hash {
        return Err("Stable frame changed before publication".into());
    }
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    fs::File::create_new(&path)?.write_all(&bytes)?;
    Ok(Capture {
        image: relative.into(),
        sha256: hash.into(),
        receipt_frame: frame.frame,
        size: [frame.width, frame.height],
        uniform: frame.uniform,
    })
}
