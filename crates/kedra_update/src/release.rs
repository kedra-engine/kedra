//! Strict consumer of the existing official GitHub release/inventory contract.
use crate::Config;
use semver::Version;
use serde::Deserialize;
use url::Url;

pub(super) const MAX_PACKAGE: u64 = 2 * 1024 * 1024 * 1024;
pub(super) const MAX_LISTING: usize = 256 * 1024;
pub(super) const MAX_INVENTORY: usize = 1024 * 1024;
const FORMATS: [(&str, &str, &str); 5] = [
    ("x86_64-pc-windows-msvc", "nsis", "exe"),
    ("x86_64-pc-windows-msvc", "zip", "zip"),
    ("x86_64-unknown-linux-gnu", "appimage", "AppImage"),
    ("x86_64-unknown-linux-gnu", "tar.gz", "tar.gz"),
    ("aarch64-apple-darwin", "dmg", "dmg"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
/// Supported primary installation kinds; portable/source installations are excluded.
pub enum Kind {
    /// Windows x64 installed through NSIS; manual installation in this MVP.
    Nsis,
    /// Linux x64 outer AppImage; manual installation in this MVP.
    AppImage,
    /// Installed macOS ARM64 application bundle; explicit automatic replacement.
    MacApp,
}
impl Kind {
    /// Exact release target triple for this installation kind.
    pub fn target(self) -> &'static str {
        match self {
            Self::Nsis => "x86_64-pc-windows-msvc",
            Self::AppImage => "x86_64-unknown-linux-gnu",
            Self::MacApp => "aarch64-apple-darwin",
        }
    }
    /// Existing inventory format identifier.
    pub fn format(self) -> &'static str {
        match self {
            Self::Nsis => "nsis",
            Self::AppImage => "appimage",
            Self::MacApp => "dmg",
        }
    }
    /// Exact case-sensitive package extension.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Nsis => "exe",
            Self::AppImage => "AppImage",
            Self::MacApp => "dmg",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Asset {
    pub name: String,
    pub browser_download_url: Url,
    pub size: u64,
    pub state: String,
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct Candidate {
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub html_url: Url,
    pub target_commitish: String,
    pub body: Option<String>,
    pub assets: Vec<Asset>,
}
#[derive(Clone, Debug)]
/// Immutable validated release metadata. Construction requires official repository,
/// version, complete inventory, target, size and SHA-256 checks.
pub struct Release {
    pub(crate) version: Version,
    pub(crate) source_revision: String,
    pub(crate) notes: String,
    pub(crate) name: String,
    pub(crate) url: Url,
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
}

impl Release {
    /// Selected newer application version.
    pub fn version(&self) -> &Version {
        &self.version
    }
    /// Exact source revision bound by the release inventory.
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    /// Bounded plain-text summary; presentation is the consumer's responsibility.
    pub fn notes(&self) -> &str {
        &self.notes
    }
    /// Expected payload size in bytes, not a server-reported progress total.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

pub(super) fn candidate(config: &Config, bytes: &[u8]) -> Result<Option<Candidate>, String> {
    if bytes.len() > MAX_LISTING {
        return Err("Release listing is too large".into());
    }
    let releases: Vec<Candidate> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    Ok(releases
        .into_iter()
        .filter(|r| {
            !r.draft
                && r.prerelease
                && version(r).is_ok_and(|v| v > config.current)
                && r.html_url.as_str()
                    == format!(
                        "https://github.com/{}/releases/tag/{}",
                        config.repository, r.tag_name
                    )
        })
        .max_by_key(|r| version(r).ok()))
}
fn version(c: &Candidate) -> Result<Version, String> {
    let value = c
        .tag_name
        .strip_prefix('v')
        .ok_or("Release tag must start with v")?;
    let version = Version::parse(value).map_err(|e| e.to_string())?;
    if !version.pre.is_empty() || !version.build.is_empty() || version.to_string() != value {
        return Err("Unsupported release version".into());
    }
    Ok(version)
}
pub(super) fn asset<'a>(
    config: &Config,
    c: &'a Candidate,
    name: &str,
) -> Result<&'a Asset, String> {
    let items: Vec<_> = c.assets.iter().filter(|a| a.name == name).collect();
    let [item] = items.as_slice() else {
        return Err("Missing or duplicate release asset".into());
    };
    let expected = format!(
        "https://github.com/{}/releases/download/{}/{name}",
        config.repository, c.tag_name
    );
    if item.state != "uploaded" || item.size == 0 || item.browser_download_url.as_str() != expected
    {
        return Err("Release asset identity or origin is invalid".into());
    }
    Ok(item)
}
#[derive(Deserialize)]
struct Inventory {
    schema: u32,
    application: String,
    version: String,
    source_revision: String,
    artifacts: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    name: String,
    format: String,
    target: String,
    bytes: u64,
    sha256: String,
}

pub(super) fn validate(
    config: &Config,
    c: &Candidate,
    bytes: &[u8],
    kind: Kind,
) -> Result<Release, String> {
    if bytes.len() > MAX_INVENTORY {
        return Err("Release inventory is too large".into());
    }
    let v = version(c)?;
    if c.draft
        || !c.prerelease
        || c.html_url.as_str()
            != format!(
                "https://github.com/{}/releases/tag/{}",
                config.repository, c.tag_name
            )
    {
        return Err("Not an official published early-access release".into());
    }
    let inventory: Inventory = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if inventory.schema != 1
        || inventory.application != config.application
        || inventory.version != v.to_string()
        || inventory.source_revision != c.target_commitish
        || inventory.source_revision.len() != 40
        || !inventory
            .source_revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || inventory.artifacts.len() != FORMATS.len()
        || c.assets.len() != FORMATS.len() + 2
    {
        return Err("Release inventory does not match official source/version/schema".into());
    }
    asset(config, c, "artifact-inventory.json")?;
    asset(config, c, "SHA256SUMS")?;
    for (target, format, extension) in FORMATS {
        let name = format!("{}-{v}-{target}.{extension}", config.application);
        let entries: Vec<_> = inventory
            .artifacts
            .iter()
            .filter(|e| e.name == name)
            .collect();
        let [entry] = entries.as_slice() else {
            return Err("Release is incomplete or has duplicate artifacts".into());
        };
        let file = asset(config, c, &name)?;
        if entry.target != target
            || entry.format != format
            || entry.bytes == 0
            || entry.bytes > MAX_PACKAGE
            || entry.bytes != file.size
            || entry.sha256.len() != 64
            || !entry
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Invalid release artifact size/hash/target".into());
        }
    }
    let name = format!(
        "{}-{v}-{}.{}",
        config.application,
        kind.target(),
        kind.extension()
    );
    let entry = inventory
        .artifacts
        .iter()
        .find(|e| e.name == name && e.format == kind.format())
        .ok_or("Platform artifact is missing")?;
    let mut notes = c
        .body
        .as_deref()
        .unwrap_or("No release summary provided.")
        .to_owned();
    let mut end = notes.len().min(16 * 1024);
    while !notes.is_char_boundary(end) {
        end -= 1;
    }
    notes.truncate(end);
    notes.retain(|c| !c.is_control() || c == '\n');
    Ok(Release {
        version: v,
        source_revision: inventory.source_revision,
        notes,
        name: name.clone(),
        url: asset(config, c, &name)?.browser_download_url.clone(),
        bytes: entry.bytes,
        sha256: entry.sha256.clone(),
    })
}

#[cfg(test)]
#[path = "release_tests.rs"]
mod tests;
