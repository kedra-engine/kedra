//! Actual headless recovery command: explicit root, clear outcome and no implicit editor launch.
use std::process::{Command, Output, Stdio};
fn bounded(command: &mut Command) -> std::io::Result<Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Includes the cold loader cost of the full debug editor binary under CI I/O
    // contention; this is a command/safety contract, not a startup benchmark.
    // Keep an independent kill deadline so a parser regression cannot launch and
    // leave a native editor running indefinitely instead of the headless command.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if std::time::Instant::now() >= deadline {
            child.kill()?;
            let output = child.wait_with_output()?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "Recovery command exceeded its external deadline: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            ));
        }
        std::thread::yield_now();
    }
}
#[test]
fn recovery_requires_an_explicit_existing_root_and_preserves_unreadable_intent()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    let binary = env!("CARGO_BIN_EXE_kedra");
    let run = || bounded(Command::new(binary).arg("recover-save").arg(&root));
    let output = run()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("No interrupted source Save found"));
    assert!(
        !bounded(Command::new(binary).arg("recover-save"))?
            .status
            .success()
    );
    let intent = directory.path().join("assets.kedra-save.request.ron");
    std::fs::write(&intent, "Untrusted invalid recovery record")?;
    let output = run()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("FAILED:"));
    assert_eq!(
        std::fs::read_to_string(intent)?,
        "Untrusted invalid recovery record"
    );
    assert_eq!(std::fs::read_dir(root)?.count(), 0);
    Ok(())
}
