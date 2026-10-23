//! Independent scene chrome/menu geometry, including narrow native hosts.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};

pub(super) fn toolbar(c: &Editor<'_>, p: &str) -> Result<Node> {
    let viewport = c.node(&format!("{p}viewport"))?;
    let bar = c.node(&format!("{p}toolbar"))?;
    near(bar.rect[3] - bar.rect[1], 38., 1.)?;
    near(bar.rect[3], viewport.rect[1], 1.)?;
    let mut targets = Vec::new();
    for name in ["views", "overlays", "grid", "lighting"] {
        let n = c.node(&format!("{p}{name}"))?;
        check(
            n.window == viewport.window,
            "Toolbar control belongs to another window",
        )?;
        check(
            bar.rect[0] <= n.rect[0] && n.rect[0] < n.rect[2] && n.rect[2] <= bar.rect[2],
            "Toolbar target escaped horizontal bounds",
        )?;
        // Current accepted shared Flat controls are 22px (scene-view.md),
        // centered in the unchanged 38px toolbar; the old Python 26px check predates it.
        near(n.rect[1] - bar.rect[1], 8., 1.)?;
        near(n.rect[3] - n.rect[1], 22., 1.)?;
        targets.push(n.rect);
    }
    check(
        targets.windows(2).all(|w| w[0][2] <= w[1][0]),
        "Scene toolbar targets overlap",
    )?;
    let [x, y, r, b] = viewport.rect;
    if r - x < 640. {
        check(
            !c.present(&format!("{p}select-menu"))?,
            "Narrow layout retains Select",
        )?;
    }
    if r - x >= 160. && b - y >= 230. {
        let orbit = c.node(&format!("{p}orbit"))?;
        for (a, b) in [
            (orbit.rect[2] - orbit.rect[0], 96.),
            (orbit.rect[3] - orbit.rect[1], 96.),
            (r - orbit.rect[2], 12.),
            (orbit.rect[1] - y, 8.),
        ] {
            near(a, b, 1.)?;
        }
        for name in ["zoom", "pan", "frame", "projection", "select-tool"] {
            let n = c.node(&format!("{p}{name}"))?;
            check(
                n.window == viewport.window
                    && x <= n.rect[0]
                    && n.rect[2] <= r
                    && y <= n.rect[1]
                    && n.rect[3] <= b,
                "Scene navigation target escaped viewport",
            )?;
            near(n.rect[2] - n.rect[0], 22., 1.)?;
            near(n.rect[3] - n.rect[1], 22., 1.)?;
        }
    } else {
        for name in ["orbit", "zoom", "pan", "frame", "projection"] {
            check(
                !c.present(&format!("{p}{name}"))?,
                "Short canvas retains hidden navigation target",
            )?;
        }
    }
    Ok(viewport)
}
pub(super) fn menu(c: &Editor<'_>, p: &str, key: &str) -> Result<Value> {
    let a = c.node(&format!("{p}{key}"))?;
    let popup = c.node("dock-menu")?;
    check(
        popup.window == a.window && popup.parent == Some(a.entity),
        "Scene menu lost anchor/window",
    )?;
    check(
        (popup.rect[1] - a.rect[3] - 4.)
            .abs()
            .min((a.rect[1] - popup.rect[3] - 4.).abs())
            <= 1.,
        "Scene menu anchor gap differs",
    )?;
    check(
        popup.rect[0] <= a.rect[2] && popup.rect[2] >= a.rect[0],
        "Menu detached horizontally",
    )?;
    let observation = c.client.observe()?;
    let w = array(&observation["windows"])?
        .iter()
        .find(|w| w["id"] == a.window.unwrap_or(0))
        .ok_or("Missing scene menu host")?;
    check(
        popup.rect[0] >= 3.
            && popup.rect[1] >= 3.
            && popup.rect[2] <= number(&w["width"])? - 3.
            && popup.rect[3] <= number(&w["height"])? - 3.,
        "Scene menu escaped native host",
    )?;
    Ok(json!({"anchor":a.rect,"popup":popup.rect,"window":a.window}))
}
pub(super) fn resize(c: &Editor<'_>, view: &str, width: f64) -> Result<()> {
    let group = c.node(&Editor::group_id(&c.group(view)?))?;
    let divider = c
        .nodes()?
        .into_iter()
        .filter(|n| {
            n.id.ends_with("/divider")
                && n.rect[2] - n.rect[0] <= 8.
                && n.rect[3].min(group.rect[3]) > n.rect[1].max(group.rect[1])
        })
        .min_by(|a, b| distance(a, &group).total_cmp(&distance(b, &group)))
        .ok_or("No adjacent scene divider")?;
    check(
        distance(&divider, &group) <= 8.,
        "No adjoining vertical scene divider",
    )?;
    let point = divider.center();
    let half = (divider.rect[2] - divider.rect[0]) / 2.;
    let x = if (divider.rect[0] - group.rect[2]).abs() <= 8. {
        group.rect[0] + width + half
    } else {
        group.rect[2] - width - half
    };
    c.move_to(point)?;
    c.held(true)?;
    c.move_to([x, point[1]])?;
    c.held(false)
}
fn distance(n: &Node, g: &Node) -> f64 {
    (n.rect[0] - g.rect[2])
        .abs()
        .min((n.rect[2] - g.rect[0]).abs())
}
