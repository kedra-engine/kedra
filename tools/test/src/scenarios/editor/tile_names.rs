//! Verify filename ink, not just a centered text box or a CSS property.
use super::super::support::*;
use kedra_test::{Result, evidence};
use serde_json::json;

pub(super) fn run(c: &Editor<'_>, browser: &str) -> Result<()> {
    let search = c.find("Assets search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    let prefix = search.id.strip_suffix("search").ok_or("Search identity")?;
    for (index, (mode, name)) in [
        ("grid", "Native"),
        ("list", "Native"),
        ("grid", "Native"),
        ("grid", "Straße.txt"),
        ("grid", "Composed.scene.ron"),
    ]
    .into_iter()
    .enumerate()
    {
        c.click(&format!("{prefix}{mode}"))?;
        c.field(&search.id, name)?;
        let label = c.find("Filename", |n| {
            n.id.starts_with(prefix)
                && n.id.contains("/row/")
                && n.id.ends_with("/name")
                && n.caption() == name
        })?;
        let tile = c
            .nodes()?
            .into_iter()
            .find(|n| Some(n.entity) == label.parent)
            .ok_or("Filename tile")?;
        // Leave the field and tile so neither blinking caret nor hover changes the capture.
        c.input(
            tile.window()?,
            json!({"kind":"click", "position":[tile.rect[2] + 20., tile.rect[3] + 20.]}),
        )?;
        c.frame(tile.window()?, "asset filename layout")?;
        let observation = c.client.observe()?;
        let window = array(&observation["windows"])?
            .iter()
            .find(|w| w["id"] == tile.window.unwrap_or_default())
            .ok_or("Window")?;
        let dpr = number(&window["dpr"])?;
        let size = [
            window["physical_width"].as_u64().ok_or("Width")? as u32,
            window["physical_height"].as_u64().ok_or("Height")? as u32,
        ];
        let (frame, _) = evidence::stable(c.client, tile.window()?, c.artifacts, size, None)?;
        let image =
            image::load_from_memory(&evidence::read(&frame.path, 64 * 1024 * 1024)?)?.to_rgba8();
        let label = c.node(&label.id)?;
        let tile = c.node(&tile.id)?;
        let x0 = (if mode == "grid" {
            tile.rect[0] + 4.
        } else {
            label.rect[0]
        } * dpr)
            .ceil() as u32;
        let x1 = (if mode == "grid" {
            tile.rect[2] - 4.
        } else {
            label.rect[2]
        } * dpr)
            .floor() as u32;
        let mut ink = Vec::new();
        for y in (label.rect[1] * dpr).ceil() as u32..(label.rect[3] * dpr).floor() as u32 {
            for x in x0..x1 {
                let pixel = image.get_pixel(x, y);
                if pixel[0].min(pixel[1]).min(pixel[2]) > 110 {
                    ink.push(x);
                }
            }
        }
        let left = *ink.iter().min().ok_or("No filename ink")? as f64 / dpr;
        let right = (*ink.iter().max().ok_or("No filename ink")? as f64 + 1.) / dpr;
        if mode == "list" {
            near(left, label.rect[0], 2.)?;
        } else if name != "Composed.scene.ron" {
            near((left + right) / 2., tile.center()[0], 1.5)?;
        } else {
            check(
                label.rect[2] - label.rect[0] <= 72.5,
                "Long filename escaped tile width",
            )?;
            check(
                label.caption() == name,
                "Clipping changed the source filename",
            )?;
        }
        // Exercise the same label layout after a real dock-size change.
        if index == 1 {
            c.menu(browser, "Maximize group")?;
        }
        if index == 2 {
            c.menu(browser, "Restore group")?;
        }
    }
    c.field(&search.id, "")?;
    Ok(())
}
