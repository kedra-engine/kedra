#![allow(clippy::unwrap_used, reason = "Delivery contract assertions")]
use crate::delivery::*;
use std::collections::BTreeMap;

fn fixture() -> (Manifest, BTreeMap<String, Vec<u8>>) {
    let bytes = b"delivered data".to_vec();
    let meta = b"(meta_format_version:\"1.0\",asset:Load(loader:\"test.DataLoader\",settings:()))"
        .to_vec();
    let files: BTreeMap<String, Vec<u8>> =
        BTreeMap::from([("data.txt".into(), bytes), ("data.txt.meta".into(), meta)]);
    let manifest = Manifest {
        format: 1,
        profile: "test".into(),
        assets: vec![AssetRecord {
            id: uuid::Uuid::new_v4(),
            path: "data.txt".into(),
            loader: "test.DataLoader".into(),
        }],
        files: files
            .iter()
            .map(|(path, bytes)| {
                (
                    path.clone(),
                    FileDigest {
                        bytes: bytes.len() as u64,
                        sha256: digest(bytes),
                    },
                )
            })
            .collect(),
    };
    (manifest, files)
}
fn verify(manifest: &Manifest, files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    manifest.verify("test", |path| {
        files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Missing {path}"))
    })
}
#[test]
fn manifest_rejects_corruption_missing_metadata_wrong_loader_and_duplicate_identity() {
    let (manifest, files) = fixture();
    assert!(verify(&manifest, &files).is_ok());
    let mut bad = files.clone();
    bad.insert("data.txt".into(), b"corruption".to_vec());
    assert!(verify(&manifest, &bad).is_err());
    let mut bad = manifest.clone();
    bad.assets.push(bad.assets[0].clone());
    assert!(verify(&bad, &files).is_err());
    let mut bad = manifest.clone();
    bad.assets[0].id = uuid::Uuid::nil();
    assert!(verify(&bad, &files).is_err());
    let mut bad = manifest.clone();
    bad.assets[0].loader = "Other.Loader".into();
    assert!(verify(&bad, &files).is_err());
    let mut bad = manifest.clone();
    bad.files.remove("data.txt.meta");
    assert!(verify(&bad, &files).is_err());
    let mut bad = manifest.clone();
    bad.format = 2;
    assert!(verify(&bad, &files).is_err());
    assert!(
        manifest
            .verify("other-profile", |path| Ok(files[path].clone()))
            .is_err()
    );
    for path in [
        "",
        "../data.txt",
        "/data.txt",
        "a\\b",
        "a//b",
        "./data.txt",
        "a:b",
    ] {
        assert!(!normalized(path), "{path}");
    }
}
#[test]
fn native_delivery_verification_needs_no_authoring_sources() {
    let dir = tempfile::tempdir().unwrap();
    let (manifest, files) = fixture();
    for (name, bytes) in files {
        std::fs::write(dir.path().join(name), bytes).unwrap();
    }
    std::fs::write(
        dir.path().join(MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(read_directory(dir.path(), "test").is_ok());
    assert!(read_directory(dir.path(), "other").is_err());
    std::fs::write(dir.path().join(MANIFEST), "invalid JSON").unwrap();
    assert!(read_directory(dir.path(), "test").is_err());
    std::fs::write(dir.path().join(MANIFEST), vec![b' '; 65537]).unwrap();
    assert!(read_directory(dir.path(), "test").is_err());
}

#[test]
fn bootstrap_reads_only_manifest_but_full_verification_still_rejects_missing_members() {
    let dir = tempfile::tempdir().unwrap();
    let (manifest, _) = fixture();
    std::fs::write(
        dir.path().join(MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(read_manifest(dir.path(), "test").is_ok());
    assert!(read_directory(dir.path(), "test").is_err());
    let mut invalid = manifest.clone();
    invalid.assets.push(invalid.assets[0].clone());
    std::fs::write(
        dir.path().join(MANIFEST),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    assert!(read_manifest(dir.path(), "test").is_err());
    assert!(read_manifest(dir.path(), "other").is_err());
}

#[test]
fn uuid_address_syntax_is_reserved_so_paths_cannot_shadow_stable_identity() {
    let (mut manifest, mut files) = fixture();
    let path = format!("{}/asset.txt", manifest.assets[0].id);
    let bytes = files.remove("data.txt").unwrap();
    let meta = files.remove("data.txt.meta").unwrap();
    manifest.files = BTreeMap::from([
        (
            path.clone(),
            FileDigest {
                bytes: bytes.len() as u64,
                sha256: digest(&bytes),
            },
        ),
        (
            format!("{path}.meta"),
            FileDigest {
                bytes: meta.len() as u64,
                sha256: digest(&meta),
            },
        ),
    ]);
    files.insert(path.clone(), bytes);
    files.insert(format!("{path}.meta"), meta);
    manifest.assets[0].path = path;
    assert!(verify(&manifest, &files).is_err());
}

#[test]
fn physical_member_paths_preserve_native_asset_path_meaning() {
    for path in [
        "actor#variant.txt",
        "parent#variant/actor.txt",
        "actor.txt#",
    ] {
        assert!(!normalized(path), "{path}");
        let (mut manifest, _) = fixture();
        manifest.files.insert(
            path.into(),
            FileDigest {
                bytes: 0,
                sha256: digest(&[]),
            },
        );
        assert!(manifest.validate("test").is_err(), "{path}");
    }
    for path in [
        "Кедра 日本語/data.txt",
        "data%23literal.txt",
        "data space.txt",
    ] {
        assert!(normalized(path), "{path}");
        let uri = format!("game://{path}");
        let native = bevy::asset::AssetPath::try_parse(&uri).unwrap();
        assert_eq!(native.path().to_str(), Some(path));
        assert!(native.label().is_none());
    }
}

#[test]
fn native_manifest_accepts_exact_byte_boundary_and_rejects_one_more_byte() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = Manifest {
        format: 1,
        profile: "test".into(),
        assets: Vec::new(),
        files: BTreeMap::new(),
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    bytes.resize(crate::delivery::MAX_MANIFEST_BYTES, b' ');
    std::fs::write(directory.path().join(MANIFEST), &bytes).unwrap();
    assert!(read_directory(directory.path(), "test").is_ok());
    bytes.push(b' ');
    std::fs::write(directory.path().join(MANIFEST), bytes).unwrap();
    assert!(read_manifest(directory.path(), "test").is_err());
}

#[cfg(unix)]
#[test]
fn directory_verification_rejects_fifo_manifest_data_and_metadata_without_a_writer()
-> Result<(), Box<dyn std::error::Error>> {
    for fifo in [MANIFEST, "data.txt", "data.txt.meta"] {
        let directory = tempfile::tempdir()?;
        let (manifest, files) = fixture();
        for (path, bytes) in files {
            std::fs::write(directory.path().join(path), bytes)?;
        }
        std::fs::write(
            directory.path().join(MANIFEST),
            serde_json::to_vec(&manifest)?,
        )?;
        let path = directory.path().join(fifo);
        std::fs::remove_file(&path)?;
        assert!(
            std::process::Command::new("mkfifo")
                .arg(path)
                .status()?
                .success()
        );
        let error = read_directory(directory.path(), "test")
            .err()
            .ok_or("FIFO accepted")?;
        assert!(error.contains("regular files"), "{error}");
    }
    Ok(())
}

#[test]
fn directory_verification_rejects_oversized_members_before_parsing()
-> Result<(), Box<dyn std::error::Error>> {
    for oversized in ["data.txt", "data.txt.meta"] {
        let directory = tempfile::tempdir()?;
        let (manifest, files) = fixture();
        for (path, bytes) in files {
            std::fs::write(directory.path().join(path), bytes)?;
        }
        std::fs::write(
            directory.path().join(MANIFEST),
            serde_json::to_vec(&manifest)?,
        )?;
        // Sparse regular file: no test allocation proportional to its apparent size.
        std::fs::File::create(directory.path().join(oversized))?.set_len(4 * 1024 * 1024)?;
        let error = read_directory(directory.path(), "test")
            .err()
            .ok_or("Oversized member accepted")?;
        assert!(error.contains("byte limit"), "{error}");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn directory_verification_rejects_escaping_manifest_data_and_metadata_symlinks()
-> Result<(), Box<dyn std::error::Error>> {
    for relative in [MANIFEST, "data.txt", "data.txt.meta"] {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("delivery");
        std::fs::create_dir(&root)?;
        let (manifest, files) = fixture();
        for (path, bytes) in files {
            std::fs::write(root.join(path), bytes)?;
        }
        std::fs::write(root.join(MANIFEST), serde_json::to_vec(&manifest)?)?;
        let outside = directory.path().join("outside");
        std::fs::rename(root.join(relative), &outside)?;
        std::os::unix::fs::symlink(outside, root.join(relative))?;
        let error = read_directory(&root, "test")
            .err()
            .ok_or("Escaping member accepted")?;
        assert!(error.contains("escapes delivery"), "{error}");
    }
    Ok(())
}
