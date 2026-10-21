//! Streaming integrity verification; partial files never become an update candidate.
use super::release::Release;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// An integrity-verified package with privately owned temporary staging.
///
/// Drop removes staging unless [`Self::persist`] was explicitly requested. A package
/// can change on disk after construction; reverify immediately before use.
pub struct VerifiedPackage {
    directory: Option<tempfile::TempDir>,
    path: PathBuf,
}
impl VerifiedPackage {
    /// Local native path; not a durable verification receipt.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Retains this package after drop for manual installation. The consumer owns
    /// eventual cache cleanup; this does not restore Ready on subsequent launches.
    pub fn persist(&mut self) -> &Path {
        if let Some(directory) = self.directory.take() {
            let _ = directory.keep();
        }
        &self.path
    }

    /// Rechecks regular-file identity, exact size and SHA-256. Returns an error on
    /// mutation, missing data or IO failure; does not install or execute the package.
    pub fn reverify(&self, release: &Release) -> Result<(), String> {
        reverify(&self.path, release)
    }
}

/// Streams a package into private cache staging with exact size/hash checks.
///
/// Progress/verifying callbacks run inline. Errors drop partial files. The caller
/// supplies a bounded reader/transport timeout; this function cannot interrupt a
/// blocking reader. Only a validated [`Release`] can describe the expected payload.
pub fn stage<R: Read>(
    mut source: R,
    release: &Release,
    cache: &Path,
    mut progress: impl FnMut(u64),
    verifying: impl FnOnce(),
) -> Result<VerifiedPackage, String> {
    if release.bytes == 0
        || release.bytes > super::release::MAX_PACKAGE
        || Path::new(&release.name)
            .file_name()
            .and_then(|n| n.to_str())
            != Some(release.name.as_str())
    {
        return Err("Invalid package size or filename".into());
    }
    std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let directory = tempfile::Builder::new()
        .prefix("kedra-update-")
        .tempdir_in(cache)
        .map_err(|e| e.to_string())?;
    let path = directory.path().join(&release.name);
    let mut file = File::create_new(&path).map_err(|e| e.to_string())?;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > release.bytes {
            return Err("Downloaded package exceeds expected size".into());
        }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        progress(total);
    }
    if total != release.bytes {
        return Err("Downloaded package is incomplete".into());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    verifying();
    reverify(&path, release)?;
    Ok(VerifiedPackage {
        directory: Some(directory),
        path,
    })
}

pub(super) fn reverify(path: &Path, release: &Release) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file()
        || metadata.len() != release.bytes
        || release.bytes > super::release::MAX_PACKAGE
    {
        return Err("Staged package identity changed".into());
    }
    let mut file = File::open(path)
        .map_err(|e| e.to_string())?
        .take(release.bytes + 1);
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    let mut count = 0_u64;
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        count += n as u64;
        hash.update(&buffer[..n]);
    }
    if count != release.bytes || format!("{:x}", hash.finalize()) != release.sha256 {
        return Err("Update integrity verification failed; download again".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn release() -> Release {
        Release {
            version: semver::Version::new(0, 2, 0),
            source_revision: "1".repeat(40),
            notes: String::new(),
            name: "package.AppImage".into(),
            url: url::Url::parse(
                "https://github.com/kedra-engine/kedra/releases/download/v0.2.0/package.AppImage",
            )
            .unwrap_or_else(|e| panic!("{e}")),
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
        }
    }
    #[test]
    fn valid_bytes_are_verified_before_ready_and_can_be_reverified() -> TestResult {
        let dir = tempfile::tempdir()?;
        let mut count = 0;
        let mut verifying = false;
        let package = stage(
            &b"abc"[..],
            &release(),
            dir.path(),
            |bytes| count = bytes,
            || verifying = true,
        )?;
        assert_eq!(count, 3);
        assert!(verifying);
        assert_eq!(std::fs::read(package.path())?, b"abc");
        reverify(package.path(), &release())?;
        std::fs::write(package.path(), b"bad")?;
        assert!(reverify(package.path(), &release()).is_err());
        Ok(())
    }
    #[test]
    fn truncated_oversized_and_wrong_hash_never_publish_partial_files() -> TestResult {
        let dir = tempfile::tempdir()?;
        for body in [b"ab".as_slice(), b"abcd".as_slice(), b"bad".as_slice()] {
            assert!(stage(body, &release(), dir.path(), |_| {}, || {}).is_err());
            assert_eq!(std::fs::read_dir(dir.path())?.count(), 0);
        }
        Ok(())
    }
    #[test]
    fn revealed_packages_survive_owner_drop_but_partial_downloads_do_not() -> TestResult {
        let dir = tempfile::tempdir()?;
        let mut package = stage(&b"abc"[..], &release(), dir.path(), |_| {}, || {})?;
        let path = package.persist().to_owned();
        drop(package);
        assert_eq!(std::fs::read(path)?, b"abc");
        Ok(())
    }
}
