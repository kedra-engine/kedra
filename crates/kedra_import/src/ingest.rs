//! Explicit external-file ingestion into authoring sources, independent of UI.
//! Uses WalkDir, tempfile's no-clobber publication and the ordinary metadata initializer.
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

/// Create one validated environment source through the ordinary staged import
/// publisher and metadata initializer. The parent must exist; neither an existing
/// source nor sidecar is replaced. Publication has the same per-file interruption
/// semantics as `import_files`; no runtime delivery is built. Call on a worker.
pub fn create_environment(root: &Path, directory: &Path, name: &str) -> Result<usize, String> {
    kedra_authoring_fs::entries::name(name)?;
    let name = if name.ends_with(".environment.ron") {
        name.to_owned()
    } else if name.ends_with(".ron") {
        return Err("Use a name ending in .environment.ron".into());
    } else {
        format!("{name}.environment.ron")
    };
    kedra_authoring_fs::entries::name(&name)?;
    let value = kedra_scene::environment::Environment {
        version: 1,
        background: bevy::prelude::Color::srgb(0.1, 0.1, 0.1),
        ambient_color: bevy::prelude::Color::WHITE,
        ambient_brightness: 100.0,
    };
    value.validate().map_err(|e| e.to_string())?;
    let text = ron::ser::to_string_pretty(&value, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = stage.path().join(name);
    fs::write(&path, text).map_err(|e| e.to_string())?;
    import_files(root, directory, &[path], |_| {})
}

/// Import regular files or folder trees into an existing project-relative directory.
/// Original files are never moved. Existing destination files/metadata are never replaced.
/// Incoming `.meta` files are excluded: new assets receive new identities. Preparation
/// (including metadata initialization) completes before publication. Publication is per-file,
/// not a multi-file transaction; an I/O failure reports the number already published.
/// Static symlinks/special files and nested/duplicate destinations refuse. This is not a
/// sandbox against concurrent filesystem replacement. `progress` runs on the caller thread.
/// Returns the number of imported source files. Does not build a runtime delivery.
pub fn import_files(
    root: &Path,
    directory: &Path,
    inputs: &[PathBuf],
    mut progress: impl FnMut(&str),
) -> Result<usize, String> {
    checked_directory(root, directory)?;
    if inputs.is_empty() {
        return Ok(0);
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let destination = root.join(directory);
    let mut files = BTreeMap::new();
    for input in inputs {
        let name = input.file_name().ok_or("Import source has no filename")?;
        for entry in walkdir::WalkDir::new(input).follow_links(false) {
            let entry = entry.map_err(|e| {
                format!(
                    "Cannot read import source: {}",
                    e.io_error()
                        .map_or_else(|| "directory traversal failed".into(), ToString::to_string)
                )
            })?;
            if entry.file_type().is_symlink()
                || !(entry.file_type().is_file() || entry.file_type().is_dir())
            {
                return Err(
                    "Import requires regular files and folders; links are not supported".into(),
                );
            }
            if entry.file_type().is_dir() {
                continue;
            }
            if entry.path().extension().is_some_and(|e| e == "meta") {
                continue;
            }
            let suffix = entry
                .path()
                .strip_prefix(input)
                .map_err(|e| e.to_string())?;
            let relative = if suffix.as_os_str().is_empty() {
                PathBuf::from(name)
            } else {
                PathBuf::from(name).join(suffix)
            };
            let text = relative
                .to_str()
                .ok_or("Asset filenames must be Unicode")?
                .replace(std::path::MAIN_SEPARATOR, "/");
            if !kedra_assets::delivery::normalized(&text) {
                return Err(format!("Invalid asset path: {text}"));
            }
            if entry
                .path()
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&root)
            {
                return Err("Choose files outside this project's assets".into());
            }
            check_destination(&destination, &relative)?;
            if files.insert(relative, entry.path().to_owned()).is_some() {
                return Err("Several import sources have the same destination".into());
            }
            if files.len() > 4096 {
                return Err("Import exceeds 4096 files".into());
            }
        }
    }
    let planned: std::collections::BTreeSet<_> = files
        .keys()
        .flat_map(|path| [path.clone(), sidecar(path)])
        .collect();
    for relative in &planned {
        if relative
            .ancestors()
            .skip(1)
            .any(|parent| planned.contains(parent))
        {
            return Err("An imported file conflicts with an imported directory".into());
        }
    }
    // Stage outside the asset tree so listing/admission cannot see partial preparation.
    let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
    for (relative, input) in &files {
        let output = stage.path().join(relative);
        fs::create_dir_all(output.parent().ok_or("Missing staging directory")?)
            .map_err(|e| e.to_string())?;
        progress(&format!("Preparing {}", relative.display()));
        let mut source = fs::File::open(input).map_err(|e| e.to_string())?;
        let mut staged = fs::File::create_new(output).map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut staged).map_err(|e| e.to_string())?;
    }
    crate::initialize(stage.path(), |_| {})
        .map_err(|e| e.replace(stage.path().to_string_lossy().as_ref(), "<import>"))?;
    let mut count = 0;
    let result = (|| {
        for relative in files.keys() {
            checked_directory(&root, directory)?;
            check_destination(&destination, relative)?;
            let target = destination.join(relative);
            let parent = target.parent().ok_or("Missing destination directory")?;
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            checked_directory(
                &root,
                parent.strip_prefix(&root).map_err(|e| e.to_string())?,
            )?;
            // Source first: a failed metadata publication leaves an explicitly untracked
            // source, never a sidecar that could claim another file's identity.
            publish(&stage.path().join(relative), &target)?;
            count += 1;
            publish(&sidecar(&stage.path().join(relative)), &sidecar(&target))?;
            progress(&format!("Imported {}", relative.display()));
        }
        Ok(count)
    })();
    result.map_err(|error: String| format!("Import stopped after {count} source file(s): {error}"))
}

/// Validate an existing destination directory without following static links.
/// Relative paths may be empty (the asset root); traversal and absolute paths refuse.
pub fn checked_directory(root: &Path, relative: &Path) -> Result<(), String> {
    let mut path = root.to_owned();
    if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
        return Err("Asset root is unavailable or linked".into());
    }
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("Import destination must be project-relative".into());
        };
        path.push(name);
        if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
            return Err("Import destination is unavailable or linked".into());
        }
    }
    Ok(())
}

fn sidecar(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".meta");
    name.into()
}
fn check_destination(root: &Path, relative: &Path) -> Result<(), String> {
    let mut path = root.to_owned();
    for part in relative.parent().into_iter().flat_map(Path::components) {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => (),
            Ok(_) => return Err("Import destination ancestor is not a regular directory".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    for path in [root.join(relative), sidecar(&root.join(relative))] {
        match fs::symlink_metadata(path) {
            Ok(_) => return Err(format!("Asset already exists: {}", relative.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
fn publish(source: &Path, target: &Path) -> Result<(), String> {
    let mut temporary =
        tempfile::NamedTempFile::new_in(target.parent().ok_or("Missing destination")?)
            .map_err(|e| e.to_string())?;
    let mut source = fs::File::open(source).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut temporary).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary
        .persist_noclobber(target)
        .map_err(|e| e.error.to_string())?;
    Ok(())
}
