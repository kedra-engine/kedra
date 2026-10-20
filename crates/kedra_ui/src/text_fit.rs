//! Ellipsis from Bevy's shaped clusters, not byte slicing or guessed glyph widths.
use bevy::{
    prelude::*,
    text::{ComputedTextBlock, LineHeight, TextLayoutInfo},
};

#[derive(Clone, Copy)]
pub(super) struct Title {
    full: Entity,
    ellipsis: Entity,
}
impl Title {
    pub(super) fn new(world: &mut World, parent: Entity) -> Self {
        let mut measure = |text: &str| {
            world
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    Text::new(text),
                    TextLayout::no_wrap(),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                    ChildOf(parent),
                ))
                .id()
        };
        Self {
            full: measure(""),
            ellipsis: measure("…"),
        }
    }
    pub(super) fn set(&self, world: &mut World, text: &str) {
        if let Some(mut full) = world.get_mut::<Text>(self.full)
            && full.0 != text
        {
            full.0 = text.into();
        }
    }
    pub(super) fn fit(&self, world: &mut World, visible: Entity, label: Entity) -> bool {
        let Some(font) = world.get::<TextFont>(visible).cloned() else {
            return false;
        };
        let line = world
            .get::<LineHeight>(visible)
            .copied()
            .unwrap_or_default();
        let mut changed = false;
        for entity in [self.full, self.ellipsis] {
            if world.get::<TextFont>(entity) != Some(&font) {
                world.entity_mut(entity).insert(font.clone());
                changed = true;
            }
            if world.get::<LineHeight>(entity) != Some(&line) {
                world.entity_mut(entity).insert(line);
                changed = true;
            }
        }
        if changed {
            return false;
        }
        let Some(node) = world.get::<ComputedNode>(label) else {
            return false;
        };
        let width = node.size().x - node.padding().min_inset.x - node.padding().max_inset.x;
        let Some(full) = world.get::<Text>(self.full) else {
            return false;
        };
        let Some(info) = world.get::<TextLayoutInfo>(self.full) else {
            return false;
        };
        if info.size.x <= 0. || width <= 0. {
            return false;
        }
        let clipped = info.size.x > width + 0.5;
        let next = if clipped {
            let Some(ellipsis) = world.get::<TextLayoutInfo>(self.ellipsis) else {
                return false;
            };
            let Some(block) = world.get::<ComputedTextBlock>(self.full) else {
                return false;
            };
            // Logical cluster order keeps combining sequences and shaped ligatures
            // intact, including when visual runs have been reordered by bidi.
            let mut clusters = Vec::new();
            for line in block.buffer().lines() {
                for run in line.runs() {
                    for cluster in run.clusters() {
                        clusters.push((cluster.text_range(), cluster.advance()));
                    }
                }
            }
            abbreviate(&full.0, &mut clusters, width, ellipsis.size.x)
        } else {
            full.0.clone()
        };
        if let Some(mut text) = world.get_mut::<Text>(visible)
            && text.0 != next
        {
            text.0 = next;
        }
        clipped
    }
}
fn abbreviate(
    text: &str,
    clusters: &mut [(std::ops::Range<usize>, f32)],
    width: f32,
    ellipsis: f32,
) -> String {
    clusters.sort_by_key(|(range, _)| range.start);
    let mut advance = 0.;
    let mut end = 0;
    for (range, cluster_width) in clusters {
        advance += *cluster_width;
        if advance + ellipsis > width {
            break;
        }
        end = range.end;
    }
    format!("{}…", text.get(..end).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitting_preserves_shaped_clusters_and_sorts_bidi_runs_in_logical_order() {
        let text = "Ae\u{301}👩‍🚀";
        let mut clusters = [(4..15, 12.), (1..4, 7.), (0..1, 6.)];
        assert_eq!(abbreviate(text, &mut clusters, 18., 5.), "Ae\u{301}…");
        assert_eq!(abbreviate(text, &mut clusters, 10., 5.), "…");
        assert_eq!(text, "Ae\u{301}👩‍🚀");
    }
    #[test]
    fn ligature_and_unbreakable_long_text_fit_at_cluster_not_byte_boundaries() {
        let mut clusters = [(0..3, 12.), (3..4, 6.)];
        assert_eq!(abbreviate("ffix", &mut clusters, 16., 5.), "…");
        assert_eq!(abbreviate("ffix", &mut clusters, 17., 5.), "ffi…");
    }
    #[test]
    fn missing_native_layout_or_retired_title_never_erases_full_source() {
        let mut world = World::new();
        let label = world.spawn(Node::default()).id();
        let visible = world
            .spawn((Text::new("original"), TextFont::default()))
            .id();
        let title = Title::new(&mut world, label);
        title.set(&mut world, "Long e\u{301}👩‍🚀 title");
        assert!(!title.fit(&mut world, visible, label));
        assert_eq!(
            world.get::<Text>(title.full).map(|text| text.0.as_str()),
            Some("Long e\u{301}👩‍🚀 title")
        );
        assert_eq!(
            world.get::<Text>(visible).map(|text| text.0.as_str()),
            Some("original")
        );
        world.despawn(visible);
        assert!(!title.fit(&mut world, visible, label));
        world.despawn(label);
        assert!(!world.entities().contains(title.full));
        assert!(!world.entities().contains(title.ellipsis));
    }
}
