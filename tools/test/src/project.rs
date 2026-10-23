//! One checked source project, copied without generation or identity changes.
use kedra_test::{Result, evidence};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub fn inventory(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    let mut result = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            return Err("Unexpected project source symlink".into());
        }
        if entry.file_type().is_file() {
            result.insert(
                entry.path().strip_prefix(root)?.into(),
                evidence::file_digest(entry.path())?,
            );
        }
    }
    Ok(result)
}

pub fn source(root: &Path) -> PathBuf {
    root.join("tests/fixtures/test_project")
}

pub fn copy(root: &Path, destination: &Path) -> Result<PathBuf> {
    let source = source(root);
    if !source.join("kedra.toml").is_file() || !source.join("assets").is_dir() {
        return Err("The checked test_project is incomplete".into());
    }
    let before = inventory(&source)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(destination)?;
    eprintln!("Copying shared test project → {}", destination.display());
    for entry in walkdir::WalkDir::new(&source)
        .follow_links(false)
        .min_depth(1)
    {
        let entry = entry?;
        let target = destination.join(entry.path().strip_prefix(&source)?);
        if entry.file_type().is_symlink() {
            return Err("Shared test project must not contain symlinks".into());
        } else if entry.file_type().is_dir() {
            fs::create_dir(&target)?;
        } else if entry.file_type().is_file() {
            fs::write(&target, evidence::read(entry.path(), 64 * 1024 * 1024)?)?;
        } else {
            return Err("Shared test project contains a nonregular source".into());
        }
    }
    if inventory(&source)? != before || inventory(destination)? != before {
        return Err("Shared project changed during copy or copied bytes differ".into());
    }
    Ok(destination.canonicalize()?)
}

pub fn delivery(root: &Path, output: &Path, editor: &Path) -> Result<PathBuf> {
    let project = copy(root, &output.join("private/delivery-source"))?;
    let delivery = output.join("private/project-delivery");
    crate::command(
        editor,
        &[
            "assets".as_ref(),
            "build".as_ref(),
            project.join("assets").as_os_str(),
            delivery.as_os_str(),
            "--namespace".as_ref(),
            "game".as_ref(),
        ],
        root,
    )?;
    crate::command(
        editor,
        &["assets".as_ref(), "verify".as_ref(), delivery.as_os_str()],
        root,
    )?;
    Ok(delivery)
}
