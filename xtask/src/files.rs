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
        if info.len() > MAX_BYTES {
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
