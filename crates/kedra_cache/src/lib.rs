//! Disposable project-local derived data. `cacache` owns integrity verification,
//! concurrent publication and the on-disk index; Kedra owns placement and quotas.
//! All methods perform blocking I/O: call them on an existing worker pool.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// A namespaced, versioned content key; no user-provided text becomes a filename.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key(String);
impl Key {
    /// Include all input/dependency revisions and options in `parts`. Length
    /// prefixes prevent ambiguous concatenation; a provider version invalidates
    /// old representations without changing source identity.
    pub fn new(namespace: &str, version: u32, parts: &[&[u8]]) -> Self {
        let mut hash = Sha256::new();
        for part in std::iter::once(namespace.as_bytes())
            .chain(std::iter::once(version.to_le_bytes().as_slice()))
            .chain(parts.iter().copied())
        {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
        Self(format!("{:x}", hash.finalize()))
    }
}

/// Per-project global payload limits, shared across all provider namespaces.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum bytes of an individual value, checked on both read and write.
    pub entry_bytes: usize,
    /// Total indexed payload budget; index/temporary-file overhead is additional.
    pub total_bytes: usize,
    /// Maximum indexed entries.
    pub entries: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            entry_bytes: 4 * 1024 * 1024,
            total_bytes: 1024 * 1024 * 1024,
            entries: 4096,
        }
    }
}

/// Cloneable location/configuration, not a second source registry. Deleting the
/// cache loses only derived data. Read-only/busy/unavailable storage is an error
/// that consumers should report once and fall back to uncached generation.
#[derive(Clone, Debug)]
pub struct Store {
    directory: PathBuf,
    limits: Limits,
}
impl Store {
    /// Select `.kedra/cache/v1` below an existing project root. Does not create or
    /// touch storage until an operation. Never point this at source asset storage.
    pub fn project(root: &Path, limits: Limits) -> Result<Self, String> {
        if limits.entry_bytes == 0 || limits.total_bytes < limits.entry_bytes || limits.entries == 0
        {
            return Err("Invalid project cache limits".into());
        }
        Ok(Self {
            directory: root.join(".kedra/cache/v1"),
            limits,
        })
    }
    fn lock(&self) -> Result<File, String> {
        let project = self
            .directory
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or("Invalid project cache root")?;
        for path in [
            project.join(".kedra"),
            project.join(".kedra/cache"),
            self.directory.clone(),
        ] {
            match fs::symlink_metadata(&path) {
                Ok(m) if m.file_type().is_dir() => (),
                Ok(_) => return Err("Project cache location is not a regular directory".into()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&path).map_err(|e| e.to_string())?
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        let path = self.directory.join("owner.lock");
        if fs::symlink_metadata(&path).is_ok_and(|m| !m.file_type().is_file()) {
            return Err("Project cache lock is not a regular file".into());
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|e| format!("Project cache is busy: {e}"))?;
        let ignore = self
            .directory
            .parent()
            .ok_or("Invalid cache parent")?
            .join(".gitignore");
        match OpenOptions::new().create_new(true).write(true).open(ignore) {
            Ok(mut ignore) => ignore.write_all(b"*\n").map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        Ok(file)
    }
    fn data(&self) -> PathBuf {
        self.directory.join("data")
    }
    /// Read bounded, integrity-checked bytes. Missing/corrupt values are cache
    /// misses, never source failures. Infrastructure errors remain explicit.
    pub fn get(&self, key: &Key) -> Result<Option<Vec<u8>>, String> {
        let _lock = self.lock()?;
        let path = self.data();
        let Some(metadata) = cacache::metadata_sync(&path, &key.0).map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        if metadata.size > self.limits.entry_bytes {
            return Ok(None);
        }
        let Ok(mut reader) = cacache::SyncReader::open(&path, &key.0) else {
            return Ok(None);
        };
        let mut bytes = Vec::with_capacity(metadata.size);
        if reader
            .by_ref()
            .take(self.limits.entry_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > self.limits.entry_bytes
            || reader.check().is_err()
        {
            return Ok(None);
        }
        Ok(Some(bytes))
    }
    /// Atomically publish a complete value. Reject oversized values before I/O.
    /// Oldest-written entries are evicted across namespaces before publication;
    /// cache eviction never touches project sources or another project's cache.
    pub fn put(&self, key: &Key, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > self.limits.entry_bytes {
            return Err("Project cache entry exceeds its byte budget".into());
        }
        let _lock = self.lock()?;
        let path = self.data();
        if !path.exists() {
            cacache::write_sync(&path, &key.0, bytes).map_err(|e| e.to_string())?;
            return Ok(());
        }
        let mut entries = cacache::list_sync(&path)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|v| v.time);
        let mut total = entries
            .iter()
            .try_fold(0usize, |n, e| n.checked_add(e.size))
            .ok_or("Project cache accounting overflow")?;
        let mut count = entries.len();
        for (index, entry) in entries.iter().enumerate() {
            if total.saturating_add(bytes.len()) <= self.limits.total_bytes
                && count < self.limits.entries
            {
                break;
            }
            cacache::remove_sync(&path, &entry.key).map_err(|e| e.to_string())?;
            if !entries[index + 1..]
                .iter()
                .any(|other| other.integrity == entry.integrity)
            {
                match cacache::remove_hash_sync(&path, &entry.integrity) {
                    Ok(()) => (),
                    Err(cacache::Error::IoError(error, _))
                        if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
            total = total.saturating_sub(entry.size);
            count = count.saturating_sub(1);
        }
        cacache::write_sync(&path, &key.0, bytes).map_err(|e| e.to_string())?;
        Ok(())
    }
}
