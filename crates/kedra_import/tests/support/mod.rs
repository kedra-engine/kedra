//! External deadlines for importer probes that can block inside native readers.
use std::{
    ffi::OsStr,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn bounded_probe(
    test: &str,
    environment: &[(&str, &OsStr)],
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let logs = tempfile::tempdir()?;
    let stdout = logs.path().join("stdout");
    let stderr = logs.path().join("stderr");
    // Files cannot fill a pipe while the supervisor waits for native code.
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--exact", test, "--nocapture"])
        .envs(environment.iter().copied())
        .stdout(Stdio::from(std::fs::File::create(&stdout)?))
        .stderr(Stdio::from(std::fs::File::create(&stderr)?))
        .spawn()?;
    let deadline = Instant::now() + timeout;
    let result = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if result.is_none_or(|status| !status.success()) {
        return Err(format!(
            "{test}: {result:?} (None means external timeout)\n{}\n{}",
            std::fs::read_to_string(stdout)?,
            std::fs::read_to_string(stderr)?
        )
        .into());
    }
    Ok(())
}
