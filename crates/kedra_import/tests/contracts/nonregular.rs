//! Deadline-bounded subprocesses prevent a special-file regression hanging Cargo.
#![cfg(unix)]
use std::{
    fs,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn special_source_and_sidecar_files_are_rejected_before_content_opens() -> Result<()> {
    for special in ["data.txt.meta", "unrelated.pipe"] {
        for operation in ["build", "initialize"] {
            let temp = tempfile::tempdir()?;
            let source = temp.path().join("source");
            fs::create_dir(&source)?;
            fs::write(source.join("data.txt"), "original")?;
            assert!(
                Command::new("mkfifo")
                    .arg(source.join(special))
                    .status()?
                    .success()
            );
            let mut child = Command::new(std::env::current_exe()?)
                .args(["--exact", "nonregular::nonregular_child", "--nocapture"])
                .env("KEDRA_IMPORT_FIFO_ROOT", temp.path())
                .env("KEDRA_IMPORT_FIFO_OPERATION", operation)
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
                        "{operation} blocked on {special}: {}",
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
            assert_eq!(fs::read_to_string(source.join("data.txt"))?, "original");
            assert!(!temp.path().join("output").exists());
        }
    }
    Ok(())
}

#[test]
fn nonregular_child() -> Result<()> {
    let Some(root) = std::env::var_os("KEDRA_IMPORT_FIFO_ROOT") else {
        return Ok(());
    };
    let root = std::path::PathBuf::from(root);
    let result = match std::env::var("KEDRA_IMPORT_FIFO_OPERATION")?.as_str() {
        "build" => kedra_import::build(
            &root.join("source"),
            &root.join("output"),
            "game",
            "game",
            |_| {},
        )
        .map(|_| ()),
        _ => kedra_import::initialize(&root.join("source"), |_| {}).map(|_| ()),
    };
    let error = result.err().ok_or("special file was accepted")?;
    assert!(error.contains("regular files"), "{error}");
    Ok(())
}
