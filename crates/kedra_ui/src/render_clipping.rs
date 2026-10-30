//! Elide ineffective clips only in changed, retained Bevy render copies.
//!
//! Bevy 0.20 retains extracted nodes but clips their geometry again in Prepare.
//! Its public containment test can prove that clipping cannot affect an entire
//! extracted item. Reuse that test, its change set and its renderer: no second
//! clipping algorithm, geometry cache or source/main-world writer is needed.
use bevy::{
    math::Affine2,
    prelude::*,
    render::{Render, RenderApp, RenderSystems},
    ui::CalculatedClip,
    ui_render::{ExtractedUiItem, ExtractedUiNodes},
};

pub(super) fn install(app: &mut App) {
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(
            Render,
            elide_contained_clips
                .after(RenderSystems::ExtractCommands)
                .before(bevy::ui_render::prepare_uinodes),
        );
    }
}

fn elide_contained_clips(extracted: Option<ResMut<ExtractedUiNodes>>) {
    let Some(mut extracted) = extracted else {
        return;
    };
    let ExtractedUiNodes { uinodes, changed } = &mut *extracted;
    for entity in changed.iter() {
        let Some((_, items)) = uinodes.get_mut(entity) else {
            continue;
        };
        for item in items.values_mut() {
            if item.clip.as_ref().is_some_and(|clip| {
                !clip.is_fully_clipped() && contained(&item.item, item.transform, clip)
            }) {
                item.clip = None;
            }
        }
    }
}

fn contained(item: &ExtractedUiItem, transform: Affine2, clip: &CalculatedClip) -> bool {
    match item {
        ExtractedUiItem::Node { rect, .. } => {
            quad_contained(rect.size(), Vec2::ZERO, transform, clip)
        }
        ExtractedUiItem::Glyphs { glyphs } => {
            !glyphs.is_empty()
                && glyphs.iter().all(|glyph| {
                    quad_contained(glyph.rect.size(), glyph.translation, transform, clip)
                })
        }
    }
}

fn quad_contained(size: Vec2, offset: Vec2, transform: Affine2, clip: &CalculatedClip) -> bool {
    if !size.is_finite() || size.min_element() < 0. {
        return false;
    }
    // Same centered local quad used by Bevy's Node/Glyphs preparation. Test every
    // actual glyph, not a text-layout box which may omit glyph overhangs.
    [
        Vec2::new(-0.5, -0.5),
        Vec2::new(0.5, -0.5),
        Vec2::new(0.5, 0.5),
        Vec2::new(-0.5, 0.5),
    ]
    .into_iter()
    .all(|corner| {
        let point = transform.transform_point2(offset + corner * size);
        point.is_finite() && clip.contains_point(point)
    })
}

#[cfg(test)]
mod tests;
