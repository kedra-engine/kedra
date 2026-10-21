//! Maintained native packager and archive libraries own their file formats.
use crate::config::{Application, NativeTarget};
use cargo_packager::{
    PackageFormat,
    config::{Binary, Resource},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(crate) fn create(
    app: &Application,
    stage: &Path,
    output: &Path,
    icons: Vec<PathBuf>,
) -> Result<Vec<(PathBuf, &'static str)>, String> {
    let native_libraries = if app.target == NativeTarget::Linux {
        crate::native_licenses::collect(&stage.join(&app.binary), &stage.join("licenses"))?
    } else {
        Vec::new()
    };
    let native_runtime = if app.target == NativeTarget::Linux {
        Some(crate::native_runtime::prepare(&stage.join("licenses"))?)
    } else {
        None
    };
    let mut config = app.native.clone();
    config.binaries = vec![Binary::new(&app.binary).main(true)];
    config.binaries_dir = Some(stage.to_path_buf());
    config.out_dir = output.join("packages");
    config.target_triple = Some(app.target.triple().into());
    config.icons = Some(native_icons(app.target, icons)?);
    config.resources = Some(
        ["assets", "licenses"]
            .into_iter()
            .map(|name| {
                let path = stage.join(name);
                let src = path
                    .to_str()
                    .ok_or("Native resource paths must be Unicode")?
                    .to_owned();
                Ok(Resource::Mapped {
                    src,
                    target: PathBuf::from(name),
                })
            })
            .collect::<Result<_, String>>()?,
    );
    config.formats = Some(vec![match app.target {
        NativeTarget::Windows => PackageFormat::Nsis,
        NativeTarget::Linux => PackageFormat::AppImage,
        NativeTarget::MacArm | NativeTarget::MacIntel => PackageFormat::Dmg,
    }]);
    fs::create_dir(&config.out_dir).map_err(|e| e.to_string())?;
    let generated = crate::worker::package(
        &config,
        app.package
            .manifest_path
            .parent()
            .ok_or("Application directory missing")?
            .as_std_path(),
        native_runtime.as_deref(),
    )?;
    let prefix = format!(
        "{}-{}-{}",
        app.package.name,
        app.package.version,
        app.target.triple()
    );
    let mut artifacts = Vec::new();
    for package in generated {
        let (format, extension) = match package.format {
            PackageFormat::App => continue,
            PackageFormat::Dmg => ("dmg", "dmg"),
            PackageFormat::Nsis => ("nsis", "exe"),
            PackageFormat::AppImage => ("appimage", "AppImage"),
            _ => return Err("Unexpected native package format".into()),
        };
        if package.paths.len() != 1 {
            return Err("Native package output is incomplete or ambiguous".into());
        }
        for source in package.paths {
            if !source.is_file() {
                return Err("Native package file is missing".into());
            }
            if package.format == PackageFormat::AppImage {
                crate::native_runtime::verify_prefix(
                    &source,
                    native_runtime
                        .as_deref()
                        .ok_or("Selected AppImage runtime missing")?,
                )?;
                crate::native_licenses::verify(&source, &native_libraries, &app.binary, stage)?;
            }
            if !source
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(config.out_dir.canonicalize().map_err(|e| e.to_string())?)
            {
                return Err("Native artifact escaped its reserved package output".into());
            }
            let destination = output.join(format!("{prefix}.{extension}"));
            if destination.exists() {
                return Err("Duplicate native package artifact".into());
            }
            fs::rename(source, &destination).map_err(|e| e.to_string())?;
            artifacts.push((destination, format));
        }
    }
    match app.target {
        NativeTarget::Windows => {
            let path = output.join(format!("{prefix}.zip"));
            portable_zip(stage, &path, app.package.name.as_str())?;
            artifacts.push((path, "zip"));
        }
        NativeTarget::Linux => {
            let path = output.join(format!("{prefix}.tar.gz"));
            let file = fs::File::create_new(&path).map_err(|e| e.to_string())?;
            let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut archive = tar::Builder::new(encoder);
            archive.follow_symlinks(false);
            archive
                .append_dir_all(app.package.name.as_str(), stage)
                .map_err(|e| e.to_string())?;
            archive
                .into_inner()
                .map_err(|e| e.to_string())?
                .finish()
                .map_err(|e| e.to_string())?;
            artifacts.push((path, "tar.gz"));
        }
        _ => {}
    }
    let expected = if matches!(app.target, NativeTarget::Windows | NativeTarget::Linux) {
        2
    } else {
        1
    };
    if artifacts.len() != expected {
        return Err("Required package formats are missing".into());
    }
    Ok(artifacts)
}

fn native_icons(target: NativeTarget, icons: Vec<PathBuf>) -> Result<Vec<String>, String> {
    let directory = icons
        .first()
        .and_then(|path| path.parent())
        .ok_or("Native icons missing")?;
    let icons = match target {
        NativeTarget::MacArm | NativeTarget::MacIntel => vec![directory.join("application.icns")],
        NativeTarget::Windows => vec![directory.join("application.ico")],
        NativeTarget::Linux => icons,
    };
    icons
        .iter()
        .map(|path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or("Native icon paths must be Unicode".to_string())
        })
        .collect()
}

fn portable_zip(stage: &Path, path: &Path, prefix: &str) -> Result<(), String> {
    let mut archive = zip::ZipWriter::new(fs::File::create_new(path).map_err(|e| e.to_string())?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for entry in walkdir::WalkDir::new(stage) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_symlink() {
            return Err("Portable package staging must not contain symbolic links".into());
        }
        let relative = entry
            .path()
            .strip_prefix(stage)
            .map_err(|e| e.to_string())?;
        let mut name = prefix.to_owned();
        for component in relative.components() {
            name.push('/');
            name.push_str(
                component
                    .as_os_str()
                    .to_str()
                    .ok_or("ZIP member paths must be Unicode")?,
            );
        }
        if entry.file_type().is_dir() {
            archive
                .add_directory(name, options)
                .map_err(|e| e.to_string())?;
        } else {
            archive
                .start_file(name, options)
                .map_err(|e| e.to_string())?;
            let mut file = fs::File::open(entry.path()).map_err(|e| e.to_string())?;
            io::copy(&mut file, &mut archive).map_err(|e| e.to_string())?;
        }
    }
    archive.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
#[path = "package_tests.rs"]
mod tests;
