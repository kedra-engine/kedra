//! Native ELF discovery and ownership belong to lddtree and dpkg-query.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Library {
    pub files: Vec<String>,
    pub package: String,
    pub version: String,
    pub source: String,
    pub source_version: String,
    pub elf_identity: String,
}

pub(crate) fn collect(executable: &Path, notices: &Path) -> Result<Vec<Library>, String> {
    eprintln!("Collecting native ELF ownership and copyright with lddtree/dpkg-query");
    let directory = notices.join("native-libraries");
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    // Ubuntu's apt-installed pyelftools belongs to its system Python, not a
    // virtualenv or setup-python interpreter selected through PATH.
    let dependencies = command_output(
        Command::new("/usr/bin/python3")
            .arg("/usr/bin/lddtree")
            .arg("--list")
            .arg(executable),
    )?;
    let executable = executable.canonicalize().map_err(|e| e.to_string())?;
    let mut libraries = Vec::new();
    let mut packages = BTreeSet::new();
    for dependency in dependencies.lines().map(Path::new) {
        if !dependency.is_absolute() || !dependency.is_file() {
            return Err(format!(
                "Unresolved native dependency: {}",
                dependency.display()
            ));
        }
        let resolved = dependency.canonicalize().map_err(|e| e.to_string())?;
        if resolved == executable {
            continue;
        }
        let owner = owner(dependency).or_else(|_| owner(&resolved))?;
        let metadata = command_output(Command::new("dpkg-query").args([
            "--show",
            "--showformat=${Package}\n${Version}\n${source:Package}\n${source:Version}\n",
            &owner,
        ]))?;
        let mut library = package_metadata(&metadata)?;
        library.elf_identity =
            crate::native_elf::identity(&fs::read(&resolved).map_err(|e| e.to_string())?)?;
        for path in [dependency, resolved.as_path()] {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("Native library filename is not Unicode")?;
            if !library.files.iter().any(|file| file == name) {
                library.files.push(name.into());
            }
        }
        if packages.insert(library.package.clone()) {
            stage_notice(Path::new("/usr/share/doc"), &directory, &library)?;
        }
        libraries.push(library);
    }
    if libraries.is_empty() {
        return Err("No native ELF dependencies discovered for the GNU Linux target".into());
    }
    let common = directory.join("common-licenses");
    fs::create_dir(&common).map_err(|e| e.to_string())?;
    for entry in fs::read_dir("/usr/share/common-licenses").map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().is_file() {
            copy_new(&entry.path(), &common.join(entry.file_name()))?;
        }
    }
    fs::File::create_new(directory.join("inventory.json"))
        .and_then(|mut file| file.write_all(serde_json::to_string_pretty(&libraries)?.as_bytes()))
        .map_err(|e| e.to_string())?;
    Ok(libraries)
}

fn command_output(command: &mut Command) -> Result<String, String> {
    eprintln!("+ {command:?}");
    let result = command
        .env("LC_ALL", "C.UTF-8")
        .env("PYTHONUTF8", "1")
        .output()
        .map_err(|e| format!("Native license tool {command:?}: {e}; install pax-utils and dpkg"))?;
    if !result.status.success() {
        return Err(format!(
            "Native license tool failed ({}): {}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    String::from_utf8(result.stdout).map_err(|e| e.to_string())
}

fn owner(path: &Path) -> Result<String, String> {
    let output = command_output(Command::new("dpkg-query").arg("--search").arg(path))?;
    let owners = package_owners(&output, path);
    if owners.len() != 1 {
        return Err(format!(
            "Native library has missing/ambiguous distro ownership: {}",
            path.display()
        ));
    }
    owners
        .into_iter()
        .next()
        .ok_or("Native library owner missing".into())
}

fn package_owners(output: &str, path: &Path) -> BTreeSet<String> {
    output
        .lines()
        .filter_map(|line| line.split_once(": "))
        .filter(|(_, owned)| Path::new(owned) == path)
        // dpkg-query also reports diversion descriptions, not just owners.
        // Accept literal package identities, including a multiarch qualifier.
        .filter(|(package, _)| {
            package.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"+.:-".contains(&byte)
            })
        })
        .map(|(package, _)| package.to_owned())
        .collect()
}

fn package_metadata(metadata: &str) -> Result<Library, String> {
    let fields: Vec<_> = metadata.lines().collect();
    if fields.len() != 4
        || fields.iter().any(|s| s.is_empty())
        || !fields[0]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+.-".contains(&b))
    {
        return Err("Native library package identity is incomplete or unsafe".into());
    }
    Ok(Library {
        files: Vec::new(),
        package: fields[0].into(),
        version: fields[1].into(),
        source: fields[2].into(),
        source_version: fields[3].into(),
        elf_identity: String::new(),
    })
}

fn copy_new(source: &Path, destination: &Path) -> Result<(), String> {
    let mut source = fs::File::open(source)
        .map_err(|e| format!("Required native copyright/license missing: {e}"))?;
    let mut destination = fs::File::create_new(destination).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut destination).map_err(|e| e.to_string())?;
    Ok(())
}

fn stage_notice(documentation: &Path, output: &Path, library: &Library) -> Result<(), String> {
    copy_new(
        &documentation.join(&library.package).join("copyright"),
        &output.join(format!("{}.copyright", library.package)),
    )
}

fn reviewed_notice(source: &str, notice: &[u8]) -> bool {
    use sha2::{Digest, Sha256};
    // Reviewed runtime-library terms, not a home-grown DEP-5/SPDX parser.
    let expected = match source {
        "brotli" => "24a64e5bb83d0960d1835696a1e23c0896ad6055b0ca47c66ab0eb9a766324b1",
        "bzip2" => "832ed535ff3c3d025a8d2348eb1b697b89addcf2eaadbc17650262040b9145e2",
        "libffi" => "737ff68f0eb104d3b0d96b291c3a8f40a775a3ec2b53f9b2e4bcfe3188bde118",
        "libpng1.6" => "4620d402b97601a910946acccbbe2e15bdffac11615bf9046520990d3b00f2b9",
        "wayland" => "736bf54141fa9808939ffff2691b817888ea53197484c2099e8d03b954b83e3b",
        _ => return false,
    };
    format!("{:x}", Sha256::digest(notice)) == expected
}

fn verify_notice_delivery(staged: &Path, installed: &Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let inventory = |root: &Path| -> Result<std::collections::BTreeMap<PathBuf, String>, String> {
        let mut files = std::collections::BTreeMap::new();
        for entry in walkdir::WalkDir::new(root) {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().is_symlink() {
                return Err(
                    "Source/notice delivery must contain regular files, not symbolic links".into(),
                );
            }
            if entry.file_type().is_file() {
                files.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_path_buf(),
                    format!(
                        "{:x}",
                        Sha256::digest(fs::read(entry.path()).map_err(|e| e.to_string())?)
                    ),
                );
            }
        }
        Ok(files)
    };
    if inventory(staged)? != inventory(installed)? {
        return Err(
            "Installed native/Rust source archives and notices differ from verified staging".into(),
        );
    }
    Ok(())
}

pub(crate) fn verify(
    appimage: &Path,
    libraries: &[Library],
    app: &crate::config::Application,
    staged: &Path,
) -> Result<(), String> {
    let binary = &app.binary;
    let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
    let status = Command::new(appimage)
        .arg("--appimage-extract")
        .current_dir(temporary.path())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!(
            "Cannot inspect actual bundled native libraries: {status}"
        ));
    }
    let root = temporary.path().join("squashfs-root");
    crate::native_runtime::verify_apprun(&root.join("AppRun"))?;
    let application = root.join("usr/bin").join(binary);
    if crate::native_elf::identity(&fs::read(&application).map_err(|e| e.to_string())?)?
        != crate::native_elf::identity(&fs::read(staged.join(binary)).map_err(|e| e.to_string())?)?
    {
        return Err("Bundled application differs from the compiled application".into());
    }
    let bundled_packages = verify_libraries(&root, libraries, &[application, root.join("AppRun")])?;
    let installed = root
        .join("usr/lib")
        .join(binary)
        .join("licenses/native-libraries");
    verify_notice_delivery(
        &staged.join("licenses"),
        installed
            .parent()
            .ok_or("Installed license directory missing")?,
    )?;
    for library in libraries {
        let path = installed.join(format!("{}.copyright", library.package));
        let notice = fs::read(&path)
            .map_err(|e| format!("Bundled native copyright missing: {}: {e}", library.package))?;
        if bundled_packages.contains(&library.package) && !reviewed_notice(&library.source, &notice)
        {
            return Err(format!(
                "Bundled native library terms require review/source support: {} from {} {}",
                library.package, library.source, library.source_version
            ));
        }
    }
    crate::package_verify::installed(
        app,
        staged,
        &root,
        &root.join("usr/bin").join(binary),
        &root.join("usr/lib").join(binary).join("assets"),
    )?;
    Ok(())
}

fn verify_libraries(
    directory: &Path,
    libraries: &[Library],
    known_elf: &[PathBuf],
) -> Result<BTreeSet<String>, String> {
    let mut packages = BTreeSet::new();
    for entry in walkdir::WalkDir::new(directory)
        .follow_links(true)
        .into_iter()
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry
            .path()
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(directory.canonicalize().map_err(|e| e.to_string())?)
        {
            return Err("Native package symlink escaped its AppDir".into());
        }
        if entry.file_type().is_file() {
            let mut magic = [0; 4];
            let length = fs::File::open(entry.path())
                .and_then(|mut file| file.read(&mut magic))
                .map_err(|e| e.to_string())?;
            if length == 4 && &magic == b"\x7fELF" {
                if known_elf.iter().any(|path| path == entry.path()) {
                    continue;
                }
                let identity = crate::native_elf::identity(
                    &fs::read(entry.path()).map_err(|e| e.to_string())?,
                )?;
                let name = entry
                    .file_name()
                    .to_str()
                    .ok_or("Native library filename is not Unicode")?;
                let owners: BTreeSet<_> = libraries
                    .iter()
                    .filter(|library| {
                        library.files.iter().any(|file| file == name)
                            && library.elf_identity == identity
                    })
                    .map(|library| library.package.clone())
                    .collect();
                if owners.len() != 1 {
                    return Err(format!(
                        "Bundled ELF has no verified distro copyright owner: {name}"
                    ));
                }
                packages.extend(owners);
            }
        }
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpkg_diversion_diagnostics_are_not_package_ownership_records() {
        let alias = Path::new("/lib64/ld-linux-x86-64.so.2");
        let diversion = "diversion by libc6 from: /lib64/ld-linux-x86-64.so.2\ndiversion by libc6 to: /lib64/ld-linux-x86-64.so.2.usr-is-merged\n";
        assert!(package_owners(diversion, alias).is_empty());
        let real = Path::new("/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2");
        assert_eq!(
            package_owners(
                "libc6:amd64: /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2\n",
                real
            ),
            BTreeSet::from(["libc6:amd64".into()])
        );
    }

    fn library() -> Library {
        Library {
            files: vec!["libfixture.so.1".into()],
            package: "fixture".into(),
            version: "1.2-3".into(),
            source: "fixture-source".into(),
            source_version: "1.2-3".into(),
            elf_identity: String::new(),
        }
    }

    #[test]
    fn native_source_and_notice_delivery_rejects_missing_or_changed_materials()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let stage = root.path().join("stage");
        let installed = root.path().join("installed");
        fs::create_dir(&stage)?;
        fs::create_dir(&installed)?;
        fs::write(stage.join("fuse-source.tar.xz"), b"original covered source")?;
        fs::write(
            installed.join("fuse-source.tar.xz"),
            b"original covered source",
        )?;
        verify_notice_delivery(&stage, &installed)?;
        fs::remove_file(installed.join("fuse-source.tar.xz"))?;
        assert!(verify_notice_delivery(&stage, &installed).is_err());
        fs::write(
            installed.join("fuse-source.tar.xz"),
            b"changed covered source",
        )?;
        assert!(verify_notice_delivery(&stage, &installed).is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires native Debian lddtree/dpkg; explicitly run by Linux distribution CI"]
    fn native_distribution_collects_real_elf_ownership_and_full_distro_notices()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let libraries = collect(Path::new("/bin/true"), root.path())?;
        assert!(libraries.iter().any(|library| library.source == "glibc"));
        let inventory = fs::read_to_string(root.path().join("native-libraries/inventory.json"))?;
        assert!(!inventory.contains("/usr/lib"));
        for library in libraries {
            assert!(
                !fs::read(
                    root.path()
                        .join("native-libraries")
                        .join(format!("{}.copyright", library.package))
                )?
                .is_empty()
            );
        }
        Ok(())
    }

    #[test]
    fn native_admission_does_not_approve_new_copyleft_libraries_or_changed_terms() {
        assert!(reviewed_notice(
            "brotli",
            include_bytes!("../../../tests/fixtures/distribution/native-brotli.copyright")
        ));
        assert!(!reviewed_notice("new-lgpl-library", b"GNU LGPL"));
        assert!(!reviewed_notice("brotli", b"Changed license terms"));
    }

    #[test]
    fn replacement_elf_cannot_inherit_distro_license_by_matching_its_filename()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let original = crate::native_elf::fixture(0xc3)?;
        let mut record = library();
        record.elf_identity = crate::native_elf::identity(&original)?;
        let path = root.path().join("libfixture.so.1");
        fs::write(&path, &original)?;
        verify_libraries(root.path(), &[record], &[])?;
        let mut record = library();
        record.elf_identity = crate::native_elf::identity(&original)?;
        fs::write(&path, crate::native_elf::fixture(0x90)?)?;
        assert!(verify_libraries(root.path(), &[record], &[]).is_err());
        Ok(())
    }

    #[test]
    fn nested_runtime_plugin_cannot_hide_under_application_resource_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let application = root.path().join("application");
        fs::create_dir(&application)?;
        fs::write(
            application.join("plugin.so"),
            crate::native_elf::fixture(0xc3)?,
        )?;
        assert!(verify_libraries(root.path(), &[], &[application]).is_err());
        Ok(())
    }

    #[test]
    fn native_notice_preserves_full_copyright_and_never_overwrites()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let docs = root.path().join("documentation");
        let output = root.path().join("licenses");
        fs::create_dir_all(docs.join("fixture"))?;
        fs::create_dir(&output)?;
        let notice = "Copyright 2026 Example Authors ©\nFull license terms\n";
        fs::write(docs.join("fixture/copyright"), notice)?;
        stage_notice(&docs, &output, &library())?;
        assert_eq!(
            fs::read_to_string(output.join("fixture.copyright"))?,
            notice
        );
        assert!(stage_notice(&docs, &output, &library()).is_err());
        Ok(())
    }

    #[test]
    fn missing_native_copyright_is_a_packaging_failure() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        assert!(stage_notice(root.path(), root.path(), &library()).is_err());
        Ok(())
    }

    #[test]
    fn unexpected_bundled_elf_cannot_pass_native_license_inspection()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("nested"))?;
        let original = crate::native_elf::fixture(0xc3)?;
        let mut record = library();
        record.elf_identity = crate::native_elf::identity(&original)?;
        fs::write(root.path().join("nested/libfixture.so.1"), original)?;
        verify_libraries(root.path(), &[record], &[])?;
        fs::write(
            root.path().join("nested/libunknown.so.1"),
            b"\x7fELFunknown",
        )?;
        assert!(verify_libraries(root.path(), &[library()], &[]).is_err());
        Ok(())
    }
}
