//! Inspect actual distributed contents before a successful inventory is published.
use crate::config::{Application, NativeTarget};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) fn verify(
    app: &Application,
    stage: &Path,
    artifacts: &[(PathBuf, &'static str)],
) -> Result<(), String> {
    eprintln!("Verifying actual native and portable package contents");
    for (path, format) in artifacts {
        let work = tempfile::Builder::new()
            .prefix("kedra inspection Юникод ")
            .tempdir()
            .map_err(|e| e.to_string())?;
        match *format {
            "dmg" => dmg(app, stage, path, work.path())?,
            "zip" | "tar.gz" => {
                if *format == "zip" {
                    unzip(path, work.path())?;
                } else {
                    untar(path, work.path())?;
                }
                let root = work.path().join(app.package.name.as_str());
                let binary = root.join(format!(
                    "{}{}",
                    app.binary,
                    if app.target == NativeTarget::Windows {
                        ".exe"
                    } else {
                        ""
                    }
                ));
                installed(app, stage, &root, &binary, &root.join("assets"))?;
            }
            "nsis" => nsis(app, stage, path, work.path())?,
            // Creation already extracts and validates this exact image once in
            // native_licenses::verify, including generic installed resources.
            "appimage" => {}
            _ => return Err(format!("Unsupported distribution format: {format}")),
        }
    }
    Ok(())
}

pub(crate) fn installed(
    app: &Application,
    stage: &Path,
    root: &Path,
    binary: &Path,
    assets: &Path,
) -> Result<(), String> {
    if !binary
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(root.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("Installed executable escaped package output".into());
    }
    crate::verify::executable(binary, app.target)?;
    let notices = assets
        .parent()
        .ok_or("Installed resource parent missing")?
        .join("licenses");
    crate::verify::resources(app, assets, &notices)?;
    same_tree(&stage.join("assets"), assets)?;
    same_tree(&stage.join("licenses"), &notices)?;
    Ok(())
}

pub(crate) fn same_tree(expected: &Path, actual: &Path) -> Result<(), String> {
    let entries = |root: &Path| -> Result<std::collections::BTreeMap<PathBuf, [u8; 32]>, String> {
        use sha2::{Digest, Sha256};
        let mut result = std::collections::BTreeMap::new();
        for entry in walkdir::WalkDir::new(root).min_depth(1) {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_path_buf();
            let hash = if entry.file_type().is_file() {
                let mut file = fs::File::open(entry.path()).map_err(|e| e.to_string())?;
                let mut digest = Sha256::new();
                std::io::copy(&mut file, &mut digest).map_err(|e| e.to_string())?;
                digest.finalize().into()
            } else if entry.file_type().is_dir() {
                continue;
            } else {
                return Err(
                    "Installed resources must not contain symbolic links or special files".into(),
                );
            };
            result.insert(path, hash);
        }
        Ok(result)
    };
    if entries(expected)? != entries(actual)? {
        return Err(format!(
            "Installed resources differ from verified staging: {}",
            actual.display()
        ));
    }
    Ok(())
}

pub(crate) fn unzip(archive: &Path, output: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(archive).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let relative = entry
            .enclosed_name()
            .ok_or("ZIP member escaped inspection directory")?;
        if entry.is_symlink() {
            return Err("ZIP symbolic links are not supported".into());
        }
        let path = output.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            fs::create_dir_all(path.parent().ok_or("ZIP member parent missing")?)
                .map_err(|e| e.to_string())?;
            let mut file = fs::File::create_new(&path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn untar(archive: &Path, output: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
        fs::File::open(archive).map_err(|e| e.to_string())?,
    ));
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        if !entry.header().entry_type().is_file() && !entry.header().entry_type().is_dir() {
            return Err("Portable archive contains unsupported links or special files".into());
        }
        if !entry.unpack_in(output).map_err(|e| e.to_string())? {
            return Err("Archive member escaped inspection directory".into());
        }
    }
    Ok(())
}

fn dmg(app: &Application, stage: &Path, image: &Path, work: &Path) -> Result<(), String> {
    run(Command::new("hdiutil").arg("verify").arg(image))?;
    let mount = work.join("readonly mount");
    fs::create_dir(&mount).map_err(|e| e.to_string())?;
    run(Command::new("hdiutil")
        .args(["attach", "-readonly", "-nobrowse", "-mountpoint"])
        .arg(&mount)
        .arg(image))?;
    let mut guard = MountedImage(Some(mount.clone()));
    let result = (|| {
        let name = format!("{}.app", app.native.product_name);
        let root = work.join(&name);
        run(Command::new("/usr/bin/ditto")
            .arg(mount.join(name))
            .arg(&root))?;
        crate::verify::macos(app, &root)?;
        installed(
            app,
            stage,
            &root,
            &root.join("Contents/MacOS").join(&app.binary),
            &root.join("Contents/Resources/assets"),
        )
    })();
    let detach = run(Command::new("hdiutil").arg("detach").arg(&mount));
    if detach.is_ok() {
        guard.0 = None;
    }
    result.and(detach)
}

struct MountedImage(Option<PathBuf>);
impl Drop for MountedImage {
    fn drop(&mut self) {
        if let Some(path) = &self.0
            && let Err(error) = run(Command::new("hdiutil").arg("detach").arg(path))
        {
            eprintln!(
                "FAILED: cleanup could not detach {}: {error}",
                path.display()
            );
        }
    }
}

fn nsis(app: &Application, stage: &Path, installer: &Path, work: &Path) -> Result<(), String> {
    nsis_with_tool(app, stage, installer, work, std::ffi::OsStr::new("7z"))
}

pub(crate) fn nsis_with_tool(
    app: &Application,
    stage: &Path,
    installer: &Path,
    work: &Path,
    tool: &std::ffi::OsStr,
) -> Result<(), String> {
    let root = work.join("installed Юникод space");
    let mut destination = std::ffi::OsString::from("-o");
    destination.push(&root);
    // Never execute an installer during ordinary local packaging: /D= does not
    // isolate its registry, shortcuts, upgrade handling or process termination.
    run(Command::new(tool)
        .args(["x", "-y"])
        .arg(destination)
        .arg(installer))?;
    let binary = root.join(format!("{}.exe", app.binary));
    installed(app, stage, &root, &binary, &root.join("assets"))
}

fn run(command: &mut Command) -> Result<(), String> {
    eprintln!("Running {command:?}");
    let mut child = command
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("Native package inspection failed: {status}"))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Native package inspection timed out".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
