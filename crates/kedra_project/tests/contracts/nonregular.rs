//! Static special-file inputs must fail without blocking the native project job.
#![cfg(unix)]
use std::{
    fs,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn project_and_history_fifo_reads_reject_without_waiting_for_a_writer() -> Result<()> {
    for kind in ["project", "history"] {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join(if kind == "project" {
            "kedra.toml"
        } else {
            "recent.toml"
        });
        assert!(Command::new("mkfifo").arg(&path).status()?.success());
        let mut child = Command::new(std::env::current_exe()?)
            .args(["--exact", "nonregular::nonregular_child", "--nocapture"])
            .env("KEDRA_PROJECT_FIFO_ROOT", temp.path())
            .env("KEDRA_PROJECT_FIFO_KIND", kind)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill()?;
                let output = child.wait_with_output()?;
                panic!(
                    "{kind} blocked on FIFO: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let result = child.wait_with_output()?;
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(fs::symlink_metadata(&path)?.file_type().is_fifo());
    }
    Ok(())
}

use std::os::unix::fs::FileTypeExt;

#[test]
fn nonregular_child() -> Result<()> {
    let Some(root) = std::env::var_os("KEDRA_PROJECT_FIFO_ROOT") else {
        return Ok(());
    };
    let root = std::path::PathBuf::from(root);
    let result = match std::env::var("KEDRA_PROJECT_FIFO_KIND")?.as_str() {
        "project" => kedra_project::open(&root).map(|_| ()),
        _ => kedra_project::recent::RecentProjects::load(&root.join("recent.toml")).map(|_| ()),
    };
    let error = result.err().ok_or("FIFO accepted")?;
    assert!(error.to_string().contains("regular file"), "{error}");
    Ok(())
}
