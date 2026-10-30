//! Native verification must be consumer-neutral and reject corrupt delivery.
use crate::{
    application,
    test_support::{configuration, delivery},
    verify,
};
use std::fs;

#[test]
fn configured_notices_do_not_require_sources_for_unneeded_mpl_dependencies()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = delivery()?;
    fixture.app.distribution.license_config = Some(fixture.root.path().join("about.toml"));
    fs::write(
        fixture.stage.join("licenses/dependency-notices.html"),
        "Complete permissive dependency notices",
    )?;
    verify::resources(
        &fixture.app,
        &fixture.stage.join("assets"),
        &fixture.stage.join("licenses"),
    )?;
    Ok(())
}

#[test]
fn runnable_result_binds_identity_and_relative_locations() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = configuration()?;
    let result = application::layout(&fixture.app);
    assert_eq!(result.schema_version, 1);
    assert_eq!(result.application, "independent-probe");
    assert_eq!(result.version, "1.2.3");
    assert!(!result.executable.is_absolute());
    assert!(!result.resources.is_absolute());
    Ok(())
}

#[test]
fn native_binary_architecture_is_checked_without_executing_consumer()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = configuration()?;
    let binary = fixture.compile_executable()?;
    verify::executable(&binary, fixture.app.target)?;
    let wrong = if fixture.app.target == crate::config::NativeTarget::MacArm {
        crate::config::NativeTarget::MacIntel
    } else {
        crate::config::NativeTarget::MacArm
    };
    assert!(verify::executable(&binary, wrong).is_err());
    fs::write(&binary, "not an executable")?;
    assert!(verify::executable(&binary, fixture.app.target).is_err());
    Ok(())
}

#[test]
fn matching_architecture_shared_library_is_not_a_runnable_application()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = configuration()?;
    let source = fixture.root.path().join("library.rs");
    fs::write(&source, "pub fn value() -> u32 { 42 }\n")?;
    let library = fixture
        .root
        .path()
        .join(format!("library{}", std::env::consts::DLL_SUFFIX));
    assert!(
        std::process::Command::new("rustc")
            .arg("--crate-type=cdylib")
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .status()?
            .success()
    );
    assert!(
        verify::executable(&library, fixture.app.target).is_err(),
        "A matching-architecture dynamic library must not pass as an application"
    );
    Ok(())
}

#[test]
fn verified_resources_reject_tampering_and_missing_notices()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = delivery()?;
    verify::resources(
        &fixture.app,
        &fixture.stage.join("assets"),
        &fixture.stage.join("licenses"),
    )?;
    fs::write(fixture.stage.join("assets/hello.txt"), "changed")?;
    assert!(
        verify::resources(
            &fixture.app,
            &fixture.stage.join("assets"),
            &fixture.stage.join("licenses")
        )
        .is_err()
    );
    let another = delivery()?;
    fs::remove_file(another.stage.join("licenses/NOTICE.txt"))?;
    assert!(
        verify::resources(
            &another.app,
            &another.stage.join("assets"),
            &another.stage.join("licenses")
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn result_paths_cannot_escape_the_reserved_output() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = configuration()?;
    let mut result = application::layout(&fixture.app);
    result.executable = "../elsewhere".into();
    assert!(verify::application(&fixture.app, fixture.root.path(), &result).is_err());
    assert!(!fixture.root.path().join("build-result.json").exists());
    Ok(())
}
