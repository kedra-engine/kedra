//! Native immutable-delivery preflight and bounded standard-library reads.
//! This is not a sandbox against concurrent mutation of the delivery tree.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub(crate) fn member(root: &Path, relative: &str, maximum: u64) -> Result<PathBuf, String> {
    if !crate::delivery::normalized(relative) {
        return Err("Invalid delivery-relative member".into());
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !path.starts_with(root) {
        return Err(format!("Asset member escapes delivery: {relative}"));
    }
    let metadata = path.metadata().map_err(|error| error.to_string())?;
    validate(&metadata, maximum)?;
    Ok(path)
}

fn validate(metadata: &std::fs::Metadata, maximum: u64) -> Result<(), String> {
    if !metadata.is_file() {
        return Err("Asset delivery members must be regular files".into());
    }
    if metadata.len() > maximum {
        return Err("Asset delivery member exceeds its byte limit".into());
    }
    Ok(())
}

pub(crate) fn read(root: &Path, relative: &str, maximum: u64) -> Result<Vec<u8>, String> {
    let limit = maximum.checked_add(1).ok_or("Asset byte limit overflow")?;
    let path = member(root, relative, maximum)?;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    validate(
        &file.metadata().map_err(|error| error.to_string())?,
        maximum,
    )?;
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > maximum {
        return Err("Asset delivery member exceeds its byte limit".into());
    }
    Ok(bytes)
}
