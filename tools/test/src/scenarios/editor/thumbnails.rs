//! Native tile demand, generated pixels and persistent reuse across project close.
use super::support::*;
use kedra_test::Result;
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

fn generated(root: &Path) -> Result<BTreeMap<PathBuf, SystemTime>> {
    let mut files = BTreeMap::new();
    let root = root.join(".kedra/cache");
    if !root.exists() {
        return Ok(files);
    }
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let bytes = std::fs::read(entry.path())?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            continue;
        }
        let image = image::load_from_memory(&bytes)?.to_rgba8();
        check(
            image.width() <= 288 && image.height() <= 288,
            "Thumbnail escaped output size budget",
        )?;
        check(
            image
                .pixels()
                .any(|p| p[3] > 0 && p[0].max(p[1]).max(p[2]) > 20),
            "Thumbnail contains no visible content",
        )?;
        files.insert(entry.path().to_owned(), entry.metadata()?.modified()?);
    }
    Ok(files)
}
fn show(c: &Editor<'_>, name: &str) -> Result<()> {
    let search = c.find("Browser search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, name)?;
    c.until("visible generated thumbnail", || {
        Ok(c.nodes()?
            .iter()
            .any(|n| n.id.starts_with("browser/") && n.id.ends_with("/thumbnail") && n.visible))
    })?;
    c.frame(c.window()?, &format!("Thumbnail {name}"))?;
    let thumbnail = c.find("Visible thumbnail", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/thumbnail")
    })?;
    let frame = c.client.capture(thumbnail.window()?)?;
    check(
        frame
            .path
            .canonicalize()?
            .starts_with(c.artifacts.canonicalize()?),
        "Thumbnail capture outside session",
    )?;
    let image =
        image::load_from_memory(&kedra_test::evidence::read(&frame.path, 64 * 1024 * 1024)?)?
            .to_rgba8();
    let observation = c.client.observe()?;
    let window = array(&observation["windows"])?
        .iter()
        .find(|w| w["id"] == thumbnail.window.unwrap_or_default())
        .ok_or("Thumbnail window")?;
    let dpr = number(&window["dpr"])?;
    let mut colors = std::collections::BTreeSet::new();
    for y in (thumbnail.rect[1] * dpr).ceil() as u32..(thumbnail.rect[3] * dpr).floor() as u32 {
        for x in (thumbnail.rect[0] * dpr).ceil() as u32..(thumbnail.rect[2] * dpr).floor() as u32 {
            colors.insert(image.get_pixel(x, y).0);
        }
    }
    check(
        colors.len() > 8,
        format!("Thumbnail {name} is allocated but not painted"),
    )?;
    Ok(())
}
pub(super) fn run(c: &Editor<'_>, project: &Path) -> Result<()> {
    for name in ["checker.png", "marker.glb"] {
        show(c, name)?;
    }
    c.until("thumbnail disk publication", || {
        Ok(generated(project)?.len() >= 2)
    })?;
    let before = generated(project)?;
    let restored = c.current_document().ok();
    c.client
        .operation("editor.project", json!({"command":{"kind":"projects"}}))?;
    c.until("project closed", || {
        Ok(c.rpc("editor.state")?["phase"] == "projects")
    })?;
    c.open_project()?;
    if let Some(document) = restored {
        let view = text(&document["view"])?;
        super::scene::ready(c, &format!("panel/ViewId({view})/"))?;
        check(
            c.current_document()?["document"] == document["document"],
            "Restored tab did not admit its source automatically",
        )?;
    }
    for name in ["checker.png", "marker.glb"] {
        show(c, name)?;
    }
    check(
        generated(project)? == before,
        "Warm project reopening regenerated disk thumbnails",
    )?;
    let search = c.find("Browser search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "")?;
    Ok(())
}
