use super::*;
use crate::delivery::{AssetRecord, FileDigest};
use bevy::tasks::{block_on, futures_lite::io::AsyncRead};
use std::{
    collections::BTreeMap,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

struct CountingReader(Arc<AtomicUsize>);

#[cfg(feature = "model-support")]
#[test]
fn independent_model_selectors_reject_before_exposing_a_reader()
-> Result<(), Box<dyn std::error::Error>> {
    let cycle = serde_json::json!({"asset":{"version":"2.0"},"nodes":[{"children":[0]}],"scenes":[{"nodes":[0]}]});
    // Each row selects admission by exactly ONE predicate. Removing any one
    // selector must expose a Reader and fail this test, despite valid hashes.
    for (path, loader, bytes) in [
        (
            "renamed.bin",
            "test.DataLoader",
            crate::model::tests::glb(cycle.clone())?,
        ),
        (
            "model.bin",
            "bevy_gltf::loader::GltfLoader",
            serde_json::to_vec(&cycle)?,
        ),
        (
            "malformed.GLB",
            "test.DataLoader",
            b"not a binary model".to_vec(),
        ),
    ] {
        let directory = tempfile::tempdir()?;
        let mut manifest = manifest();
        manifest.assets[0].path = path.into();
        manifest.assets[0].loader = loader.into();
        manifest.files.clear();
        for (member, data) in [
            (path.to_string(), bytes.as_slice()),
            (format!("{path}.meta"), b"metadata".as_slice()),
        ] {
            std::fs::write(directory.path().join(&member), data)?;
            manifest.files.insert(
                member,
                FileDigest {
                    bytes: data.len() as u64,
                    sha256: digest(data),
                },
            );
        }
        let id = manifest.assets[0].id;
        let reader = VerifiedReader {
            inner: Box::new(bevy::asset::io::file::FileAssetReader::new(
                directory.path(),
            )),
            manifest: Arc::new(manifest),
            native_root: Ok(directory.path().canonicalize()?),
        };
        assert!(block_on(AssetReader::read_meta(&reader, Path::new(path))).is_ok());
        for requested in [path.to_string(), format!("{id}/asset.bin")] {
            let error = block_on(AssetReader::read(&reader, Path::new(&requested)))
                .err()
                .ok_or("unsafe model exposed as Reader")?;
            assert!(error.to_string().contains("GLB"), "{path}: {error}");
        }
    }
    Ok(())
}

#[cfg(feature = "model-support")]
#[test]
fn matching_glb_hashes_do_not_bypass_reader_topology_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let bad = crate::model::tests::glb(serde_json::json!({
        "asset":{"version":"2.0"}, "nodes":[{"children":[0]}],
        "scenes":[{"nodes":[0]}]
    }))?;
    let mut manifest = manifest();
    manifest.assets[0].path = "model.glb".into();
    manifest.assets[0].loader = "bevy_gltf::loader::GltfLoader".into();
    manifest.files.clear();
    for (path, bytes) in [
        ("model.glb", bad.as_slice()),
        ("model.glb.meta", b"metadata".as_slice()),
    ] {
        std::fs::write(directory.path().join(path), bytes)?;
        manifest.files.insert(
            path.into(),
            FileDigest {
                bytes: bytes.len() as u64,
                sha256: digest(bytes),
            },
        );
    }
    let id = manifest.assets[0].id;
    let reader = VerifiedReader {
        inner: Box::new(bevy::asset::io::file::FileAssetReader::new(
            directory.path(),
        )),
        manifest: Arc::new(manifest),
        native_root: Ok(directory.path().canonicalize()?),
    };
    // Metadata is not model data. The byte reader must reject the data itself,
    // before the native loader can receive a Reader and start recursion.
    assert!(block_on(reader.bytes(Path::new("model.glb"), true)).is_ok());
    for path in ["model.glb".to_string(), format!("{id}/asset.glb")] {
        let error = block_on(reader.bytes(Path::new(&path), false))
            .err()
            .ok_or("unsafe GLB reached native loader")?;
        assert!(error.to_string().contains("GLB"), "{error}");
    }
    let valid = crate::model::tests::glb(serde_json::json!({
        "asset":{"version":"2.0"}, "nodes":[{}], "scenes":[{"nodes":[0]}]
    }))?;
    // Both path and UUID reads still admit a valid, matching model.
    std::fs::write(directory.path().join("model.glb"), &valid)?;
    let mut updated = (*reader.manifest).clone();
    updated.files.insert(
        "model.glb".into(),
        FileDigest {
            bytes: valid.len() as u64,
            sha256: digest(&valid),
        },
    );
    let reader = VerifiedReader {
        manifest: Arc::new(updated),
        ..reader
    };
    for path in ["model.glb".to_string(), format!("{id}/asset.glb")] {
        assert!(block_on(reader.bytes(Path::new(&path), false)).is_ok());
    }
    Ok(())
}
impl AsyncRead for CountingReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        output: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let count = output.len().min(1024 - self.0.load(Ordering::Relaxed));
        output[..count].fill(0);
        self.0.fetch_add(count, Ordering::Relaxed);
        Poll::Ready(Ok(count))
    }
}
impl Reader for CountingReader {
    fn seekable(
        &mut self,
    ) -> Result<&mut dyn bevy::asset::io::SeekableReader, bevy::asset::io::ReaderNotSeekableError>
    {
        Err(bevy::asset::io::ReaderNotSeekableError)
    }
}
struct ProbeReader(Arc<AtomicUsize>);
impl AssetReader for ProbeReader {
    async fn read<'a>(&'a self, _: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Ok(CountingReader(self.0.clone()))
    }
    async fn read_meta<'a>(&'a self, _: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Ok(CountingReader(self.0.clone()))
    }
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.into()))
    }
    async fn is_directory<'a>(&'a self, _: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}
fn manifest() -> Manifest {
    Manifest {
        format: 1,
        profile: "test".into(),
        assets: vec![AssetRecord {
            id: uuid::Uuid::new_v4(),
            path: "data.txt".into(),
            loader: "test.DataLoader".into(),
        }],
        files: ["data.txt", "data.txt.meta"]
            .into_iter()
            .map(|path| {
                (
                    path.into(),
                    FileDigest {
                        bytes: 2,
                        sha256: digest(&[0, 0]),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    }
}

#[test]
fn oversized_data_and_metadata_stop_at_declared_length_plus_one()
-> Result<(), Box<dyn std::error::Error>> {
    for meta in [false, true] {
        let directory = tempfile::tempdir()?;
        for path in ["data.txt", "data.txt.meta"] {
            std::fs::write(directory.path().join(path), [0, 0])?;
        }
        let consumed = Arc::new(AtomicUsize::new(0));
        let reader = VerifiedReader {
            inner: Box::new(ProbeReader(consumed.clone())),
            manifest: Arc::new(manifest()),
            native_root: Ok(directory.path().canonicalize()?),
        };
        assert!(block_on(reader.bytes(Path::new("data.txt"), meta)).is_err());
        assert_eq!(consumed.load(Ordering::Relaxed), 3);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn native_runtime_rejects_escaping_data_and_metadata_symlinks()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("delivery");
    std::fs::create_dir(&root)?;
    let outside = directory.path().join("outside");
    std::fs::write(&outside, [0, 0])?;
    for path in ["data.txt", "data.txt.meta"] {
        std::os::unix::fs::symlink(&outside, root.join(path))?;
    }
    let reader = VerifiedReader {
        inner: Box::new(bevy::asset::io::file::FileAssetReader::new(&root)),
        manifest: Arc::new(manifest()),
        native_root: Ok(root.canonicalize()?),
    };
    for meta in [false, true] {
        assert!(block_on(reader.bytes(Path::new("data.txt"), meta)).is_err());
    }
    Ok(())
}

#[test]
fn excessive_declarations_fail_before_opening_or_consuming_a_reader()
-> Result<(), Box<dyn std::error::Error>> {
    for (path, length) in [
        ("data.txt", u64::MAX),
        ("data.txt", 256 * 1024 * 1024 + 1),
        ("data.txt.meta", 65_537),
    ] {
        let mut manifest = manifest();
        manifest
            .files
            .get_mut(path)
            .ok_or("missing fixture member")?
            .bytes = length;
        assert!(manifest.validate("test").is_err());
        let consumed = Arc::new(AtomicUsize::new(0));
        let reader = VerifiedReader {
            inner: Box::new(ProbeReader(consumed.clone())),
            manifest: Arc::new(manifest),
            native_root: Err("This root must not be accessed".into()),
        };
        assert!(block_on(reader.bytes(Path::new("data.txt"), path.ends_with(".meta"))).is_err());
        assert_eq!(consumed.load(Ordering::Relaxed), 0);
    }
    Ok(())
}

#[test]
fn native_reader_preserves_uuid_data_metadata_and_integrity_checks()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    for path in ["data.txt", "data.txt.meta"] {
        std::fs::write(directory.path().join(path), [0, 0])?;
    }
    let manifest = manifest();
    let uuid = format!("{}/asset.txt", manifest.assets[0].id);
    let reader = VerifiedReader {
        inner: Box::new(bevy::asset::io::file::FileAssetReader::new(
            directory.path(),
        )),
        manifest: Arc::new(manifest),
        native_root: Ok(directory.path().canonicalize()?),
    };
    for path in ["data.txt", uuid.as_str()] {
        for meta in [false, true] {
            assert!(block_on(reader.bytes(Path::new(path), meta)).is_ok());
        }
    }
    for bytes in [&[1, 1][..], &[0][..]] {
        for (file, meta) in [("data.txt", false), ("data.txt.meta", true)] {
            std::fs::write(directory.path().join(file), bytes)?;
            assert!(block_on(reader.bytes(Path::new("data.txt"), meta)).is_err());
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn native_runtime_rejects_fifos_and_escaping_parent_symlinks_before_open()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("delivery");
    std::fs::create_dir(&root)?;
    let reader = VerifiedReader {
        inner: Box::new(bevy::asset::io::file::FileAssetReader::new(&root)),
        manifest: Arc::new(manifest()),
        native_root: Ok(root.canonicalize()?),
    };
    for (path, meta) in [("data.txt", false), ("data.txt.meta", true)] {
        assert!(
            std::process::Command::new("mkfifo")
                .arg(root.join(path))
                .status()?
                .success()
        );
        let error = block_on(reader.bytes(Path::new("data.txt"), meta))
            .err()
            .ok_or("FIFO accepted")?;
        assert!(error.to_string().contains("regular files"));
    }
    let outside = directory.path().join("outside");
    std::fs::create_dir(&outside)?;
    std::fs::write(outside.join("data.txt"), [0, 0])?;
    std::os::unix::fs::symlink(&outside, root.join("alias"))?;
    let mut nested = manifest();
    nested.assets[0].path = "alias/data.txt".into();
    for (old, new) in [
        ("data.txt", "alias/data.txt"),
        ("data.txt.meta", "alias/data.txt.meta"),
    ] {
        let record = nested.files.remove(old).ok_or("missing fixture member")?;
        nested.files.insert(new.into(), record);
    }
    let reader = VerifiedReader {
        manifest: Arc::new(nested),
        ..reader
    };
    assert!(block_on(reader.bytes(Path::new("alias/data.txt"), false)).is_err());
    Ok(())
}

#[test]
fn resolved_native_paths_match_portable_manifest_keys_without_relaxing_integrity()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    std::fs::create_dir(directory.path().join("styles"))?;
    let mut manifest = manifest();
    manifest.assets[0].path = "styles/日本語.txt".into();
    for (old, new) in [
        ("data.txt", "styles/日本語.txt"),
        ("data.txt.meta", "styles/日本語.txt.meta"),
    ] {
        let record = manifest.files.remove(old).ok_or("missing fixture member")?;
        manifest.files.insert(new.into(), record);
        std::fs::write(directory.path().join(new), [0, 0])?;
    }
    let id = manifest.assets[0].id;
    let reader = VerifiedReader {
        inner: Box::new(bevy::asset::io::file::FileAssetReader::new(
            directory.path(),
        )),
        manifest: Arc::new(manifest),
        native_root: Ok(directory.path().canonicalize()?),
    };
    // Exercise the exact Bevy normalization used by relative CSS imports. On
    // Windows it constructs a backslash path even though the input uses slashes.
    let resolved =
        bevy::asset::AssetPath::from("game://fixtures").resolve_str("../styles/日本語.txt")?;
    let uuid_path = PathBuf::from(id.to_string()).join("asset.txt");
    for path in [resolved.path(), uuid_path.as_path()] {
        for meta in [false, true] {
            assert!(block_on(reader.bytes(path, meta)).is_ok(), "{path:?}");
        }
    }
    for invalid in [
        "../styles/日本語.txt",
        "/styles/日本語.txt",
        "styles/missing.txt",
        r"C:\styles\日本語.txt",
    ] {
        assert!(reader.mapped(Path::new(invalid)).is_err(), "{invalid}");
    }
    #[cfg(unix)]
    assert!(reader.mapped(Path::new(r"styles\日本語.txt")).is_err());
    for (file, meta) in [
        ("styles/日本語.txt", false),
        ("styles/日本語.txt.meta", true),
    ] {
        std::fs::write(directory.path().join(file), [1, 1])?;
        assert!(block_on(reader.bytes(resolved.path(), meta)).is_err());
    }
    Ok(())
}
