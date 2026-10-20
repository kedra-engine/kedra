//! Source-independent delivery manifest. Filesystem access is supplied by hosts.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// Published delivery marker, written only after all output is validated.
pub const MANIFEST: &str = "kedra-assets.json";
/// Maximum encoded delivery manifest size, shared by native readers and producers.
pub const MAX_MANIFEST_BYTES: usize = 65_536;
/// Content checksum of an asset or its loader metadata.
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Portable physical member whose native asset path preserves its exact spelling.
/// Absolute paths, traversal and URI source/label delimiters are rejected.
pub fn normalized(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', '\0', ':'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && bevy::asset::AssetPath::try_parse(path).is_ok_and(|native| {
            matches!(native.source(), bevy::asset::io::AssetSourceId::Default)
                && native.label().is_none()
                && native.path().to_str() == Some(path)
        })
}
/// Expected byte content of one delivery member.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileDigest {
    /// Exact byte length, checked before accepting a read.
    pub bytes: u64,
    /// SHA-256 of the whole member.
    pub sha256: String,
}
impl FileDigest {
    pub(crate) fn read_limit(&self, path: &str) -> Result<u64, String> {
        // Bound even a malicious declared length, not only corrupt member bytes.
        let maximum = if path.ends_with(".meta") {
            65_536
        } else {
            256 * 1024 * 1024
        };
        if self.bytes > maximum {
            return Err(format!(
                "Asset delivery member exceeds its byte limit: {path}"
            ));
        }
        let limit = self
            .bytes
            .checked_add(1)
            .ok_or("Asset byte limit overflow")?;
        usize::try_from(limit).map_err(|_| "Asset byte limit exceeds address space")?;
        Ok(limit)
    }
}
/// Stable identity mapped to a physical processed path and native loader.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRecord {
    /// UUID retained by source metadata across renames.
    pub id: Uuid,
    /// Normalized delivery-relative path.
    pub path: String,
    /// Exact Bevy loader type path from processed metadata.
    pub loader: String,
}
/// Versioned, immutable processed delivery, independent of the source tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Manifest contract version.
    pub format: u32,
    /// Processing profile; currently `editor-desktop`.
    pub profile: String,
    /// Delivered assets; ignored source files never appear here.
    pub assets: Vec<AssetRecord>,
    /// All required bytes and loader metadata, excluding this manifest itself.
    pub files: BTreeMap<String, FileDigest>,
}
impl Manifest {
    /// Validate manifest structure without opening asset members. This permits a
    /// bounded bootstrap UI through the hash-verifying reader while the host
    /// verifies the complete delivery asynchronously before normal startup.
    /// Declared members are limited to 256 MiB (64 KiB for `.meta` members).
    pub fn validate(&self, profile: &str) -> Result<(), String> {
        if self.format != 1 || self.profile != profile {
            return Err("Asset delivery format/profile mismatch".into());
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for asset in &self.assets {
            if asset.id.is_nil()
                || !ids.insert(asset.id)
                || !paths.insert(&asset.path)
                || !normalized(&asset.path)
                || crate::source::uuid_reference(&asset.path).is_some()
                || !self.files.contains_key(&asset.path)
                || !self.files.contains_key(&format!("{}.meta", asset.path))
            {
                return Err(format!(
                    "Invalid/duplicate asset identity or incomplete delivery: {}",
                    asset.path
                ));
            }
        }
        for (path, expected) in &self.files {
            if !normalized(path) || path == MANIFEST {
                return Err(format!("Invalid delivery member: {path}"));
            }
            expected.read_limit(path)?;
        }
        Ok(())
    }
    /// Validate identity, paths, native loaders and every member's content.
    /// No sources are modified. The reader must not follow paths outside its root.
    pub fn verify(
        &self,
        profile: &str,
        mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        self.validate(profile)?;
        for asset in &self.assets {
            let meta = read(&format!("{}.meta", asset.path))?;
            let meta: bevy::asset::meta::AssetMetaMinimal =
                ron::de::from_bytes(&meta).map_err(|e| e.to_string())?;
            if !matches!(meta.asset,bevy::asset::meta::AssetActionMinimal::Load {loader} if loader == asset.loader)
            {
                return Err(format!("Unexpected processed loader: {}", asset.path));
            }
        }
        for (path, expected) in &self.files {
            let bytes = read(path)?;
            if bytes.len() as u64 != expected.bytes || digest(&bytes) != expected.sha256 {
                return Err(format!("Asset delivery integrity failure: {path}"));
            }
        }
        Ok(())
    }
}

/// Read and verify a native filesystem delivery. Sources and import caches are
/// not required. Members escaping the canonical root are rejected.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_directory(
    root: &std::path::Path,
    profile: &str,
) -> Result<(std::path::PathBuf, Manifest), String> {
    let (root, manifest) = read_manifest(root, profile)?;
    manifest.verify(profile, |relative| {
        let expected = manifest
            .files
            .get(relative)
            .ok_or("Undeclared delivery member")?;
        expected.read_limit(relative)?;
        crate::filesystem::read(&root, relative, expected.bytes)
    })?;
    Ok((root, manifest))
}

/// Read only the bounded, structurally validated manifest. This is not complete
/// delivery verification. Bootstrap consumers must use the hash-verifying asset
/// reader and finish `read_directory` verification before entering the product.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_manifest(
    root: &std::path::Path,
    profile: &str,
) -> Result<(std::path::PathBuf, Manifest), String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("Cannot access asset delivery {}: {e}", root.display()))?;
    let bytes =
        crate::filesystem::read(&root, MANIFEST, MAX_MANIFEST_BYTES as u64).map_err(|e| {
            format!("Cannot read asset delivery manifest: {e}; build the delivery before launching")
        })?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    manifest.validate(profile)?;
    Ok((root, manifest))
}
