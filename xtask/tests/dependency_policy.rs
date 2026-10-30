//! Real policy CLI: offline admission and mandatory explicit consumer validation.
use std::process::Command;

#[test]
fn policy_is_offline_and_explicit_consumer_failures_are_terminal()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let cache = root.path().join("cache");
    std::fs::create_dir(&cache)?;
    let lock = root.path().join("consumer.lock");
    let run = |explicit: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .args(["dependency-policy", "--non-interactive"])
            .env("CARGO_HOME", &cache)
            .env("PATH", "")
            .env_remove("CARGO")
            .env_remove("KEDRA_LUAU_LOCKFILE");
        if explicit {
            command.env("KEDRA_LUAU_LOCKFILE", &lock);
        }
        command.output()
    };
    let output = run(false)?;
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for (content, success) in [
        (None, false),
        (Some("malformed Cargo lock"), false),
        (
            Some("version = 4\n[[package]]\nname = \"proc-macro-error2\"\nversion = \"2.0.2\"\n"),
            false,
        ),
        (Some("version = 4\n"), true),
    ] {
        if let Some(content) = content {
            std::fs::write(&lock, content)?;
        }
        let output = run(true)?;
        assert_eq!(
            output.status.success(),
            success,
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(content) = content {
            assert_eq!(std::fs::read_to_string(&lock)?, content);
        } else {
            assert!(!lock.exists());
        }
    }
    assert!(std::fs::read_dir(cache)?.next().is_none());
    Ok(())
}
