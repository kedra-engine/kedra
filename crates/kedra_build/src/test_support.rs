//! Small real independent applications for native tool tests.
use crate::config::Application;
use std::{fs, path::PathBuf, process::Command};

pub(crate) struct Fixture {
    pub root: tempfile::TempDir,
    pub app: Application,
    pub stage: PathBuf,
}

pub(crate) fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::create_dir(root.path().join("assets"))?;
    fs::write(root.path().join("src/main.rs"), "fn main() {}\n")?;
    fs::write(
        root.path().join("Cargo.toml"),
        r#"[package]
name = "independent-probe"
version = "1.2.3"
edition = "2024"
[workspace]
[package.metadata.packager]
productName = "Independent Юникод Probe"
identifier = "dev.example.independent"
[package.metadata.kedra.distribution]
assets = "assets"
profile = "game-desktop"
namespace = "probe"
icon = "icon.svg"
notices = ["NOTICE.txt"]
"#,
    )?;
    fs::write(
        root.path().join("NOTICE.txt"),
        "Copyright Independent Probe\n",
    )?;
    fs::write(
        root.path().join("icon.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="purple"/></svg>"#,
    )?;
    fs::write(
        root.path().join("assets/hello.txt"),
        "independent delivery\n",
    )?;
    kedra_import::initialize(&root.path().join("assets"), |_| {})?;
    let app = Application::load(&root.path().join("Cargo.toml"), None, None)?;
    let stage = root.path().join("application");
    fs::create_dir(&stage)?;
    kedra_import::build(
        &app.distribution.assets,
        &stage.join("assets"),
        "game-desktop",
        "probe",
        |_| {},
    )?;
    fs::create_dir(stage.join("licenses"))?;
    fs::copy(
        root.path().join("NOTICE.txt"),
        stage.join("licenses/NOTICE.txt"),
    )?;
    let binary = stage.join(format!("independent-probe{}", std::env::consts::EXE_SUFFIX));
    let status = Command::new("rustc")
        .args(["--edition", "2024", "--crate-name", "probe"])
        .arg(root.path().join("src/main.rs"))
        .arg("-o")
        .arg(&binary)
        .status()?;
    assert!(status.success());
    Ok(Fixture { root, app, stage })
}
