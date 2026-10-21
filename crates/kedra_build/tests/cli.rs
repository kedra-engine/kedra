//! Actual library CLI entrypoint used by every thin project launcher.
use std::{ffi::OsString, process::ExitCode};

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn help_and_version_do_not_require_project_or_native_packaging_tools() {
    for option in ["--help", "--version"] {
        assert_eq!(
            kedra_build::run_cli(args(&["cargo kedra", option])),
            ExitCode::SUCCESS
        );
    }
}

#[test]
fn incomplete_unknown_and_unsupported_commands_fail() {
    for command in [
        vec!["cargo kedra"],
        vec!["cargo kedra", "unknown"],
        vec!["cargo kedra", "build", "--target", "not-a-rust-target"],
        vec!["cargo kedra", "package", "--unexpected"],
    ] {
        assert_eq!(kedra_build::run_cli(args(&command)), ExitCode::from(2));
    }
}

#[test]
fn missing_application_metadata_fails_before_running_build_scripts() -> std::io::Result<()> {
    let project = tempfile::tempdir()?;
    std::fs::create_dir(project.path().join("src"))?;
    std::fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname='independent-probe'\nversion='0.1.0'\nedition='2024'\n[workspace]\n",
    )?;
    std::fs::write(project.path().join("src/main.rs"), "fn main() {}")?;
    std::fs::write(
        project.path().join("build.rs"),
        "fn main() { std::fs::write(\"executed-build-script\", b\"unexpected\").unwrap(); }",
    )?;
    let mut command = args(&["cargo kedra", "build", "--manifest-path"]);
    command.push(project.path().join("Cargo.toml").into_os_string());
    assert_eq!(kedra_build::run_cli(command), ExitCode::FAILURE);
    assert!(!project.path().join("executed-build-script").exists());
    Ok(())
}

#[test]
fn icon_inputs_can_be_generated_without_compiling_an_application() -> std::io::Result<()> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("source.svg");
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="purple"/></svg>"#;
    std::fs::write(&source, svg)?;
    let output = directory.path().join("generated");
    let command = vec![
        OsString::from("cargo kedra"),
        "icons".into(),
        source.as_os_str().to_owned(),
        "--output".into(),
        output.as_os_str().to_owned(),
    ];
    assert_eq!(kedra_build::run_cli(command.clone()), ExitCode::SUCCESS);
    assert!(output.join("application.ico").is_file());
    assert!(output.join("icon-1024.png").is_file());
    assert_eq!(kedra_build::run_cli(command), ExitCode::FAILURE);
    assert_eq!(std::fs::read(source)?, svg);
    Ok(())
}

#[test]
fn standalone_asset_initialization_is_explicit_and_preserves_existing_identities()
-> std::io::Result<()> {
    let directory = tempfile::tempdir()?;
    std::fs::write(directory.path().join("greeting.txt"), "hello")?;
    let command = vec![
        OsString::from("cargo kedra"),
        "assets".into(),
        "init".into(),
        directory.path().as_os_str().to_owned(),
    ];
    assert_eq!(kedra_build::run_cli(command.clone()), ExitCode::SUCCESS);
    let meta = directory.path().join("greeting.txt.meta");
    let before = std::fs::read(&meta)?;
    assert_eq!(kedra_build::run_cli(command), ExitCode::SUCCESS);
    assert_eq!(std::fs::read(meta)?, before);
    let absent = directory.path().join("absent");
    let command = vec![
        OsString::from("cargo kedra"),
        "assets".into(),
        "init".into(),
        absent.as_os_str().to_owned(),
    ];
    assert_eq!(kedra_build::run_cli(command), ExitCode::FAILURE);
    assert!(!absent.exists());
    Ok(())
}
