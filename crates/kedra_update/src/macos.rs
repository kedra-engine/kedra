//! Native DMG staging and whole-bundle replacement without privilege escalation.
use super::{Config, download, release::Release};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

/// Failure to establish safe update access without blocking ordinary read-only use.
#[derive(Debug)]
pub enum OpenError {
    /// Another updater holds the exclusive lease; the application must not run
    /// against an installation that may be replaced underneath it.
    Busy,
    /// Invalid identity/location, missing resources or unavailable write access.
    /// The host may disable updating and continue normal application startup.
    Unavailable(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => {
                formatter.write_str("An update is using this installation; try again shortly")
            }
            Self::Unavailable(error) => formatter.write_str(error),
        }
    }
}
impl std::error::Error for OpenError {}

#[derive(Clone, Debug)]
/// Shared installation-scoped lease. Clones retain the lease; competing installs
/// are refused, not coordinated or terminated. No application lifecycle authority.
pub struct Installation {
    root: PathBuf,
    config: Config,
    lease: Arc<File>,
}

impl Installation {
    /// Validates the installed bundle's configured identity/version/executable and
    /// acquires a shared lease beside it. Returns errors for ambiguous locations,
    /// foreign metadata, missing parents, permissions or exclusive contention.
    /// The caller supplies the canonical root of a marker-validated installation.
    pub fn open(config: &Config, root: PathBuf) -> Result<Self, OpenError> {
        validate_installed(config, &root).map_err(OpenError::Unavailable)?;
        let parent = root
            .parent()
            .ok_or_else(|| OpenError::Unavailable("Application parent is missing".into()))?;
        let name = root
            .file_name()
            .ok_or_else(|| OpenError::Unavailable("Application name is missing".into()))?;
        let mut lock_name = name.to_os_string();
        lock_name.push(".update-lock");
        let path = parent.join(lock_name);
        let lease = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| {
                OpenError::Unavailable(format!("Cannot reserve app update access: {e}"))
            })?;
        lease.try_lock_shared().map_err(|e| match e {
            fs::TryLockError::WouldBlock => OpenError::Busy,
            fs::TryLockError::Error(error) => {
                OpenError::Unavailable(format!("Cannot reserve app update access: {error}"))
            }
        })?;
        Ok(Self {
            root,
            config: config.clone(),
            lease: Arc::new(lease),
        })
    }
}

/// Owned same-filesystem staging, backup reservation and exclusive install lease.
/// Drop cancels preparation without replacing the installed bundle.
pub struct Prepared {
    installation: Installation,
    staging: tempfile::TempDir,
    backup: Option<tempfile::TempDir>,
    exclusive: bool,
}

fn replace(staged: &Path, destination: &Path, backup: &Path) -> Result<(), String> {
    self_update::Move::from_source(staged)
        .replace_using_temp(backup)
        .to_dest(destination)
        .map_err(|e| format!("Bundle replacement failed; previous app or backup was retained: {e}"))
}

impl Prepared {
    /// Replaces the complete bundle and spawns its new executable with native
    /// arguments. Call only after explicit host authorization and UI shutdown.
    /// Handled replacement failures retain recovery material; power-loss recovery
    /// and automatic post-launch rollback are not provided. A returned child only
    /// proves spawn succeeded, not that the new application became healthy.
    pub fn commit(
        mut self,
        arguments: &[std::ffi::OsString],
    ) -> Result<std::process::Child, String> {
        let staged = self
            .staging
            .path()
            .join(&self.installation.config.bundle_name);
        let backup = self.backup.as_ref().ok_or("Update backup is unavailable")?;
        if let Err(error) = replace(
            &staged,
            &self.installation.root,
            &backup.path().join(&self.installation.config.bundle_name),
        ) {
            return Err(self.retain_recovery(error));
        }
        self.relaunch(arguments)
            .map_err(|error| self.retain_recovery(error))
    }

    fn retain_recovery(&mut self, error: String) -> String {
        let recovery = self.backup.take().map(|backup| backup.keep());
        format!(
            "{error}. Recovery directory: {}",
            recovery
                .as_deref()
                .unwrap_or(Path::new("unavailable"))
                .display()
        )
    }

    fn relaunch(
        &mut self,
        arguments: &[std::ffi::OsString],
    ) -> Result<std::process::Child, String> {
        self.installation
            .lease
            .unlock()
            .map_err(|e| e.to_string())?;
        self.exclusive = false;
        let child = Command::new(
            self.installation
                .root
                .join("Contents/MacOS")
                .join(&self.installation.config.executable),
        )
        .args(arguments)
        .env_remove("KEDRA_ASSETS")
        .current_dir(
            self.installation
                .root
                .parent()
                .ok_or("Application parent is missing")?,
        )
        .spawn()
        .map_err(|e| format!("Update installed, but relaunch failed: {e}"))?;
        Ok(child)
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if self.exclusive {
            let _ = self.installation.lease.try_lock_shared();
        }
    }
}

fn run(command: &mut Command) -> Result<(), String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot start {program}: {e}"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "{program} failed ({status}); the installed app was not changed"
                ))
            };
        }
        if start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "{program} timed out; the installed app was not changed"
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

struct Mounted {
    directory: Option<tempfile::TempDir>,
}
impl Mounted {
    fn detach(&mut self) -> Result<(), String> {
        if let Some(directory) = &self.directory {
            run(Command::new("/usr/bin/hdiutil")
                .arg("detach")
                .arg("-quiet")
                .arg(directory.path()))?;
        }
        self.directory = None;
        Ok(())
    }
}
impl Drop for Mounted {
    fn drop(&mut self) {
        if self.detach().is_err()
            && let Some(directory) = self.directory.take()
        {
            // Never recurse into a still-mounted disk image during temporary cleanup.
            let _ = directory.keep();
        }
    }
}

fn validate_identity(config: &Config, root: &Path, version: &str) -> Result<(), String> {
    let mut bytes = Vec::new();
    File::open(root.join("Contents/Info.plist"))
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Update bundle Info.plist is too large".into());
    }
    let info = plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let info = info
        .as_dictionary()
        .ok_or("Update bundle metadata must be a dictionary")?;
    let value = |key| info.get(key).and_then(plist::Value::as_string);
    if value("CFBundleIdentifier") != Some(config.bundle_identifier.as_str())
        || value("CFBundleShortVersionString") != Some(version)
        || value("CFBundleExecutable") != Some(config.executable.as_str())
    {
        return Err(
            "Application bundle identity/version does not match the expected application".into(),
        );
    }
    Ok(())
}

pub(super) fn validate_installed(config: &Config, root: &Path) -> Result<(), String> {
    if !root.is_absolute()
        || root.canonicalize().map_err(|e| e.to_string())? != root
        || root.extension() != Some(std::ffi::OsStr::new("app"))
        || root
            .components()
            .any(|c| c.as_os_str() == "Volumes" || c.as_os_str() == "AppTranslocation")
    {
        return Err("Installed bundle location is unsupported or ambiguous".into());
    }
    let executable = root.join("Contents/MacOS").join(&config.executable);
    if !fs::symlink_metadata(&executable)
        .map_err(|e| e.to_string())?
        .is_file()
        || executable.canonicalize().map_err(|e| e.to_string())? != executable
    {
        return Err("Installed executable identity is ambiguous".into());
    }
    validate_identity(config, root, &config.current.to_string())
}

fn validate_bundle(config: &Config, root: &Path, version: &str) -> Result<(), String> {
    validate_identity(config, root, version)?;
    let mut total = 0_u64;
    let mut count = 0_u32;
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| e.to_string())?;
        count += 1;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        total = total.saturating_add(metadata.len());
        if count > 50000 || total > 4 * 1024 * 1024 * 1024_u64 {
            return Err("Update bundle exceeds installation limits".into());
        }
        if entry.file_type().is_symlink() {
            if !entry
                .path()
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&canonical)
            {
                return Err("Update bundle contains an escaping symbolic link".into());
            }
        } else if !entry.file_type().is_file() && !entry.file_type().is_dir() {
            return Err("Update bundle contains an unsupported filesystem entry".into());
        }
    }
    if !root
        .join("Contents/MacOS")
        .join(&config.executable)
        .is_file()
        || !root.join("Contents/Resources/assets").is_dir()
        || !root.join("Contents/Resources/licenses").is_dir()
    {
        return Err("Update bundle is missing its executable, assets or notices".into());
    }
    Ok(())
}

fn arm64_executable(path: &Path) -> Result<(), String> {
    use object::read::macho::MachHeader;
    let mut bytes = [0_u8; 32];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|e| e.to_string())?;
    let header = object::macho::MachHeader64::<object::Endianness>::parse(bytes.as_slice(), 0)
        .map_err(|e| e.to_string())?;
    let endian = header.endian().map_err(|e| e.to_string())?;
    if header.cputype(endian) != object::macho::CPU_TYPE_ARM64
        || header.cpusubtype(endian) != object::macho::CPU_SUBTYPE_ARM64_ALL.into()
        || header.filetype(endian) != object::macho::MH_EXECUTE
    {
        return Err("Update executable is not a supported ARM64 Mach-O application".into());
    }
    Ok(())
}

/// Revalidates a downloaded DMG, mounts it read-only, validates configured bundle
/// identity/version/ARM64/resources and stages it beside the installation.
/// Returns errors without authorizing host shutdown or replacing the installed app.
/// Requires native macOS hdiutil/ditto; no elevation, signing bypass or SDK tools.
pub fn prepare(
    installation: Installation,
    package: &Path,
    release: &Release,
) -> Result<Prepared, String> {
    download::reverify(package, release)?;
    let parent = installation
        .root
        .parent()
        .ok_or("Application parent is missing")?;
    let staging = tempfile::Builder::new()
        .prefix(".kedra-update-")
        .tempdir_in(parent)
        .map_err(|e| {
            format!("Cannot stage update next to the app; write permission is required: {e}")
        })?;
    let backup = tempfile::Builder::new()
        .prefix(".kedra-previous-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    if installation.lease.try_lock().is_err() {
        installation
            .lease
            .try_lock_shared()
            .map_err(|e| e.to_string())?;
        return Err("Close other Kedra instances using this installation before updating".into());
    }
    let prepared = Prepared {
        installation,
        staging,
        backup: Some(backup),
        exclusive: true,
    };
    let directory = tempfile::Builder::new()
        .prefix("kedra-update-image-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    run(Command::new("/usr/bin/hdiutil")
        .args(["attach", "-readonly", "-nobrowse", "-quiet", "-mountpoint"])
        .arg(directory.path())
        .arg(package))?;
    let mut mount = Mounted {
        directory: Some(directory),
    };
    let root = mount
        .directory
        .as_ref()
        .ok_or("Update image was retired")?
        .path()
        .join(&prepared.installation.config.bundle_name);
    validate_bundle(
        &prepared.installation.config,
        &root,
        &release.version.to_string(),
    )?;
    arm64_executable(
        &root
            .join("Contents/MacOS")
            .join(&prepared.installation.config.executable),
    )?;
    let staged = prepared
        .staging
        .path()
        .join(&prepared.installation.config.bundle_name);
    run(Command::new("/usr/bin/ditto").arg(&root).arg(&staged))?;
    validate_bundle(
        &prepared.installation.config,
        &staged,
        &release.version.to_string(),
    )?;
    mount.detach()?;
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn installed_fixture(root: &Path) -> TestResult {
        fs::create_dir_all(root.join("Contents/MacOS"))?;
        fs::create_dir_all(root.join("Contents/Resources/assets"))?;
        fs::create_dir_all(root.join("Contents/Resources/licenses"))?;
        fs::write(root.join("Contents/MacOS/kedra"), b"fixture")?;
        let info: plist::Value = serde_json::from_value(serde_json::json!({
            "CFBundleIdentifier":"dev.kedra.editor", "CFBundleExecutable":"kedra",
            "CFBundleShortVersionString":"0.1.0"
        }))?;
        info.to_file_xml(root.join("Contents/Info.plist"))?;
        Ok(())
    }

    #[test]
    fn foreign_installed_bundle_cannot_obtain_replacement_capability() -> TestResult {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("Kedra.app");
        installed_fixture(&root)?;
        let config = crate::config::fixture();
        Installation::open(&config, root.canonicalize()?)?;
        for (key, value) in [
            ("CFBundleIdentifier", "org.foreign.app"),
            ("CFBundleExecutable", "foreign"),
            ("CFBundleShortVersionString", "9.0.0"),
        ] {
            installed_fixture(&root)?;
            let path = root.join("Contents/Info.plist");
            let mut info = plist::Value::from_file(&path)?;
            info.as_dictionary_mut()
                .ok_or("Dictionary")?
                .insert(key.into(), value.into());
            info.to_file_xml(&path)?;
            assert!(
                Installation::open(&config, root.canonicalize()?).is_err(),
                "{key}"
            );
        }
        Ok(())
    }

    #[test]
    fn dropping_prepared_preserves_installation_and_removes_owned_staging() -> TestResult {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("Kedra.app");
        installed_fixture(&root)?;
        let installation = Installation::open(&crate::config::fixture(), root.canonicalize()?)?;
        // Portable ownership fixture, not a native installation transaction.
        // Actual shared/exclusive conversion is covered on macOS below.
        let staging = tempfile::tempdir_in(dir.path())?;
        let staging_path = staging.path().to_owned();
        fs::write(staging.path().join("owned"), b"staged")?;
        let backup = tempfile::tempdir_in(dir.path())?;
        let backup_path = backup.path().to_owned();
        drop(Prepared {
            installation,
            staging,
            backup: Some(backup),
            exclusive: false,
        });
        assert_eq!(fs::read(root.join("Contents/MacOS/kedra"))?, b"fixture");
        assert!(!staging_path.exists());
        assert!(!backup_path.exists());
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cancelled_preparation_restores_shared_installation_lease() -> TestResult {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("Kedra.app");
        installed_fixture(&root)?;
        let installation = Installation::open(&crate::config::fixture(), root.canonicalize()?)?;
        installation.lease.try_lock()?;
        let observer = installation.clone();
        drop(Prepared {
            installation,
            staging: tempfile::tempdir_in(dir.path())?,
            backup: Some(tempfile::tempdir_in(dir.path())?),
            exclusive: true,
        });
        let second = Installation::open(&crate::config::fixture(), root.canonicalize()?)?;
        assert!(
            observer.lease.try_lock().is_err(),
            "Cancellation must restore a shared lease"
        );
        drop(second);
        observer.lease.try_lock()?;
        Ok(())
    }

    #[test]
    fn commit_failure_retains_recovery_material_after_owner_drop() -> TestResult {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("Kedra.app");
        installed_fixture(&root)?;
        let installation = Installation::open(&crate::config::fixture(), root.canonicalize()?)?;
        let staging = tempfile::tempdir_in(dir.path())?;
        let staging_path = staging.path().to_owned();
        let backup = tempfile::tempdir_in(dir.path())?;
        let backup_path = backup.path().to_owned();
        fs::write(backup.path().join("recovery"), b"retained")?;
        let prepared = Prepared {
            installation,
            staging,
            backup: Some(backup),
            exclusive: false,
        };
        assert!(prepared.commit(&[]).is_err());
        assert_eq!(fs::read(root.join("Contents/MacOS/kedra"))?, b"fixture");
        assert_eq!(fs::read(backup_path.join("recovery"))?, b"retained");
        assert!(!staging_path.exists());
        Ok(())
    }

    #[test]
    fn immediate_relaunch_failure_retains_previous_bundle_for_manual_recovery() -> TestResult {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("Kedra.app");
        installed_fixture(&root)?;
        let installation = Installation::open(&crate::config::fixture(), root.canonicalize()?)?;
        let staging = tempfile::tempdir_in(dir.path())?;
        // Force spawn failure after replacement without executing any code.
        fs::create_dir(staging.path().join("Kedra.app"))?;
        let backup = tempfile::tempdir_in(dir.path())?;
        let backup_path = backup.path().to_owned();
        let prepared = Prepared {
            installation,
            staging,
            backup: Some(backup),
            exclusive: false,
        };
        let error = prepared
            .commit(&[])
            .err()
            .ok_or("Expected relaunch failure")?;
        assert!(error.contains("relaunch"));
        assert_eq!(
            fs::read(backup_path.join("Kedra.app/Contents/MacOS/kedra"))?,
            b"fixture"
        );
        assert!(error.contains(backup_path.to_string_lossy().as_ref()));
        Ok(())
    }

    #[test]
    fn configured_bundle_identity_is_required_before_staging() -> TestResult {
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path().join("Contents/MacOS"))?;
        fs::create_dir_all(dir.path().join("Contents/Resources/assets"))?;
        fs::create_dir_all(dir.path().join("Contents/Resources/licenses"))?;
        fs::write(dir.path().join("Contents/MacOS/studio"), b"fixture")?;
        let info: plist::Value = serde_json::from_value(serde_json::json!({
            "CFBundleIdentifier":"org.example.studio", "CFBundleExecutable":"studio",
            "CFBundleShortVersionString":"4.0.0"
        }))?;
        info.to_file_xml(dir.path().join("Contents/Info.plist"))?;
        let config = Config::new(
            "example/studio",
            "studio",
            "org.example.studio",
            "studio",
            "Studio.app",
            semver::Version::new(3, 0, 0),
        )?;
        validate_bundle(&config, dir.path(), "4.0.0")?;
        assert!(validate_bundle(&crate::config::fixture(), dir.path(), "4.0.0").is_err());
        assert!(validate_bundle(&config, dir.path(), "5.0.0").is_err());
        Ok(())
    }

    #[test]
    fn architecture_check_accepts_arm64_and_refuses_wrong_or_truncated_headers() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("kedra");
        let mut header = [0_u8; 32];
        header[..16].copy_from_slice(&[
            0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0,
        ]);
        fs::write(&path, header)?;
        arm64_executable(&path)?;
        header[4] = 7;
        fs::write(&path, header)?;
        assert!(arm64_executable(&path).is_err());
        fs::write(&path, b"not an executable")?;
        assert!(arm64_executable(&path).is_err());
        Ok(())
    }
    #[test]
    fn whole_bundle_replacement_updates_resources_and_keeps_the_previous_tree() -> TestResult {
        let dir = tempfile::tempdir()?;
        let old = dir.path().join("Kedra.app");
        let new = dir.path().join("new.app");
        let backup = dir.path().join("backup.app");
        for path in [&old, &new] {
            fs::create_dir_all(path.join("Contents/Resources"))?;
        }
        fs::write(old.join("Contents/Resources/version"), b"old")?;
        fs::write(new.join("Contents/Resources/version"), b"new")?;
        replace(&new, &old, &backup)?;
        assert_eq!(fs::read(old.join("Contents/Resources/version"))?, b"new");
        assert_eq!(fs::read(backup.join("Contents/Resources/version"))?, b"old");
        Ok(())
    }
    #[test]
    fn failed_replacement_preserves_old_bundle() -> TestResult {
        let dir = tempfile::tempdir()?;
        let old = dir.path().join("Kedra.app");
        fs::create_dir(&old)?;
        fs::write(old.join("kept"), b"original")?;
        assert!(
            replace(
                &dir.path().join("missing.app"),
                &old,
                &dir.path().join("backup.app")
            )
            .is_err()
        );
        assert_eq!(fs::read(old.join("kept"))?, b"original");
        Ok(())
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn installation_shared_leases_prevent_updating_another_open_editor() -> TestResult {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("Kedra.app");
        installed_fixture(&root)?;
        let first = Installation::open(&crate::config::fixture(), root.canonicalize()?)?;
        let second = Installation::open(&crate::config::fixture(), root.canonicalize()?)?;
        assert!(first.lease.try_lock().is_err());
        first.lease.try_lock_shared()?;
        drop(second);
        first.lease.try_lock()?;
        assert!(matches!(
            Installation::open(&crate::config::fixture(), root.canonicalize()?),
            Err(super::OpenError::Busy)
        ));
        first.lease.try_lock_shared()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "Creates and mounts a local DMG, replaces an isolated app fixture and relaunches it"]
    fn native_dmg_stage_replace_and_relaunch() -> TestResult {
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir()?;
        let installed = dir.path().join("installed/Kedra.app");
        let image_root = dir.path().join("image");
        let candidate = image_root.join("Kedra.app");
        for (bundle, version) in [(&installed, "0.1.0"), (&candidate, "0.2.0")] {
            fs::create_dir_all(bundle.join("Contents/MacOS"))?;
            fs::create_dir_all(bundle.join("Contents/Resources/assets"))?;
            fs::create_dir_all(bundle.join("Contents/Resources/licenses"))?;
            fs::write(bundle.join("Contents/Resources/version"), version)?;
            let info: plist::Value = serde_json::from_value(serde_json::json!({
                "CFBundleIdentifier":"dev.kedra.editor", "CFBundleExecutable":"kedra",
                "CFBundleShortVersionString":version
            }))?;
            info.to_file_xml(bundle.join("Contents/Info.plist"))?;
        }
        let image = dir.path().join("update.dmg");
        // Both versions are ordinary third-party executables. A copied Apple
        // platform executable carries launch constraints unrelated to app updates.
        let old_source = dir.path().join("old.rs");
        fs::write(
            &old_source,
            "fn main() { std::thread::sleep(std::time::Duration::from_secs(60)); }",
        )?;
        run(Command::new("rustc")
            .args(["--edition=2024", "--target", "aarch64-apple-darwin"])
            .arg(&old_source)
            .arg("-o")
            .arg(installed.join("Contents/MacOS/kedra")))?;
        let source = dir.path().join("relaunch.rs");
        fs::write(
            &source,
            r#"fn main() { if let Some(path) = std::env::args_os().nth(1) { std::fs::write(path, b"relaunched").expect("fixture receipt"); } }"#,
        )?;
        run(Command::new("rustc")
            .args(["--edition=2024", "--target", "aarch64-apple-darwin"])
            .arg(&source)
            .arg("-o")
            .arg(candidate.join("Contents/MacOS/kedra")))?;
        run(Command::new("/usr/bin/hdiutil")
            .args(["create", "-quiet", "-format", "UDZO", "-volname", "Kedra"])
            .arg("-srcfolder")
            .arg(&image_root)
            .arg(&image))?;
        let bytes = fs::read(&image)?;
        let release = Release {
            version: semver::Version::new(0, 2, 0),
            source_revision: "1".repeat(40),
            notes: String::new(),
            name: "update.dmg".into(),
            url: url::Url::parse(
                "https://github.com/kedra-engine/kedra/releases/download/v0.2.0/update.dmg",
            )?,
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        };
        struct Running(std::process::Child);
        impl Drop for Running {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let _running = Running(
            Command::new(installed.join("Contents/MacOS/kedra"))
                .arg("60")
                .spawn()?,
        );
        let installation =
            Installation::open(&crate::config::fixture(), installed.canonicalize()?)?;
        let prepared = prepare(installation, &image, &release)?;
        let receipt = dir.path().join("relaunch receipt Юникод");
        let mut relaunched = prepared.commit(&[receipt.clone().into_os_string()])?;
        assert_eq!(
            fs::read(installed.join("Contents/Resources/version"))?,
            b"0.2.0"
        );
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = relaunched.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                relaunched.kill()?;
                relaunched.wait()?;
                return Err("Relaunch fixture timed out".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(status.success(), "Relaunch failed: {status}");
        assert_eq!(fs::read(receipt)?, b"relaunched");
        Ok(())
    }
}
