//! Exercise the actual CLI, including failure exit status and Cargo integration.
use std::process::Command;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
}

fn cli(args: &[&str]) -> std::process::Output {
    match command().args(args).output() {
        Ok(output) => output,
        Err(error) => panic!("could not run xtask: {error}"),
    }
}

#[test]
fn rejects_missing_unknown_and_extra_arguments() {
    for args in [vec![], vec!["typo"], vec!["qa"], vec!["fmt", "extra"]] {
        let output = cli(&args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("error:"));
    }
}

#[test]
fn architecture_checks_real_workspace_metadata() {
    let output = cli(&["architecture"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Architecture policy passed"));
}

#[test]
fn unavailable_tools_fail_instead_of_reporting_success() {
    for task in ["fmt", "lint", "doc", "architecture", "files"] {
        let output = match command()
            .arg(task)
            .env("PATH", "")
            .env_remove("CARGO")
            .output()
        {
            Ok(output) => output,
            Err(error) => panic!("could not run {task}: {error}"),
        };
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("error:"));
    }
}

#[test]
fn help_and_non_interactive_output_are_automation_friendly()
-> Result<(), Box<dyn std::error::Error>> {
    let help = cli(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--non-interactive"));
    let output = cli(&["files", "--non-interactive"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("[1/1] File growth policy"));
    assert!(stdout.contains("1 passed, 0 failed, 0 skipped"));
    assert!(stdout.contains("File size policy passed"));
    assert!(!stdout.contains('\u{1b}'));
    let path = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Log: "))
        .ok_or("log path missing")?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("root missing")?;
    let log = std::fs::read_to_string(root.join(path))?;
    assert!(log.contains("File size policy passed"));
    assert!(log.contains("Summary"));
    Ok(())
}

#[test]
fn failed_stage_has_summary_and_stops_remaining_work() {
    let output = match command()
        .args(["check", "--non-interactive"])
        .env("PATH", "")
        .env_remove("CARGO")
        .output()
    {
        Ok(output) => output,
        Err(error) => panic!("could not run xtask: {error}"),
    };
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FAIL File growth policy"));
    assert!(stdout.contains("0 passed, 1 failed, 7 skipped"));
    assert!(!stdout.contains("[2/"));
}

#[test]
fn legacy_host_summary_environment_has_no_side_effects() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let report = root.path().join("summary.md");
    for fail in [false, true] {
        std::fs::write(&report, "untouched\n")?;
        let mut invocation = command();
        invocation
            .args(["dependency-policy", "--non-interactive"])
            .env_remove("KEDRA_LUAU_LOCKFILE")
            .env("GITHUB_STEP_SUMMARY", &report);
        if fail {
            invocation.env("KEDRA_LUAU_LOCKFILE", root.path().join("missing.lock"));
        }
        let output = invocation.output()?;
        assert_eq!(output.status.success(), !fail);
        assert_eq!(std::fs::read_to_string(&report)?, "untouched\n");
        assert!(String::from_utf8_lossy(&output.stdout).contains("Summary"));
    }
    let output = command()
        .args(["dependency-policy", "--non-interactive"])
        .env_remove("KEDRA_LUAU_LOCKFILE")
        .env("GITHUB_STEP_SUMMARY", root.path())
        .output()?;
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn feature_checks_fail_closed_when_cargo_is_unavailable() -> Result<(), Box<dyn std::error::Error>>
{
    let output = command()
        .args(["features", "--non-interactive"])
        .env("PATH", "")
        .env_remove("CARGO")
        .output()?;
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("FAIL Default features"));
    assert!(stdout.contains("0 passed, 1 failed, 9 skipped"));
    assert!(!stdout.contains("[2/"));
    let path = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Log: "))
        .ok_or("log missing")?;
    let log = std::fs::read_to_string(path)?;
    assert!(log.contains("+ cargo check --workspace --all-targets --locked"));
    assert!(log.contains("FAIL Default features"));
    Ok(())
}
