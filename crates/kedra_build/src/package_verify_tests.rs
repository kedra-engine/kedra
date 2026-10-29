//! Distribution validation inspects extracted bytes, not just input staging.
use crate::{package_verify, test_support::delivery};
use std::{fs, io::Write};

#[test]
fn corrupt_extracted_package_blocks_the_success_inventory() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = delivery()?;
    fixture.compile_executable()?;
    let output = fixture.root.path().join("packages");
    fs::create_dir(&output)?;
    let source = fixture.stage.join("assets/hello.txt");
    let original = fs::read(&source)?;
    fs::write(&source, "different bytes in the distributed archive")?;
    let archive = output.join("corrupt.zip");
    crate::package::portable_zip(&fixture.stage, &archive, "independent-probe")?;
    fs::write(&source, original)?;
    assert!(
        crate::pipeline::record_packages(
            &fixture.app,
            &fixture.stage,
            &output,
            vec![(archive, "zip")]
        )
        .is_err()
    );
    assert!(!output.join("artifact-inventory.json").exists());
    assert!(!output.join("SHA256SUMS").exists());
    Ok(())
}

#[test]
fn inspecting_an_nsis_installer_extracts_without_executing_it()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = delivery()?;
    fixture.compile_executable()?;
    #[cfg(not(windows))]
    fs::copy(
        fixture
            .stage
            .join(format!("independent-probe{}", std::env::consts::EXE_SUFFIX)),
        fixture.stage.join("independent-probe.exe"),
    )?;
    let source = fixture.root.path().join("extractor.rs");
    fs::write(
        &source,
        format!(
            r#"use std::path::Path;
fn copy(from: &Path, to: &Path) {{
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {{
        let entry = entry.unwrap();
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {{ copy(&entry.path(), &destination); }}
        else {{ std::fs::copy(entry.path(), destination).unwrap(); }}
    }}
}}
fn main() {{
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    assert_eq!(args.len(), 4);
    assert_eq!(args[0], "x"); assert_eq!(args[1], "-y");
    let output = args[2].to_str().unwrap().strip_prefix("-o").unwrap();
    assert!(Path::new(&args[3]).is_file());
    copy(Path::new({:?}), Path::new(output));
}}"#,
            fixture
                .stage
                .to_str()
                .ok_or("Fixture stage must be Unicode")?
        ),
    )?;
    let tool = fixture
        .root
        .path()
        .join(format!("extractor{}", std::env::consts::EXE_SUFFIX));
    assert!(
        std::process::Command::new("rustc")
            .arg("--crate-name=extractor")
            .arg(source)
            .arg("-o")
            .arg(&tool)
            .status()?
            .success()
    );
    let installer = fixture.root.path().join("installer.exe");
    fs::write(
        &installer,
        "This is not an executable; never execute this file",
    )?;
    let work = fixture.root.path().join("inspection");
    fs::create_dir(&work)?;
    package_verify::nsis_with_tool(
        &fixture.app,
        &fixture.stage,
        &installer,
        &work,
        tool.as_os_str(),
    )?;
    assert!(
        work.join("installed Юникод space/assets/kedra-assets.json")
            .is_file()
    );
    Ok(())
}

#[test]
fn missing_empty_source_directory_does_not_mean_missing_license_materials()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let expected = root.path().join("expected");
    let actual = root.path().join("actual");
    fs::create_dir(&expected)?;
    fs::create_dir(&actual)?;
    fs::create_dir(expected.join("covered-sources"))?;
    package_verify::same_tree(&expected, &actual)?;
    fs::write(
        expected.join("covered-sources/required.crate"),
        "required source",
    )?;
    assert!(package_verify::same_tree(&expected, &actual).is_err());
    Ok(())
}

#[test]
fn installed_delivery_cannot_differ_from_verified_staging() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = delivery()?;
    fixture.compile_executable()?;
    package_verify::installed(
        &fixture.app,
        &fixture.stage,
        &fixture.stage,
        &fixture
            .stage
            .join(format!("independent-probe{}", std::env::consts::EXE_SUFFIX)),
        &fixture.stage.join("assets"),
    )?;
    fs::write(
        fixture.stage.join("assets/unexpected.txt"),
        "not part of the delivery",
    )?;
    let other = fixture.root.path().join("expected-assets");
    fs::create_dir(&other)?;
    assert!(package_verify::same_tree(&other, &fixture.stage.join("assets")).is_err());
    Ok(())
}

#[test]
fn zip_members_cannot_escape_the_inspection_directory() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("bad.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive)?);
    zip.start_file("../escape.txt", zip::write::SimpleFileOptions::default())?;
    zip.write_all(b"escaped")?;
    zip.finish()?;
    let output = directory.path().join("inspection");
    fs::create_dir(&output)?;
    assert!(package_verify::unzip(&archive, &output).is_err());
    assert!(!directory.path().join("escape.txt").exists());
    Ok(())
}
