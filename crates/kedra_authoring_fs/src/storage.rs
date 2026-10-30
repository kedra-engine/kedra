//! Bounded existing-file port; journal/rollback algorithms stay in fs-transaction.
use super::*;
use fs_transaction::{
    ReadStorage, StdFs, Storage,
    fs::{Capabilities, DirEntry, Durability, Metadata},
};
use std::io;
pub(super) struct Guarded<'a> {
    pub session: &'a group::Session,
    pub request: &'a CommitRequest,
}
fn denied() -> io::Error {
    io::Error::other("Save storage operation is outside its admitted request")
}
pub(super) fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.is_symlink() || metadata.len() > limit {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(denied());
    }
    Ok(bytes)
}
pub(super) fn read_journal(path: &Path) -> io::Result<Vec<u8>> {
    let bytes = read_bounded(path, 16 * 1024 * 1024)?;
    // Pinned fs-transaction 0.4.0 stores its op count immediately after the
    // eight-byte magic. Its codec has no caller-supplied allocation budget.
    // Bound that count before delegating all format/checksum/op parsing to it.
    let count = bytes
        .get(8..16)
        .and_then(|value| <[u8; 8]>::try_from(value).ok())
        .map(u64::from_le_bytes)
        .ok_or_else(denied)?;
    if count > 128 {
        return Err(denied());
    }
    Ok(bytes)
}
impl Guarded<'_> {
    // fs-transaction 0.4.0's removal rollback uses bounded, op-indexed sibling
    // names. Map only those exact granted names into our private staging area;
    // temporary removal backups must never enter the authored inventory.
    fn aside(&self, path: &Path) -> Option<(PathBuf, &kedra_authoring::persistence::CommitFile)> {
        for file in self
            .request
            .files
            .iter()
            .filter(|file| file.source.is_none())
        {
            let target = self.session.root.join(&file.path);
            let name = target.file_name()?.to_str()?;
            for index in 0..self.request.files.len() {
                if target.with_file_name(format!(".{name}.fstx-aside-{index}")) == path {
                    let key = format!("{:x}", Sha256::digest(file.path.as_bytes()));
                    return Some((
                        self.session
                            .staging
                            .join(format!("{}-{key}-{index}.removed", self.request.operation)),
                        file,
                    ));
                }
            }
        }
        None
    }
    fn staging(&self) -> io::Result<&Path> {
        let path = &self.session.staging;
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        match builder.create(path) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.is_symlink() {
            return Err(denied());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(denied());
            }
        }
        Ok(path)
    }
    fn admitted(&self, path: &Path) -> bool {
        path == self.session.journal.path_in(&self.session.root)
            || self
                .request
                .files
                .iter()
                .any(|file| path == self.session.root.join(&file.path))
    }
    fn check(&self, path: &Path) -> io::Result<()> {
        if !self.admitted(path) {
            return Err(denied());
        }
        directories(path.parent().ok_or_else(denied)?).map_err(io::Error::other)
    }
    fn current(&self, path: &Path) -> io::Result<()> {
        if let Some(file) = self
            .request
            .files
            .iter()
            .find(|file| path == self.session.root.join(&file.path))
        {
            let actual = optional_hash(path).map_err(io::Error::other)?;
            if actual != file.expected_hash && actual != file.source.as_deref().map(content_hash) {
                return Err(denied());
            }
        }
        Ok(())
    }
}
impl ReadStorage for Guarded<'_> {
    async fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        if let Some((path, _)) = self.aside(path) {
            self.staging()?;
            return read_bounded(&path, 65_536);
        }
        self.check(path)?;
        if path == self.session.journal.path_in(&self.session.root) {
            return read_journal(path);
        }
        read_bounded(path, 16 * 1024 * 1024)
    }
    async fn read_to_string(&self, path: &Path) -> io::Result<String> {
        String::from_utf8(self.read(path).await?).map_err(io::Error::other)
    }
    async fn read_dir(&self, _: &Path) -> io::Result<Vec<DirEntry>> {
        Err(denied())
    }
    async fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        if let Some((path, file)) = self.aside(path) {
            self.staging()?;
            if optional_hash(&path).map_err(io::Error::other)?.as_ref()
                != file.expected_hash.as_ref()
            {
                return Err(if !path.exists() {
                    io::ErrorKind::NotFound.into()
                } else {
                    denied()
                });
            }
            return StdFs.metadata(&path).await;
        }
        if path == self.session.staging {
            let metadata = std::fs::symlink_metadata(path)?;
            if !metadata.is_dir() || metadata.is_symlink() {
                return Err(denied());
            }
            return StdFs.metadata(path).await;
        }
        if self.request.files.iter().any(|file| {
            self.session
                .root
                .join(&file.path)
                .parent()
                .is_some_and(|parent| parent.ancestors().any(|ancestor| ancestor == path))
        }) {
            directories(path).map_err(io::Error::other)?;
        } else {
            self.check(path)?;
            let metadata = std::fs::symlink_metadata(path)?;
            if !metadata.is_file() || metadata.is_symlink() {
                return Err(denied());
            }
        }
        StdFs.metadata(path).await
    }
}
impl Storage for Guarded<'_> {
    async fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        // The upstream rollback uses write, not replace. Keep the same exact-byte
        // grant and atomic replacement here rather than disabling its undo route.
        self.replace(path, contents).await
    }
    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        if path == self.session.staging {
            self.staging()?;
            return Ok(());
        }
        directories(path).map_err(io::Error::other)
    }
    async fn remove_file(&self, path: &Path) -> io::Result<()> {
        if let Some((path, file)) = self.aside(path) {
            self.staging()?;
            if optional_hash(&path).map_err(io::Error::other)? != file.expected_hash {
                return Err(denied());
            }
            std::fs::remove_file(&path)?;
            return sync_directory(&self.session.staging);
        }
        if path != self.session.journal.path_in(&self.session.root) {
            // Forward deletion and creation rollback own only exact admitted bytes.
            let file = self
                .request
                .files
                .iter()
                .find(|file| {
                    (file.expected_hash.is_none() || file.source.is_none())
                        && path == self.session.root.join(&file.path)
                })
                .ok_or_else(denied)?;
            let actual = optional_hash(path).map_err(io::Error::other)?;
            if actual != file.source.as_deref().map(content_hash) && actual != file.expected_hash {
                return Err(denied());
            }
        }
        self.check(path)?;
        read_bounded(path, 16 * 1024 * 1024)?;
        std::fs::remove_file(path)
    }
    async fn remove_dir_all(&self, _: &Path) -> io::Result<()> {
        Err(denied())
    }
    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let (from, to, expected) = if let Some((aside, file)) = self.aside(to) {
            if from != self.session.root.join(&file.path) {
                return Err(denied());
            }
            (from.to_path_buf(), aside, &file.expected_hash)
        } else if let Some((aside, file)) = self.aside(from) {
            if to != self.session.root.join(&file.path) {
                return Err(denied());
            }
            (aside, to.to_path_buf(), &file.expected_hash)
        } else {
            return Err(denied());
        };
        self.staging()?;
        directories(from.parent().ok_or_else(denied)?).map_err(io::Error::other)?;
        directories(to.parent().ok_or_else(denied)?).map_err(io::Error::other)?;
        if optional_hash(&from).map_err(io::Error::other)?.as_ref() != expected.as_ref()
            || std::fs::symlink_metadata(&to).is_ok()
        {
            return Err(denied());
        }
        std::fs::rename(&from, &to)?;
        sync_directory(from.parent().ok_or_else(denied)?)?;
        sync_directory(to.parent().ok_or_else(denied)?)
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            exclusive_create: false,
            ..Capabilities::LOCAL_FS
        }
    }
    async fn sync(&self, path: &Path, need: Durability) -> io::Result<()> {
        if let Some((path, _)) = self.aside(path) {
            self.staging()?;
            return if path.exists() {
                sync_existing_file(&path)
            } else {
                Ok(())
            };
        }
        if path.is_dir() {
            directories(path).map_err(io::Error::other)?;
            return StdFs.sync(path, need).await;
        }
        self.check(path)?;
        match read_bounded(path, 16 * 1024 * 1024) {
            Ok(_) => sync_existing_file(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
    async fn replace(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        self.check(path)?;
        self.current(path)?;
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata)
                if metadata.is_file()
                    && !metadata.is_symlink()
                    && !metadata.permissions().readonly() =>
            {
                Some(metadata)
            }
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && (path == self.session.journal.path_in(&self.session.root)
                        || self.request.files.iter().any(|file| {
                            (file.expected_hash.is_none() || file.source.is_none())
                                && path == self.session.root.join(&file.path)
                        })) =>
            {
                None
            }
            _ => return Err(denied()),
        };
        if let Some(file) = self
            .request
            .files
            .iter()
            .find(|file| path == self.session.root.join(&file.path))
        {
            let hash = format!("{:x}", Sha256::digest(contents));
            if Some(&hash) != file.expected_hash.as_ref()
                && Some(hash) != file.source.as_deref().map(content_hash)
            {
                return Err(denied());
            }
        }
        // Keep crash-orphaned temporaries out of the authored asset inventory.
        // The private sibling staging directory is on the same local filesystem.
        let mut temp = tempfile::NamedTempFile::new_in(self.staging()?)?;
        let creating = metadata.is_none();
        if let Some(metadata) = metadata {
            temp.as_file().set_permissions(metadata.permissions())?;
        }
        temp.write_all(contents)?;
        temp.as_file().sync_all()?;
        self.current(path)?;
        if creating {
            temp.persist_noclobber(path).map_err(|error| error.error)?;
        } else {
            temp.persist(path).map_err(|error| error.error)?;
        }
        Ok(())
    }
}
