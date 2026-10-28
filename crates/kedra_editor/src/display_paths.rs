//! User-facing source labels, never filesystem locators or persisted identities.
use std::path::{Component, Path};

/// Source inventory paths are relative to the project's assets directory.
/// Unexpected external paths degrade to a filename, never an absolute label.
pub(crate) fn asset(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        return "assets".into();
    }
    if path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        Path::new("assets").join(path).display().to_string()
    } else {
        path.file_name().map_or_else(
            || "Asset".into(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asset_labels_are_project_relative_unicode_and_not_filesystem_locators() {
        let root = std::env::current_dir().unwrap_or_else(|error| panic!("{error}"));
        let root = root.join("Private project").join("assets");
        let path = Path::new("Prefabs").join("日本語 Straße.prefab.ron");
        let expected = Path::new("assets").join(&path).display().to_string();
        assert_eq!(asset(&path), expected);
        assert_eq!(asset(&root.join(&path)), "日本語 Straße.prefab.ron");
        assert_eq!(asset(Path::new("../outside.png")), "outside.png");
        assert_eq!(asset(Path::new("")), "assets");
        assert!(!Path::new(&expected).is_absolute());
    }
}
