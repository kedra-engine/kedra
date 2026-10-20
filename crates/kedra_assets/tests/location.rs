//! Installed resource locations are independent of the launch directory.
use kedra_assets::location::{BundlePlatform, bundled_asset_root};
use std::path::Path;

#[test]
fn linux_appdir_uses_standard_per_application_library_resources() -> std::io::Result<()> {
    let prefix = std::env::current_dir()?.join("target/Приложение Space.AppDir/usr");
    assert_eq!(
        bundled_asset_root(&prefix.join("bin/sample-app"), BundlePlatform::Linux)?,
        prefix.join("lib/sample-app/assets")
    );
    Ok(())
}

#[test]
fn standard_bundle_uses_resources_not_macos_directory() -> std::io::Result<()> {
    let root = std::env::current_dir()?.join("target/Приложение Space/Kedra.app/Contents");
    assert_eq!(
        bundled_asset_root(&root.join("MacOS/kedra"), BundlePlatform::Macos)?,
        root.join("Resources/assets")
    );
    Ok(())
}

#[test]
fn portable_and_development_binaries_use_executable_adjacent_assets() -> std::io::Result<()> {
    let directory = std::env::current_dir()?.join("target/Приложение Space");
    for platform in [
        BundlePlatform::Windows,
        BundlePlatform::Linux,
        BundlePlatform::Macos,
    ] {
        assert_eq!(
            bundled_asset_root(&directory.join("application"), platform)?,
            directory.join("assets")
        );
    }
    // A directory merely named MacOS is not sufficient to infer an app bundle.
    let directory = directory.join("MacOS");
    assert_eq!(
        bundled_asset_root(&directory.join("application"), BundlePlatform::Macos)?,
        directory.join("assets")
    );
    Ok(())
}

#[test]
fn relative_or_non_executable_locations_do_not_fall_back_to_current_directory() {
    for executable in [
        Path::new(""),
        Path::new("application"),
        Path::new("../application"),
    ] {
        assert!(bundled_asset_root(executable, BundlePlatform::current()).is_err());
    }
}
