//! Installer failure paths use real archives and process boundaries, never network.
use super::*;

#[test]
fn distro_lddtree_probe_uses_system_python_instead_of_path_python() {
    let command = prerequisite_command("lddtree");
    assert_eq!(command.get_program(), "/usr/bin/python3");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        vec![std::ffi::OsStr::new("/usr/bin/lddtree")]
    );
    let other = prerequisite_command("tar");
    assert_eq!(other.get_program(), "tar");
    assert!(other.get_args().next().is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn native_prerequisite_probes_accept_the_real_system_tools() {
    for (tool, argument) in required_tools(NativeTarget::MacArm, false) {
        assert!(probe(tool, argument).is_ok(), "{tool}");
    }
}

#[test]
fn package_only_prerequisites_are_not_needed_for_a_runnable_build() {
    assert!(
        !required_tools(NativeTarget::MacArm, false)
            .iter()
            .any(|(tool, _)| *tool == "hdiutil")
    );
    assert!(
        required_tools(NativeTarget::MacIntel, true)
            .iter()
            .any(|(tool, _)| *tool == "hdiutil")
    );
    assert!(
        !required_tools(NativeTarget::Linux, false)
            .iter()
            .any(|(tool, _)| *tool == "dpkg-query")
    );
    assert!(
        required_tools(NativeTarget::Linux, true)
            .iter()
            .any(|(tool, _)| *tool == "dpkg-query")
    );
}

#[test]
fn existing_tool_output_is_never_overwritten() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("marker");
    fs::write(&marker, "existing")?;
    assert!(install_about(directory.path(), Some(Path::new("does-not-exist"))).is_err());
    assert_eq!(fs::read_to_string(marker)?, "existing");
    Ok(())
}

#[test]
fn verified_source_cannot_be_installed_inside_an_application_workspace()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join("Cargo.toml"), "[workspace]\n")?;
    let output = directory.path().join("tools");
    let error = install_verified(
        &output,
        b"source",
        &format!("{:x}", Sha256::digest(b"source")),
        &cargo(),
    )
    .err()
    .ok_or("Workspace isolation unexpectedly allowed installation")?;
    assert!(error.contains("outside any Cargo workspace"), "{error}");
    assert!(!output.exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn advisory_failure_retains_source_but_never_compiles_it() -> Result<(), Box<dyn std::error::Error>>
{
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir()?;
    let executable = directory.path().join("cargo-probe");
    let calls = directory.path().join("calls");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\ncase \"$1\" in\nlocate-project) exit 1;;\nupdate) exit 0;;\ndeny) exit 9;;\n*) exit 0;;\nesac\n",
            calls.display()
        ),
    )?;
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))?;
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    let manifest = b"[package]\nname='cargo-about'\nversion='0.9.2'\n";
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, "cargo-about-0.9.2/Cargo.toml", &manifest[..])?;
    let bytes = builder.into_inner()?.finish()?;
    let output = directory.path().join("tool Юникод space");
    let error = install_verified(
        &output,
        &bytes,
        &format!("{:x}", Sha256::digest(&bytes)),
        executable.as_os_str(),
    )
    .err()
    .ok_or("Advisory failure unexpectedly allowed installation")?;
    assert!(error.contains("advisory audit failed"), "{error}");
    assert!(output.join("source/cargo-about-0.9.2/Cargo.toml").is_file());
    assert_eq!(fs::read_to_string(calls)?, "locate-project\nupdate\ndeny\n");
    assert!(!output.join("build").exists());
    Ok(())
}
