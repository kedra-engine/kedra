//! File growth policy, using Git's file inventory rather than a custom walker.
use std::{fs, path::Path, process::Command};

const MAX_LINES: usize = 1000;
const MAX_BYTES: u64 = 64 * 1024;
// Exact upstream/generated artifacts, not directories or source-file exemptions.
const EXEMPT: &[&str] = &["Cargo.lock", "LICENSE-MIT", "LICENSE-APACHE"];

pub(crate) fn check() -> crate::Result<String> {
    check_at(Path::new("."))
}

fn git_paths(root: &Path, args: &[&str]) -> crate::Result<std::collections::BTreeSet<String>> {
    let output = Command::new("git").current_dir(root).args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "Git file inventory failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let inventory = std::str::from_utf8(&output.stdout)?;
    Ok(inventory
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

fn check_at(root: &Path) -> crate::Result<String> {
    let paths = git_paths(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    let deleted = git_paths(root, &["ls-files", "--deleted", "-z"])?;
    let input: Vec<u8> = paths
        .iter()
        .flat_map(|path| path.bytes().chain([0]))
        .collect();
    let attributes = duct::cmd("git", ["check-attr", "-z", "--stdin", "filter"])
        .dir(root)
        .stdin_bytes(input)
        .read()?;
    let fields: Vec<_> = attributes.split_terminator('\0').collect();
    let lfs: std::collections::BTreeSet<_> = fields
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|entry| entry[1] == "filter" && entry[2] == "lfs")
        .map(|entry| entry[0])
        .collect();
    let mut failures = Vec::new();
    for path in &paths {
        if EXEMPT.contains(&path.as_str()) {
            continue;
        }
        let info = match fs::symlink_metadata(root.join(path)) {
            Ok(info) => info,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && deleted.contains(path) =>
            {
                continue;
            }
            Err(error) => {
                failures.push(format!("{path}: cannot inspect file: {error}"));
                continue;
            }
        };
        if !info.is_file() {
            failures.push(format!(
                "{path}: symlinks/submodules require a reviewed policy"
            ));
            continue;
        }
        if info.len() > MAX_BYTES && !lfs.contains(path.as_str()) {
            failures.push(format!("{path}: {} bytes exceeds {MAX_BYTES}", info.len()));
            continue;
        }
        let bytes = match fs::read(root.join(path)) {
            Ok(bytes) => bytes,
            Err(error) => {
                failures.push(format!("{path}: cannot read file: {error}"));
                continue;
            }
        };
        if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n") {
            failures.push(format!("{path}: unhydrated LFS pointer; run git lfs pull"));
            continue;
        }
        // Binary LFS objects are size-independent. UTF-8 sources retain the
        // authored-text limits even when an overly broad attribute matches them.
        if std::str::from_utf8(&bytes).is_err() {
            if !lfs.contains(path.as_str()) {
                failures.push(format!("{path}: binary assets must use Git LFS"));
            }
            continue;
        }
        if let Err(error) = validate(path, &bytes) {
            failures.push(error);
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("\n").into());
    }
    Ok(format!(
        "File size policy passed for {} repository file(s).",
        paths.len()
    ))
}

fn validate(path: &str, bytes: &[u8]) -> std::result::Result<(), String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(format!("{path}: {} bytes exceeds {MAX_BYTES}", bytes.len()));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        let lines = text.lines().count();
        if lines > MAX_LINES {
            return Err(format!(
                "{path}: {lines} lines exceeds {MAX_LINES}; split by responsibility"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_limit_counts_comments_blank_lines_and_missing_final_newline() {
        assert!(validate("code.rs", "// comment\n".repeat(MAX_LINES).as_bytes()).is_ok());
        assert!(validate("code.rs", "\n".repeat(MAX_LINES + 1).as_bytes()).is_err());
        let content = format!("{}last", "a\n".repeat(MAX_LINES));
        assert!(validate("code.rs", content.as_bytes()).is_err());
        assert!(validate("code.rs", "a\r\n".repeat(MAX_LINES).as_bytes()).is_ok());
    }

    #[test]
    fn byte_limit_catches_compressed_source_and_binary_files() {
        assert!(validate("code.rs", &vec![b'a'; 65536]).is_ok());
        assert!(validate("code.rs", &vec![b'a'; 65537]).is_err());
        assert!(validate("asset.bin", &vec![255; 65537]).is_err());
        assert!(validate("asset.bin", &[255]).is_ok());
        assert!(validate("notes.md", "é".repeat(32769).as_bytes()).is_err());
    }

    #[test]
    fn pngs_have_no_path_or_extension_exemption() {
        for path in [
            "docs/ui/corpus/oversized.png",
            "docs/ui/corpus/text/button.full.png",
            "asset.png",
        ] {
            assert!(validate(path, &vec![255; MAX_BYTES as usize + 1]).is_err());
            assert!(validate(path, &vec![255; MAX_BYTES as usize]).is_ok());
        }
    }

    #[test]
    fn repository_inventory_requires_hydrated_lfs_binaries_and_keeps_text_limits()
    -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        git(root.path(), &["init"])?;
        let relative = "docs/ui/corpus/oversized.png";
        let target = root.path().join(relative);
        fs::create_dir_all(target.parent().ok_or("missing parent")?)?;
        fs::write(&target, vec![255; MAX_BYTES as usize + 1])?;
        assert!(check_at(root.path()).is_err());
        fs::write(
            root.path().join(".gitattributes"),
            "*.png filter=lfs diff=lfs merge=lfs -text\n",
        )?;
        assert!(check_at(root.path()).is_ok());
        fs::write(&target, vec![b'a'; MAX_BYTES as usize + 1])?;
        assert!(
            check_at(root.path()).is_err(),
            "LFS cannot bypass text limits"
        );
        fs::write(
            &target,
            "version https://git-lfs.github.com/spec/v1\noid sha256:0123456789\nsize 123\n",
        )?;
        assert!(
            check_at(root.path()).is_err(),
            "Build inputs must be hydrated"
        );
        fs::write(&target, vec![255; 16])?;
        fs::remove_file(root.path().join(".gitattributes"))?;
        assert!(
            check_at(root.path()).is_err(),
            "Small binaries also need LFS"
        );
        Ok(())
    }

    fn git(root: &Path, args: &[&str]) -> crate::Result<()> {
        let result = Command::new("git").current_dir(root).args(args).output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        Ok(())
    }

    #[test]
    fn git_inventory_covers_untracked_and_tracked_ignored_files() -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        git(root.path(), &["init"])?;
        fs::write(root.path().join(".gitignore"), "ignored.rs\n")?;
        fs::write(root.path().join("ignored.rs"), "\n".repeat(MAX_LINES + 1))?;
        assert!(check_at(root.path()).is_ok());
        git(root.path(), &["add", "-f", "ignored.rs"])?;
        assert!(check_at(root.path()).is_err());
        fs::write(root.path().join("ignored.rs"), "small\n")?;
        assert!(check_at(root.path()).is_ok());
        fs::write(
            root.path().join("file with spaces.rs"),
            "\n".repeat(MAX_LINES + 1),
        )?;
        assert!(check_at(root.path()).is_err());
        fs::write(root.path().join("file with spaces.rs"), vec![b'a'; 65537])?;
        assert!(check_at(root.path()).is_err());
        // The generated lockfile is an exact exception, not an extension wildcard.
        fs::write(root.path().join("file with spaces.rs"), "small\n")?;
        fs::write(root.path().join("Cargo.lock"), "\n".repeat(MAX_LINES + 1))?;
        assert!(check_at(root.path()).is_ok());
        assert!(check_at(&root.path().join("missing")).is_err());
        Ok(())
    }

    #[test]
    fn unstaged_deletions_and_renames_do_not_block_file_checks() -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        git(root.path(), &["init"])?;
        let source = root.path().join("tracked.rs");
        fs::write(&source, "small\n")?;
        git(root.path(), &["add", "tracked.rs"])?;
        fs::remove_file(&source)?;
        assert!(
            check_at(root.path()).is_ok(),
            "unstaged deletion is not a read failure"
        );
        fs::write(&source, "small\n")?;
        let renamed = root.path().join("renamed.rs");
        fs::rename(&source, &renamed)?;
        assert!(check_at(root.path()).is_ok());
        fs::write(&renamed, "\n".repeat(MAX_LINES + 1))?;
        assert!(
            check_at(root.path()).is_err(),
            "new side of an unstaged rename must still be checked"
        );
        Ok(())
    }
}
