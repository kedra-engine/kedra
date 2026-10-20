use crate::{FORMAT_VERSION, MANIFEST_FILE, Project, ProjectError, ProjectManifest, io};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
};

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Static input preflight, not a sandbox against concurrent path replacement.
/// Missing files retain their I/O kind so history can distinguish first launch.
pub(crate) fn open_regular(path: &Path, action: &str) -> Result<fs::File, ProjectError> {
    let validate = |metadata: fs::Metadata| {
        if metadata.is_file() {
            Ok(())
        } else {
            Err(ProjectError::Invalid(format!(
                "{action}: expected a regular file at {}.",
                path.display()
            )))
        }
    };
    validate(fs::metadata(path).map_err(|e| io(action, e))?)?;
    let file = fs::File::open(path).map_err(|e| io(action, e))?;
    validate(file.metadata().map_err(|e| io(action, e))?)?;
    Ok(file)
}

pub(super) fn open(root: &Path) -> Result<Project, ProjectError> {
    let root = root
        .canonicalize()
        .map_err(|e| io("Cannot access project directory", e))?;
    let path = root.join(MANIFEST_FILE);
    let file = open_regular(&path, "Cannot read kedra.toml in the selected directory")?;
    let mut source = String::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut source)
        .map_err(|e| io("Cannot read project manifest as UTF-8", e))?;
    if source.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ProjectError::Invalid(
            "Project manifest exceeds 64 KiB.".into(),
        ));
    }
    // Read the format before deserializing a version-specific schema.
    let document: toml::Table = toml::from_str(&source)
        .map_err(|e| ProjectError::Invalid(format!("Invalid project manifest: {e}")))?;
    let format = document
        .get("format")
        .and_then(toml::Value::as_integer)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| {
            ProjectError::Invalid("Project manifest needs a nonnegative format number.".into())
        })?;
    if format != FORMAT_VERSION {
        return Err(ProjectError::UnsupportedFormat {
            found: format,
            supported: FORMAT_VERSION,
        });
    }
    let manifest: ProjectManifest = toml::from_str(&source)
        .map_err(|e| ProjectError::Invalid(format!("Invalid project manifest: {e}")))?;
    crate::validate_name(&manifest.name)?;
    if manifest.id.is_nil() || manifest.editor_version.trim().is_empty() {
        return Err(ProjectError::Invalid(
            "Project identity and editor version must not be empty.".into(),
        ));
    }
    if manifest.assets.as_os_str().is_empty()
        || manifest
            .assets
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(ProjectError::Invalid(
            "Asset directory must be a relative path inside the project.".into(),
        ));
    }
    let assets = root
        .join(&manifest.assets)
        .canonicalize()
        .map_err(|e| io("Cannot access project asset directory", e))?;
    if !assets.starts_with(&root) || !assets.is_dir() {
        return Err(ProjectError::Invalid(
            "Asset directory must stay inside the project.".into(),
        ));
    }
    Ok(Project {
        root,
        assets,
        manifest,
    })
}

pub(super) fn create(parent: &Path, name: &str) -> Result<Project, ProjectError> {
    let parent = parent
        .canonicalize()
        .map_err(|e| io("Choose an existing parent directory", e))?;
    let root = parent.join(name);
    fs::create_dir(&root).map_err(|e| {
        io(
            "Cannot create project directory (it must not already exist)",
            e,
        )
    })?;
    let assets = root.join("assets");
    let result = (|| {
        fs::create_dir(&assets).map_err(|e| io("Cannot create asset directory", e))?;
        let manifest = ProjectManifest {
            id: uuid::Uuid::new_v4(),
            format: FORMAT_VERSION,
            name: name.into(),
            editor_version: crate::EDITOR_VERSION.into(),
            assets: "assets".into(),
        };
        let source = toml::to_string_pretty(&manifest)
            .map_err(|e| ProjectError::Invalid(format!("Cannot encode project manifest: {e}")))?;
        write_new(&root.join(MANIFEST_FILE), source.as_bytes())?;
        // No fallible operation follows publication: success owns this manifest.
        Ok(Project {
            root: root.clone(),
            assets: assets.clone(),
            manifest,
        })
    })();
    if result.is_err() {
        // These remove only empty owned directories; never recursively delete.
        let _ = fs::remove_dir(&assets);
        let _ = fs::remove_dir(&root);
    }
    result
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    let parent = path
        .parent()
        .ok_or_else(|| ProjectError::Invalid("Missing project parent.".into()))?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| io("Cannot prepare manifest", e))?;
    temporary
        .write_all(bytes)
        .map_err(|e| io("Cannot write manifest", e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| io("Cannot synchronize manifest", e))?;
    temporary
        .persist_noclobber(path)
        .map_err(|e| io("Cannot publish manifest without overwriting", e.error))?;
    Ok(())
}
