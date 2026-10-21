//! Real independent Cargo applications exercise shared build and failure paths.
use std::{
    ffi::OsString,
    fs,
    path::Path,
    process::{Command, ExitCode},
};

fn fixture() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let project = tempfile::Builder::new()
        .prefix("kedra independent Юникод ")
        .tempdir()?;
    fs::create_dir(project.path().join("src"))?;
    fs::create_dir(project.path().join("assets"))?;
    fs::write(
        project.path().join("src/main.rs"),
        "fn main() { println!(\"independent application\"); }",
    )?;
    fs::write(
        project.path().join("build.rs"),
        "fn main() { std::fs::write(\"build-script-ran\", b\"compiled\").unwrap(); }",
    )?;
    fs::write(
        project.path().join("assets/greeting.txt"),
        "hello resources",
    )?;
    fs::write(
        project.path().join("icon.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect x="2" y="2" width="28" height="28" rx="5" fill="#8844cc"/></svg>"##,
    )?;
    fs::write(
        project.path().join("NOTICE.txt"),
        "Independent test application; MIT OR Apache-2.0.\n",
    )?;
    fs::write(
        project.path().join("Cargo.toml"),
        r#"
[package]
name = "independent-app"
version = "1.2.3"
edition = "2024"
[workspace]
[package.metadata.packager]
productName = "Independent App"
identifier = "dev.kedra.independent-test"
[package.metadata.kedra.distribution]
assets = "assets"
profile = "game-desktop"
namespace = "game"
icon = "icon.svg"
notices = ["NOTICE.txt"]
"#,
    )?;
    kedra_import::initialize(&project.path().join("assets"), |_| {})?;
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .status()?;
    assert!(status.success());
    Ok(project)
}

fn run(project: &Path, output: &Path) -> ExitCode {
    let args: Vec<OsString> = vec![
        "cargo kedra".into(),
        "build".into(),
        "--release".into(),
        "--manifest-path".into(),
        project.join("Cargo.toml").into_os_string(),
        "--output".into(),
        output.as_os_str().to_owned(),
    ];
    kedra_build::run_cli(args)
}

#[test]
fn independent_release_build_stages_real_binary_verified_assets_icons_and_notices()
-> Result<(), Box<dyn std::error::Error>> {
    let project = fixture()?;
    let output = project.path().join("release space");
    assert_eq!(run(project.path(), &output), ExitCode::SUCCESS);
    assert!(project.path().join("build-script-ran").exists());
    let stage = output.join("application");
    let binary = stage.join(if cfg!(windows) {
        "independent-app.exe"
    } else {
        "independent-app"
    });
    let result = Command::new(binary)
        .current_dir(std::env::temp_dir())
        .output()?;
    assert!(result.status.success());
    assert!(String::from_utf8(result.stdout)?.contains("independent application"));
    let (_, manifest) =
        kedra_assets::delivery::read_directory(&stage.join("assets"), "game-desktop")?;
    assert_eq!(manifest.assets.len(), 1);
    assert_eq!(
        fs::read(stage.join("licenses/NOTICE.txt"))?,
        fs::read(project.path().join("NOTICE.txt"))?
    );
    for size in [16, 32, 48, 64, 128, 256, 512, 1024] {
        let image = image::open(stage.join(format!("icons/icon-{size}.png")))?;
        assert_eq!((image.width(), image.height()), (size, size));
    }
    assert!(fs::metadata(stage.join("icons/application.ico"))?.len() > 0);
    assert!(
        !output.join("artifact-inventory.json").exists(),
        "build alone must not claim packaged artifacts"
    );
    Ok(())
}

#[test]
fn unsupported_native_resources_are_rejected_before_compilation_or_output_creation()
-> Result<(), Box<dyn std::error::Error>> {
    let project = fixture()?;
    let manifest = project.path().join("Cargo.toml");
    let before = fs::read_to_string(&manifest)?;
    fs::write(
        &manifest,
        before.replace(
            "[package.metadata.packager]",
            "[package.metadata.packager]\nresources = ['settings.cfg']",
        ),
    )?;
    fs::write(
        project.path().join("settings.cfg"),
        "required application defaults",
    )?;
    let output = project.path().join("distribution");
    assert_eq!(run(project.path(), &output), ExitCode::FAILURE);
    assert!(!project.path().join("build-script-ran").exists());
    assert!(!output.exists());
    assert_eq!(
        fs::read_to_string(project.path().join("settings.cfg"))?,
        "required application defaults"
    );
    Ok(())
}

#[test]
fn existing_output_and_output_inside_assets_fail_before_build_scripts()
-> Result<(), Box<dyn std::error::Error>> {
    let project = fixture()?;
    let output = project.path().join("existing");
    fs::create_dir(&output)?;
    fs::write(output.join("keep.txt"), "preserved")?;
    assert_eq!(run(project.path(), &output), ExitCode::FAILURE);
    assert_eq!(fs::read_to_string(output.join("keep.txt"))?, "preserved");
    assert_eq!(
        run(
            project.path(),
            &project.path().join("assets/forbidden/output")
        ),
        ExitCode::FAILURE
    );
    assert!(!project.path().join("build-script-ran").exists());
    assert!(!project.path().join("assets/forbidden").exists());
    Ok(())
}

#[test]
fn manifest_selection_honors_the_independent_projects_cargo_configuration()
-> Result<(), Box<dyn std::error::Error>> {
    let project = fixture()?;
    fs::create_dir(project.path().join(".cargo"))?;
    fs::write(
        project.path().join(".cargo/config.toml"),
        "[build]\ntarget-dir='application-cargo-output'\n",
    )?;
    assert_eq!(
        run(project.path(), &project.path().join("distribution")),
        ExitCode::SUCCESS
    );
    // An explicit Cargo environment override takes precedence over config files.
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| project.path().join("application-cargo-output"));
    assert!(
        target.is_dir(),
        "Cargo must read the selected application's config, not the caller's checkout"
    );
    Ok(())
}

#[test]
fn default_output_inside_assets_is_rejected_without_modifying_sources()
-> Result<(), Box<dyn std::error::Error>> {
    if let Some(project) = std::env::var_os("KEDRA_TEST_OUTPUT_INSIDE_ASSETS") {
        let project = std::path::PathBuf::from(project);
        let args = vec![
            OsString::from("cargo kedra"),
            "build".into(),
            "--manifest-path".into(),
            project.join("Cargo.toml").into_os_string(),
        ];
        assert_eq!(kedra_build::run_cli(args), ExitCode::FAILURE);
        assert!(
            !project.join("assets/generated").exists(),
            "default staging must not create new asset sources"
        );
        assert!(!project.join("build-script-ran").exists());
        return Ok(());
    }
    let project = fixture()?;
    // A child process controls Cargo's environment without unsafe global env
    // mutation or interfering with other concurrent tests/coverage processes.
    let output = Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "default_output_inside_assets_is_rejected_without_modifying_sources",
        ])
        .env("KEDRA_TEST_OUTPUT_INSIDE_ASSETS", project.path())
        .env("CARGO_TARGET_DIR", project.path().join("assets/generated"))
        .output()?;
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn invalid_icon_and_failed_compilation_never_produce_a_success_inventory()
-> Result<(), Box<dyn std::error::Error>> {
    let project = fixture()?;
    fs::write(
        project.path().join("src/main.rs"),
        "compile_error!(\"intentional compiler failure\");",
    )?;
    let output = project.path().join("failed-compile");
    assert_eq!(run(project.path(), &output), ExitCode::FAILURE);
    assert!(project.path().join("build-script-ran").exists());
    assert!(!output.join("artifact-inventory.json").exists());
    fs::remove_file(project.path().join("build-script-ran"))?;
    fs::write(project.path().join("icon.svg"), "invalid SVG")?;
    let output = project.path().join("failed-icon");
    assert_eq!(run(project.path(), &output), ExitCode::FAILURE);
    assert!(!project.path().join("build-script-ran").exists());
    assert!(!output.join("artifact-inventory.json").exists());
    Ok(())
}
