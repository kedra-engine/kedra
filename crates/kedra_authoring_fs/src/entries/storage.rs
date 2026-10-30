//! Native entry journal adapter; upstream still owns transaction and rollback.
//! fs-transaction 0.4.0's StdFs sync opens files read-only, which cannot call
//! FlushFileBuffers on Windows. Reuse Save's existing write-capable flush helper.
use fs_transaction::{
    ReadStorage, StdFs, Storage,
    fs::{Capabilities, DirEntry, Durability, Metadata},
};
use std::{io, path::Path};

pub(super) struct EntryStorage;

impl ReadStorage for EntryStorage {
    async fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        StdFs.read(path).await
    }
    async fn read_to_string(&self, path: &Path) -> io::Result<String> {
        StdFs.read_to_string(path).await
    }
    async fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        StdFs.read_dir(path).await
    }
    async fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        StdFs.metadata(path).await
    }
}

impl Storage for EntryStorage {
    async fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        StdFs.write(path, contents).await
    }
    async fn create_new(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        StdFs.create_new(path, contents).await
    }
    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        StdFs.create_dir_all(path).await
    }
    async fn remove_file(&self, path: &Path) -> io::Result<()> {
        StdFs.remove_file(path).await
    }
    async fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        StdFs.remove_dir_all(path).await
    }
    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        StdFs.rename(from, to).await
    }
    async fn copy_permissions(&self, from: &Path, to: &Path) -> io::Result<()> {
        StdFs.copy_permissions(from, to).await
    }
    fn capabilities(&self) -> Capabilities {
        StdFs.capabilities()
    }
    async fn sync(&self, path: &Path, need: Durability) -> io::Result<()> {
        #[cfg(windows)]
        {
            // sync_all satisfies every requested durability strength.
            let _ = need;
            match std::fs::metadata(path) {
                Ok(metadata) if metadata.is_dir() => crate::sync_directory(path),
                Ok(_) => crate::sync_existing_file(path),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
        #[cfg(not(windows))]
        StdFs.sync(path, need).await
    }
}
