//! Opt-in authored text measurements and reversible glyph-paint isolation.
//! No remote methods are installed: the host owns scope and authorization.
use bevy::{
    prelude::*,
    text::{
        ComputedTextBlock, EditableText, FontSize, FontSource, LetterSpacing, LineHeight,
        TextLayoutInfo,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Component, Default)]
struct PaintState {
    saved: Vec<(Entity, TextColor)>,
    layer: String,
}

fn validate_state(
    world: &World,
    entities: &[Entity],
    requested: &str,
    state: &mut PaintState,
) -> Result<(), String> {
    if state.layer == "full" && requested == "full" {
        state.saved.clear();
    }
    for &(entity, mut expected) in &state.saved {
        if !entities.contains(&entity) {
            return Err("Visible text inventory changed during paint isolation".into());
        }
        if state.layer == "shape" {
            expected.0.set_alpha(0.);
        }
        if world.get::<TextColor>(entity) != Some(&expected) {
            return Err("Native text style changed while isolating glyph paint".into());
        }
    }
    Ok(())
}

fn set_paint(
    world: &mut World,
    entities: &[Entity],
    layer: &str,
    saved: &mut Vec<(Entity, TextColor)>,
) -> Result<(), String> {
    if !matches!(layer, "full" | "shape") {
        return Err("Expected full or shape text paint mode".into());
    }
    if entities
        .iter()
        .any(|e| world.get::<TextColor>(*e).is_none())
    {
        return Err("Text scope was retired or lacks native text color".into());
    }
    for &entity in entities {
        let original = if let Some((_, color)) = saved.iter().find(|(e, _)| *e == entity) {
            *color
        } else {
            let color = *world
                .get::<TextColor>(entity)
                .ok_or("Text color unavailable")?;
            saved.push((entity, color));
            color
        };
        let mut paint = original;
        if layer == "shape" {
            paint.0.set_alpha(0.);
        }
        world.entity_mut(entity).insert(paint);
    }
    Ok(())
}

fn effective_spacing(world: &World, entity: Entity) -> Option<f32> {
    // Bevy 0.20's editable layout uses PlainEditor without consuming LetterSpacing;
    // its effective spacing is zero, unlike ordinary Text's required component.
    if world.get::<EditableText>(entity).is_some() {
        return Some(0.);
    }
    match world.get::<LetterSpacing>(entity)? {
        LetterSpacing::Px(value) => Some(*value),
        _ => None,
    }
}

fn cluster_coverage(text: &str, ranges: &[[usize; 2]]) -> bool {
    if ranges.iter().any(|[start, end]| {
        start > end || !text.is_char_boundary(*start) || !text.is_char_boundary(*end)
    }) {
        return false;
    }
    text.char_indices().all(|(index, ch)| {
        ch.is_whitespace()
            || ranges
                .iter()
                .any(|[start, end]| *start <= index && *end >= index + ch.len_utf8())
    })
}

fn shaping_proof(world: &World, entity: Entity, text: &str) -> Option<Value> {
    // Read the shaper's source layout independently from TextLayoutInfo's final
    // positioned glyph list. Ligatures and combining clusters are not characters.
    let buffer = if let Some(input) = world.get::<EditableText>(entity) {
        input.editor().try_layout()?
    } else {
        world.get::<ComputedTextBlock>(entity)?.buffer()
    };
    let mut ranges = Vec::new();
    let mut glyph_ids = Vec::new();
    for line in buffer.lines() {
        for run in line.runs() {
            for cluster in run.clusters() {
                let range = cluster.text_range();
                ranges.push([range.start, range.end]);
                glyph_ids.extend(cluster.glyphs().map(|glyph| glyph.id));
            }
        }
    }
    Some(
        json!({"expected_glyphs":glyph_ids.len(),"missing_glyphs":glyph_ids.iter().filter(|&&id|id==0).count(),
        "coverage_complete":cluster_coverage(text,&ranges),"cluster_ranges":ranges,"glyph_ids":glyph_ids}),
    )
}

fn measured(world: &World, entity: Entity, original: TextColor) -> Option<Value> {
    let font = world.get::<TextFont>(entity)?;
    let FontSize::Px(size) = font.font_size else {
        return None;
    };
    let FontSource::Handle(handle) = &font.font else {
        return None;
    };
    let font_asset = world.get_resource::<Assets<Font>>()?.get(handle)?;
    let path = world
        .get_resource::<AssetServer>()?
        .get_path(handle.id())?
        .to_string();
    let node = world.get::<ComputedNode>(entity)?;
    let transform = world.get::<UiGlobalTransform>(entity)?;
    let layout = world.get::<TextLayoutInfo>(entity)?;
    let input = world.get::<EditableText>(entity);
    let content = if let Some(input) = input {
        input.value().to_string()
    } else {
        world.get::<Text>(entity)?.0.clone()
    };
    if !content.is_empty() && layout.glyphs.is_empty() {
        return None;
    }
    let offset = input.map_or(Vec2::ZERO, |input| input.viewport.offset);
    let local = node.content_box().min - offset;
    let points = [local, local + layout.size].map(|p| transform.affine().transform_point2(p));
    let clip = world.get::<CalculatedClip>(entity);
    let unclipped = points
        .iter()
        .all(|&p| clip.is_none_or(|clip| clip.contains_point(p)))
        && local
            .cmpge(node.content_box().min - Vec2::splat(0.01))
            .all()
        && (local + layout.size)
            .cmple(node.content_box().max + Vec2::splat(0.5))
            .all();
    let [min, max] = points.map(|p| p * node.inverse_scale_factor());
    let LineHeight::Px(line_height) = *world.get::<LineHeight>(entity)? else {
        return None;
    };
    let spacing = effective_spacing(world, entity)?;
    let lines = layout
        .glyphs
        .iter()
        .map(|g| g.line_index)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    Some(json!({"text":content,"size_px":size,"weight":font.weight.0,
        "line_height_px":line_height,"letter_spacing_px":spacing,
        "font_path":path,"font_sha256":format!("{:x}",Sha256::digest(font_asset.data.data())),
        "color":original.0.to_srgba().to_u8_array(),"rect":comparison_rect(world,entity)?,
        "content_rect":[min.x,min.y,max.x,max.y],"unclipped":unclipped,
        "lines":lines,"glyphs":layout.glyphs.len(),"editable":input.is_some(),
        "shaping":shaping_proof(world,entity,&content)?}))
}

fn comparison_rect(world: &World, entity: Entity) -> Option<[f32; 4]> {
    // Figma TextNode dimensions describe its allocated layout box (a field may
    // fill available width), not its glyph advance or alpha/ink rectangle.
    Some(crate::logical_rect(
        world.get::<ComputedNode>(entity)?,
        world.get::<UiGlobalTransform>(entity)?,
    ))
}

fn scope_texts(world: &mut World, root: Entity, owned: &[Entity]) -> Result<Vec<Entity>, String> {
    let mut hierarchy = world.query::<&Children>();
    let descendants = hierarchy
        .query(world)
        .iter_descendants::<Children>(root)
        .collect::<Vec<_>>();
    let texts = std::iter::once(root)
        .chain(descendants)
        .filter(|entity| {
            (world.get::<Text>(*entity).is_some() || world.get::<EditableText>(*entity).is_some())
                && world
                    .get::<InheritedVisibility>(*entity)
                    .is_some_and(|v| v.get())
        })
        .collect::<Vec<_>>();
    if texts.iter().any(|entity| !owned.contains(entity)) {
        return Err("Visible text in the selected subtree lacks an authored source address".into());
    }
    Ok(texts)
}

/// Measure the mounted authored subtree and select `full` or `shape` glyph paint.
/// Host supplies a live root and its complete source-address/entity inventory.
/// Returns a JSON snapshot matching retained Text/EditableText layout, loaded font
/// bytes and independent shaping coverage. Unmaterialized layout returns
/// `ready:false`; unsupported modes, scope/style changes return an error.
/// State is attached to the root and retired with it. Restore `full` before
/// releasing the scope; this does not suppress caret, selection or child icons.
pub fn capture(
    world: &mut World,
    root: Entity,
    nodes: &[(kedra_scene::document::NodeAddress, Entity)],
    layer: &str,
) -> Result<Value, String> {
    if !matches!(layer, "full" | "shape") {
        return Err("Expected full or shape".into());
    }
    if world.get_entity(root).is_err() {
        return Err("Text scope was retired".into());
    }
    let texts = scope_texts(
        world,
        root,
        &nodes.iter().map(|(_, entity)| *entity).collect::<Vec<_>>(),
    )?;
    let mut state = world
        .entity_mut(root)
        .take::<PaintState>()
        .unwrap_or_default();
    let result = (|| -> Result<Value, String> {
        validate_state(world, &texts, layer, &mut state)?;
        let mut snapshot = Vec::new();
        for (address, entity) in nodes.iter().filter(|(_, entity)| texts.contains(entity)) {
            let color = state
                .saved
                .iter()
                .find(|(e, _)| e == entity)
                .map(|(_, color)| *color)
                .or_else(|| world.get::<TextColor>(*entity).copied())
                .ok_or("Missing actual text color")?;
            let Some(style) = measured(world, *entity, color) else {
                return Ok(json!({"ready":false,"diagnostics":{
                    "address":address,"font":format!("{:?}",world.get::<TextFont>(*entity)),
                    "line_height":format!("{:?}",world.get::<LineHeight>(*entity)),
                    "spacing":format!("{:?}",world.get::<LetterSpacing>(*entity)),
                    "layout":world.get::<TextLayoutInfo>(*entity).map(|info|json!({"size":info.size.to_array(),"glyphs":info.glyphs.len()}))}}));
            };
            snapshot.push(json!({"address":address,"style":style}));
        }
        set_paint(world, &texts, layer, &mut state.saved)?;
        state.layer = layer.into();
        Ok(json!({"ready":true,"layer":state.layer,"texts":snapshot,
            "paint_hidden":texts.iter().all(|e|world.get::<TextColor>(*e).is_some_and(|c|c.0.alpha()==0.)),
            "button":crate::logical_rect(world.get::<ComputedNode>(root).ok_or("Unmeasured root")?,
                world.get::<UiGlobalTransform>(root).ok_or("Missing root transform")?)}))
    })();
    world.entity_mut(root).insert(state);
    result
}
