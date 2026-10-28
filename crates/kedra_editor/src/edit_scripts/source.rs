//! Bounded explicit Editor/ source snapshot over native files and standard Luau modules.
use kedra_script::{
    Program, Scope, Sources,
    lua::{self, Lua, LuaSerdeExt},
};
use std::{collections::BTreeMap, io::Read, path::Path};
#[cfg(test)]
mod tests;

/// Discoverable project-relative program/module path. Discovery executes no code
/// and grants no edit rights. `.luaurc` files participate in snapshots, not this list.
#[derive(Clone, Debug)]
pub struct ScriptInfo {
    /// Exact case-preserving path beneath the project's Editor directory.
    pub path: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    sources: Vec<String>,
}
fn validate(value: &serde_json::Value) -> Result<(), String> {
    let parameters: Parameters =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    if parameters.sources.is_empty() || parameters.sources.len() > 128 {
        return Err("Editor scripts require 1..=128 explicit source grants".into());
    }
    let mut unique = std::collections::BTreeSet::new();
    for path in parameters.sources {
        let path = path
            .strip_prefix("game://")
            .ok_or("Editor source grants require game:// paths")?;
        if !unique.insert(path.to_owned())
            || path.len() > 1024
            || path.contains(['\\', ':', '#'])
            || path.chars().any(char::is_control)
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
            || !(path.ends_with(".prefab.ron") || path.ends_with(".scene.ron"))
        {
            return Err("Invalid or duplicate editor source grant".into());
        }
    }
    Ok(())
}
fn bindings(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    // ctx.params was supplied/validated by Rust before VM allocation and is readonly.
    let parameters: Parameters = lua.from_value(ctx.get("params")?)?;
    super::install(lua, scope, ctx, parameters.sources)
}
fn snapshot(project: &Path) -> Result<BTreeMap<String, String>, String> {
    let project = std::fs::canonicalize(project).map_err(|error| error.to_string())?;
    let editor = project.join("Editor");
    match std::fs::symlink_metadata(&editor) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => (),
        _ => return Err("Editor script root must be a regular project directory".into()),
    }
    let mut files = BTreeMap::new();
    let mut validated = Sources::default();
    for (count, entry) in walkdir::WalkDir::new(&editor)
        .follow_links(false)
        .max_depth(17)
        .into_iter()
        .enumerate()
    {
        if count >= 1024 {
            return Err("Editor script inventory exceeds 1024 entries".into());
        }
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.depth() > 16
            || entry.file_type().is_symlink()
            || !(entry.file_type().is_dir() || entry.file_type().is_file())
        {
            return Err(
                "Editor scripts cannot contain links, special files or excessive nesting".into(),
            );
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .strip_prefix(&project)
            .map_err(|error| error.to_string())?
            .to_str()
            .ok_or("Editor script path must be Unicode")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if !(path.ends_with(".luau") || path.ends_with(".lua") || path.ends_with(".luaurc")) {
            continue;
        }
        if entry.metadata().map_err(|error| error.to_string())?.len() > 65_536 {
            return Err(format!("Editor script exceeds 64 KiB: {path}"));
        }
        let mut text = String::new();
        std::fs::File::open(entry.path())
            .map_err(|error| error.to_string())?
            .take(65_537)
            .read_to_string(&mut text)
            .map_err(|error| error.to_string())?;
        validated.insert(&path, &text)?;
        files.insert(path, text);
    }
    Ok(files)
}
/// Enumerate bounded Editor/ sources without running them or persisting trust.
/// Symlinks/special files and invalid native module paths refuse the entire snapshot.
pub fn discover(project: &Path) -> Result<Vec<ScriptInfo>, String> {
    Ok(snapshot(project)?
        .into_keys()
        .filter(|path| !path.ends_with(".luaurc"))
        .map(|path| ScriptInfo { path })
        .collect())
}
/// Load one explicitly selected Editor/ finite sequence with native source grants.
/// Standard relative/alias modules use the same immutable bounded source snapshot.
/// Loading never evaluates code, runs a build script, saves sources or persists trust.
pub fn load(project: &Path, entry: &str, sources: Vec<String>) -> Result<Program, String> {
    let params = serde_json::json!({"sources": sources});
    validate(&params)?;
    if !entry.starts_with("Editor/") || !(entry.ends_with(".luau") || entry.ends_with(".lua")) {
        return Err("Select an exact project-relative Editor/ script".into());
    }
    let mut files = snapshot(project)?;
    let source = files
        .remove(entry)
        .ok_or("Editor script is not in the admitted source snapshot")?;
    let mut program = Program::sequence("editor.project", entry, source, bindings);
    // Source commits synchronously compile the admitted catalog. Unlike runtime
    // behavior callbacks, these explicit tools need a bounded native-work allowance.
    // Keep the runtime's one-second ceiling and the ordinary whole-run deadline.
    program.limits.callback_time = std::time::Duration::from_secs(1);
    for (path, text) in files {
        program.sources.insert(path, text)?;
    }
    program.params = params;
    program.validate_params = validate;
    Ok(program)
}
