//! Exact AppImage helper provenance and LGPL relink materials.
use object::{Object, ObjectSection};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

const RUNTIME_HASH: &str = "617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46";
const APPRUN_HASH: &str = "f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f";

#[derive(Deserialize)]
struct Material {
    file: String,
    url: String,
    sha256: String,
    notices: Vec<String>,
}

pub(crate) fn prepare(notices: &Path, runtime_directory: &Path) -> Result<PathBuf, String> {
    let directory = notices.join("appimage-runtime");
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let materials: Vec<Material> =
        serde_json::from_str(include_str!("native-materials.json")).map_err(|e| e.to_string())?;
    for material in materials {
        let archive = directory.join(&material.file);
        download(&material.url, &archive, &material.sha256)?;
        for (index, notice) in material.notices.iter().enumerate() {
            let result = Command::new("tar")
                .arg("-xOf")
                .arg(&archive)
                .arg(notice)
                .output()
                .map_err(|e| format!("Cannot read verified source notice with tar: {e}"))?;
            if !result.status.success() || result.stdout.is_empty() {
                return Err(format!(
                    "Source notice missing: {} / {notice}",
                    material.file
                ));
            }
            write_new(
                &directory.join(format!("{}-{index}.NOTICE", material.file)),
                &result.stdout,
            )?;
        }
    }
    for (name, content) in [
        (
            "materials.json",
            include_bytes!("native-materials.json").as_slice(),
        ),
        (
            "README.md",
            include_bytes!("native-runtime-notice.md").as_slice(),
        ),
        (
            "rebuild.sh",
            include_bytes!("native-runtime-rebuild.sh").as_slice(),
        ),
        (
            "Dockerfile",
            include_bytes!("native-runtime.Dockerfile").as_slice(),
        ),
    ] {
        write_new(&directory.join(name), content)?;
    }
    let runtime = runtime_directory.join("runtime-x86_64");
    download(
        "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64",
        &runtime,
        RUNTIME_HASH,
    )?;
    Ok(runtime)
}

fn download(url: &str, output: &Path, hash: &str) -> Result<(), String> {
    eprintln!(
        "Archiving checksum-verified native material: {}",
        output.file_name().unwrap_or_default().to_string_lossy()
    );
    let bytes = ureq::get(url)
        .header("User-Agent", "Kedra distribution tooling")
        .call()
        .map_err(|e| format!("Native material download {url}: {e}"))?
        .body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    crate::licenses::publish_source(&bytes, output, hash)
}

fn write_new(path: &Path, contents: &[u8]) -> Result<(), String> {
    fs::File::create_new(path)
        .and_then(|mut file| file.write_all(contents))
        .map_err(|e| e.to_string())
}

pub(crate) fn verify_prefix(image: &Path, runtime: &Path) -> Result<(), String> {
    let runtime = fs::read(runtime).map_err(|e| e.to_string())?;
    let mut prefix = vec![0; runtime.len()];
    fs::File::open(image)
        .and_then(|mut file| file.read_exact(&mut prefix))
        .map_err(|e| format!("Native image runtime is truncated: {e}"))?;
    let elf = object::File::parse(runtime.as_slice())
        .map_err(|e| format!("Selected runtime ELF: {e}"))?;
    let (offset, length) = elf
        .section_by_name(".digest_md5")
        .and_then(|section| section.file_range())
        .ok_or("Selected runtime has no AppImage digest section")?;
    let start = usize::try_from(offset).map_err(|e| e.to_string())?;
    let end = usize::try_from(offset.checked_add(length).ok_or("Digest range overflow")?)
        .map_err(|e| e.to_string())?;
    if length != 16 || end > runtime.len() {
        return Err("Selected runtime has an invalid AppImage digest section".into());
    }
    // appimagetool fills precisely this standard mutable section. Derive its
    // range from the checksum-verified input, never from the untrusted output.
    if prefix[..start] != runtime[..start] || prefix[end..] != runtime[end..] {
        return Err("AppImage did not use the selected, source-covered runtime".into());
    }
    Ok(())
}

pub(crate) fn verify_apprun(path: &Path) -> Result<(), String> {
    let hash = format!(
        "{:x}",
        Sha256::digest(fs::read(path).map_err(|e| e.to_string())?)
    );
    if hash != APPRUN_HASH {
        return Err("AppRun differs from the reviewed source/notice provenance; review the new helper before distribution".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use object::{Object, ObjectSection};

    fn elf_fixture() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut elf = object::write::Object::new(
            object::BinaryFormat::Elf,
            object::Architecture::X86_64,
            object::Endianness::Little,
        );
        let text = elf.add_section(Vec::new(), b".text".to_vec(), object::SectionKind::Text);
        elf.append_section_data(text, &[0xc3; 32], 16);
        let digest = elf.add_section(
            Vec::new(),
            b".digest_md5".to_vec(),
            object::SectionKind::Data,
        );
        elf.append_section_data(digest, &[0; 16], 1);
        Ok(elf.write()?)
    }

    #[test]
    #[ignore = "requires retained real AppImage and official runtime fixture paths"]
    fn retained_appimage_digest_and_outside_field_mutation_are_checked()
    -> Result<(), Box<dyn std::error::Error>> {
        let image = PathBuf::from(
            std::env::var_os("KEDRA_TEST_APPIMAGE").ok_or("Set KEDRA_TEST_APPIMAGE")?,
        );
        let runtime =
            PathBuf::from(std::env::var_os("KEDRA_TEST_RUNTIME").ok_or("Set KEDRA_TEST_RUNTIME")?);
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(&runtime)?)),
            RUNTIME_HASH
        );
        verify_prefix(&image, &runtime)?;
        let root = tempfile::tempdir()?;
        let mut prefix = vec![0; fs::metadata(&runtime)?.len() as usize];
        fs::File::open(image)?.read_exact(&mut prefix)?;
        prefix[0] ^= 1;
        let changed = root.path().join("changed.AppImage");
        fs::write(&changed, prefix)?;
        assert!(verify_prefix(&changed, &runtime).is_err());
        Ok(())
    }

    #[test]
    fn standard_appimage_digest_changes_are_permitted_but_other_runtime_changes_fail()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let runtime = root.path().join("runtime");
        let image = root.path().join("image");
        let original = elf_fixture()?;
        let elf = object::File::parse(original.as_slice())?;
        let (offset, length) = elf
            .section_by_name(".digest_md5")
            .ok_or("Digest fixture missing")?
            .file_range()
            .ok_or("Digest data missing")?;
        assert_eq!(length, 16);
        let mut packaged = original.clone();
        packaged[offset as usize..(offset + length) as usize].fill(0x42);
        packaged.extend_from_slice(b"squashfs payload");
        fs::write(&runtime, &original)?;
        fs::write(&image, &packaged)?;
        verify_prefix(&image, &runtime)?;
        packaged[0] ^= 1;
        fs::write(&image, packaged)?;
        assert!(verify_prefix(&image, &runtime).is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires upstream network access; explicitly run by Linux distribution CI"]
    fn native_distribution_materials_provide_sources_and_modified_library_relink()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let runtime_directory = tempfile::tempdir()?;
        let runtime = prepare(root.path(), runtime_directory.path())?;
        verify_prefix(&runtime, &runtime)?;
        let directory = root.path().join("appimage-runtime");
        assert!(
            !directory.join("runtime-x86_64").exists(),
            "The packaging runtime is an input, not a license resource"
        );
        assert!(
            directory.join("GCC-COPYING3").is_file(),
            "GCC runtime license must accompany linked startup code"
        );
        assert!(
            directory.join("GCC-COPYING.RUNTIME").is_file(),
            "GCC runtime exception must accompany its license"
        );
        let work = root.path().join("source work Юникод");
        fs::create_dir(&work)?;
        let status = Command::new("sh")
            .arg(directory.join("rebuild.sh"))
            .arg("prepare")
            .arg(&work)
            .status()?;
        assert!(status.success());
        assert!(work.join("fuse/lib/fuse_lowlevel.c").is_file());
        assert!(work.join("runtime-source/src/runtime/Makefile").is_file());
        #[cfg(target_os = "linux")]
        {
            // Change LGPL implementation code that the runtime actually links.
            // Native execution plus the retained marker prove a real relink, not
            // just that the source archive or a compiler command exists.
            let source = work.join("fuse/lib/fuse_lowlevel.c");
            let original = fs::read_to_string(&source)?;
            let before = "fuse: empty argv passed to fuse_session_new().";
            assert_eq!(original.matches(before).count(), 1);
            let marker = "Kedra LGPL replacement-library fixture";
            fs::write(&source, original.replace(before, marker))?;
            let status = Command::new("sh")
                .arg(directory.join("rebuild.sh"))
                .arg("build")
                .arg(&work)
                .status()?;
            assert!(
                status.success(),
                "Recipient's modified-library relink must succeed"
            );
            let rebuilt = work.join("runtime-x86_64");
            let result = Command::new(&rebuilt).arg("--appimage-version").output()?;
            assert!(result.status.success());
            let strings = Command::new("strings").arg(&rebuilt).output()?;
            assert!(strings.status.success());
            assert!(String::from_utf8(strings.stdout)?.contains(marker));
            let map = fs::read_to_string(work.join("runtime-link.map"))?;
            assert!(
                map.contains("crtbegin"),
                "Link inventory must expose standard GCC startup objects"
            );
            assert!(
                map.contains("libfuse3.a"),
                "Link inventory must expose the replaceable library"
            );
            if let Some(evidence) = std::env::var_os("KEDRA_NATIVE_EVIDENCE") {
                let evidence = PathBuf::from(evidence);
                fs::create_dir_all(&evidence)?;
                for name in ["runtime-link.map", "toolchain-packages.txt"] {
                    write_new(&evidence.join(name), &fs::read(work.join(name))?)?;
                }
            }
        }
        assert!(prepare(root.path(), runtime_directory.path()).is_err());
        Ok(())
    }

    #[test]
    fn native_image_must_contain_the_selected_runtime_not_an_upstream_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let runtime = root.path().join("runtime");
        let image = root.path().join("application.AppImage");
        let original = elf_fixture()?;
        fs::write(&runtime, &original)?;
        let mut packaged = original.clone();
        packaged.extend_from_slice(b"payload");
        fs::write(&image, &packaged)?;
        verify_prefix(&image, &runtime)?;
        packaged[0] ^= 1;
        fs::write(&image, &packaged)?;
        assert!(verify_prefix(&image, &runtime).is_err());
        fs::write(&image, b"short")?;
        assert!(verify_prefix(&image, &runtime).is_err());
        Ok(())
    }
}
