//! Renderer-measured drag targeting and Figma's guide/insertion/identity states.
use super::*;
use kedra_docking::{
    command::Selection,
    model::{Axis, Edge, Node},
    snapshot::Snapshot,
};
use kedra_ui::docking::{Geometry, class, geometry, rect};

const DIRECTIONS: [&str; 5] = ["tab", "left", "right", "top", "bottom"];
type Glyph = [Handle<Image>; 4];
#[cfg(test)]
#[path = "feedback_tests.rs"]
mod interaction_tests;
struct Guide {
    root: Entity,
    glyph: Entity,
    images: [Glyph; 3],
}
pub(super) struct Feedback {
    host: HostId,
    guides: Vec<Guide>,
    ghost: UiView,
    insertion: Entity,
    ghost_images: [Glyph; 2],
}
impl Feedback {
    pub(super) fn ready(&self, world: &World) -> bool {
        let assets = world.resource::<Assets<Image>>();
        self.guides
            .iter()
            .flat_map(|guide| guide.images.iter())
            .chain(self.ghost_images.iter())
            .flatten()
            .all(|image| assets.contains(image.id()))
    }
}
pub(super) struct Target {
    pub destination: Option<Destination>,
    pub insertion: bool,
}

pub(super) fn clear(world: &mut World, state: &mut EditorWorkspace) {
    if let Some(feedback) = state.feedback.take() {
        for guide in feedback.guides {
            world.despawn(guide.root);
        }
        world.despawn(feedback.ghost.root);
        world.despawn(feedback.insertion);
    }
    let tabs: Vec<_> = state
        .layout
        .snapshot()
        .views
        .keys()
        .filter_map(|view| state.renderer.tab(*view))
        .collect();
    for tab in tabs {
        class(world, tab, "dragging", false);
    }
}
fn mount(world: &mut World, state: &mut EditorWorkspace, host: HostId) -> Result<(), String> {
    if state
        .feedback
        .as_ref()
        .is_some_and(|feedback| feedback.host == host)
    {
        return Ok(());
    }
    clear(world, state);
    let parent = state
        .surfaces
        .get(&host)
        .ok_or("Drag destination host was retired")?
        .root;
    let ghost = crate::views::mount_asset(
        world,
        parent,
        "editor://prefabs/dock-ghost.prefab.ron",
        "dock.drag",
    )?;
    ignore(world, ghost.root, 300);
    let mut guides = Vec::new();
    for direction in DIRECTIONS {
        let view = crate::views::mount_asset(
            world,
            parent,
            "editor://prefabs/dock-guide.prefab.ron",
            &format!("dock.guide/{direction}"),
        )?;
        ignore(world, view.root, 250);
        let glyph = view.element("glyph").map_err(|e| e.to_string())?;
        let images = ["", "-target", "-unavailable"]
            .map(|suffix| load_glyph(world, &format!("dock-guide-{direction}{suffix}")));
        guides.push(Guide {
            root: view.root,
            glyph,
            images,
        });
    }
    let insertion = world
        .spawn((
            bevy::ui::Node::default(),
            bevy_flair::style::components::ClassList::new_with_classes([
                "dock-insertion",
                "hidden",
            ]),
            Pickable::IGNORE,
            GlobalZIndex(260),
            kedra_ui::ElementId::new("dock.insertion"),
            ChildOf(parent),
        ))
        .id();
    state.feedback = Some(Feedback {
        host,
        guides,
        ghost,
        insertion,
        ghost_images: [load_glyph(world, "dock-grip"), load_glyph(world, "warning")],
    });
    Ok(())
}
fn load_glyph(world: &World, name: &str) -> Glyph {
    std::array::from_fn(|index| {
        world
            .resource::<AssetServer>()
            .load(format!("editor://icons/{name}.svg#{}x", index + 1))
    })
}
fn ignore(world: &mut World, root: Entity, z: i32) {
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
        world.entity_mut(entity).insert(Pickable::IGNORE);
    }
    world.entity_mut(root).insert(GlobalZIndex(z));
}
fn place(world: &mut World, entity: Entity, parent: Rect, bounds: Rect) -> Result<(), String> {
    geometry(
        world,
        entity,
        Geometry::Rectangle(Rect::from_corners(
            bounds.min - parent.min,
            bounds.max - parent.min,
        )),
    )
}
fn guide_bounds(center: Vec2, index: usize) -> Rect {
    let offset = [
        Vec2::ZERO,
        Vec2::new(-32., 0.),
        Vec2::new(32., 0.),
        Vec2::new(0., -32.),
        Vec2::new(0., 32.),
    ][index];
    Rect::from_center_size(center + offset, Vec2::splat(28.))
}
fn destination(state: &EditorWorkspace, group: NodeId, index: usize) -> Destination {
    if index == 0 {
        let count = match &state.layout.snapshot().nodes[&group] {
            Node::Group { tabs, .. } => tabs.len(),
            _ => 0,
        };
        Destination::Tabs {
            group,
            index: count,
        }
    } else {
        Destination::Edge {
            target: group,
            edge: [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom][index - 1],
            fraction: 0.5,
        }
    }
}
/// Candidate validation and the same recursively resolved CSS minima as projection.
/// Topology changes can collapse the source branch, so test the candidate, not a
/// hardcoded half of the old destination box.
pub(super) fn admissible(
    world: &World,
    state: &EditorWorkspace,
    selection: Selection,
    destination: Destination,
) -> bool {
    let Ok(candidate) = state.layout.prepare(Command::Move {
        selection,
        destination,
    }) else {
        return false;
    };
    // Maximize is projection state. A structurally valid edge split can put the
    // moved view in an invisible sibling; do not offer that as a drop target.
    let selected = super::input::selected_view(state, selection);
    let visible_destination = selected
        .and_then(|view| candidate.snapshot().location(view))
        .and_then(|(group, _)| {
            let host = candidate.snapshot().host_of(group)?;
            Some(
                candidate.snapshot().hosts[&host]
                    .maximized
                    .is_none_or(|max| max == group),
            )
        })
        .unwrap_or(false);
    if !visible_destination {
        return false;
    }
    let source = super::input::selected_view(state, selection).and_then(|view| {
        state
            .layout
            .snapshot()
            .location(view)
            .map(|(group, _)| group)
    });
    let fallback = source
        .map(|group| {
            state
                .renderer
                .minimum(world, state.layout.snapshot(), group)
        })
        .unwrap_or(Vec2::ZERO);
    if let Destination::Edge { target, edge, .. } = destination {
        let Some(bounds) = state
            .renderer
            .group(target)
            .and_then(|root| rect(world, root))
        else {
            return false;
        };
        let target_min = state
            .renderer
            .minimum(world, state.layout.snapshot(), target);
        let axis = if matches!(edge, Edge::Left | Edge::Right) {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        // A split is offered inside the hovered group's measured allocation;
        // it must not steal space from unrelated neighbours to become available.
        let needed = if axis == Axis::Horizontal {
            fallback.x + target_min.x + 4.
        } else {
            fallback.y + target_min.y + 4.
        };
        if needed
            > if axis == Axis::Horizontal {
                bounds.width()
            } else {
                bounds.height()
            }
        {
            return false;
        }
    }
    candidate.snapshot().hosts.iter().all(|(host, record)| {
        let Some(root) = record.root else {
            return true;
        };
        let Some(bounds) = state
            .surfaces
            .get(host)
            .and_then(|surface| rect(world, surface.root))
        else {
            return false;
        };
        let min = minimum(world, state, candidate.snapshot(), root, fallback);
        min.x <= bounds.width() + 0.5 && min.y <= bounds.height() + 0.5
    })
}
fn minimum(
    world: &World,
    state: &EditorWorkspace,
    snapshot: &Snapshot,
    id: NodeId,
    fallback: Vec2,
) -> Vec2 {
    match &snapshot.nodes[&id] {
        Node::Group { .. } => {
            if state.renderer.group(id).is_some() {
                state.renderer.minimum(world, snapshot, id)
            } else {
                fallback
            }
        }
        Node::Split {
            axis,
            first,
            second,
            ..
        } => {
            let a = minimum(world, state, snapshot, *first, fallback);
            let b = minimum(world, state, snapshot, *second, fallback);
            let divider = state.renderer.divider_size(world, id, *axis).max(4.);
            if *axis == Axis::Horizontal {
                Vec2::new(a.x + b.x + divider, a.y.max(b.y))
            } else {
                Vec2::new(a.x.max(b.x), a.y + b.y + divider)
            }
        }
    }
}

pub(super) fn update(
    world: &mut World,
    state: &mut EditorWorkspace,
    selection: Selection,
    host: HostId,
    point: Vec2,
) -> Result<Target, String> {
    mount(world, state, host)?;
    let surface = state.surfaces[&host];
    let parent = rect(world, surface.root).ok_or("Drag host has not been laid out")?;
    let group = state.layout.snapshot().nodes.iter().find_map(|(id, node)| {
        if !matches!(node, Node::Group { .. }) || state.layout.snapshot().host_of(*id) != Some(host)
        {
            return None;
        }
        state
            .renderer
            .group(*id)
            .and_then(|root| rect(world, root))
            .filter(|bounds| bounds.contains(point) && bounds.size().min_element() > 0.)
            .map(|bounds| (*id, bounds))
    });
    let mut target = None;
    let mut insertion = None;
    let mut blocked = false;
    let mut guide_states = [0usize; 5];
    let mut guide_rects = None;
    if let Some((group, bounds)) = group {
        let rail = state
            .renderer
            .rail(group)
            .and_then(|root| rect(world, root));
        if let Some(rail) = rail.filter(|rail| rail.contains(point)) {
            let tabs = match &state.layout.snapshot().nodes[&group] {
                Node::Group { tabs, .. } => tabs,
                _ => return Err("Drag tab group was retired".into()),
            };
            let boxes: Vec<_> = tabs
                .iter()
                .filter_map(|view| state.renderer.tab(*view).and_then(|tab| rect(world, tab)))
                .collect();
            let index = insertion_index(&boxes, point.x);
            let destination = Destination::Tabs { group, index };
            if admissible(world, state, selection, destination) {
                target = Some(destination);
                let x = if index < boxes.len() {
                    boxes[index].min.x - 2.
                } else {
                    boxes.last().map_or(rail.min.x, |last| last.max.x + 6.)
                };
                insertion = Some(Rect::from_corners(
                    Vec2::new(x.clamp(rail.min.x, rail.max.x - 2.), rail.min.y + 2.),
                    Vec2::new(
                        (x + 2.).clamp(rail.min.x + 2., rail.max.x),
                        rail.min.y + 30.,
                    ),
                ));
            }
        } else {
            let boxes =
                std::array::from_fn::<_, 5, _>(|index| guide_bounds(bounds.center(), index));
            let hovered = boxes.iter().position(|bounds| bounds.contains(point));
            for (index, status) in guide_states.iter_mut().enumerate() {
                let destination = destination(state, group, index);
                let valid = admissible(world, state, selection, destination);
                *status = if !valid {
                    2
                } else if hovered == Some(index) {
                    1
                } else {
                    0
                };
                if hovered == Some(index) {
                    if valid {
                        target = Some(destination);
                    } else {
                        blocked = true;
                    }
                }
            }
            // Direct edge targeting is an additional path, not a substitute for
            // the five explicit guide targets. Gaps around guides never auto-merge.
            guide_rects = Some(boxes);
            if hovered.is_none() {
                let local = point - bounds.min;
                let distances = [
                    (local.x / bounds.width(), 1),
                    ((bounds.width() - local.x) / bounds.width(), 2),
                    (local.y / bounds.height(), 3),
                    ((bounds.height() - local.y) / bounds.height(), 4),
                ];
                if let Some((distance, index)) =
                    distances.into_iter().min_by(|a, b| a.0.total_cmp(&b.0))
                    && distance < 0.25
                {
                    guide_rects = None;
                    if guide_states[index] != 2 {
                        target = Some(destination(state, group, index));
                        guide_states[index] = 1;
                    } else {
                        blocked = true;
                    }
                }
            }
        }
    } else if state
        .layout
        .snapshot()
        .hosts
        .get(&host)
        .is_some_and(|host| host.root.is_none())
        && parent.contains(point)
    {
        target = Some(Destination::EmptyHost(host));
    }
    let selected = super::input::selected_view(state, selection);
    if let Some(view) = selected {
        let source = state
            .layout
            .snapshot()
            .location(view)
            .map(|(group, _)| group);
        let tabs: Vec<_> = match selection {
            Selection::View(view) => vec![view],
            Selection::Group(_) => source
                .and_then(|group| match &state.layout.snapshot().nodes[&group] {
                    Node::Group { tabs, .. } => Some(tabs.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
        };
        for view in tabs {
            if let Some(tab) = state.renderer.tab(view) {
                class(world, tab, "dragging", true);
            }
        }
    }
    let title = selected
        .and_then(|view| state.titles.get(&view))
        .cloned()
        .unwrap_or_default();
    let feedback = state.feedback.as_ref().ok_or("Drag feedback is absent")?;
    let images_ready = feedback.ready(world);
    for (index, guide) in feedback.guides.iter().enumerate() {
        class(
            world,
            guide.root,
            "hidden",
            guide_rects.is_none() || !images_ready,
        );
        class(world, guide.root, "target", guide_states[index] == 1);
        class(world, guide.root, "unavailable", guide_states[index] == 2);
        if let Some(boxes) = guide_rects {
            place(world, guide.root, parent, boxes[index])?;
        }
        set_glyph(world, guide.glyph, &guide.images[guide_states[index]])?;
    }
    class(world, feedback.insertion, "hidden", insertion.is_none());
    if let Some(bounds) = insertion {
        place(world, feedback.insertion, parent, bounds)?;
    }
    class(world, feedback.ghost.root, "blocked", blocked);
    let title_entity = feedback.ghost.element("title").map_err(|e| e.to_string())?;
    if let Some(mut text) = world.get_mut::<Text>(title_entity)
        && text.0 != title
    {
        text.0 = title;
    }
    let glyph = feedback.ghost.element("glyph").map_err(|e| e.to_string())?;
    set_glyph(world, glyph, &feedback.ghost_images[usize::from(blocked)])?;
    let size = if blocked {
        Vec2::new(272., 56.)
    } else {
        Vec2::new(144., 32.)
    };
    // Stable pointer offset, like DockGhost; never a captured panel screenshot.
    let origin = (point + Vec2::splat(16.)).clamp(parent.min, (parent.max - size).max(parent.min));
    place(
        world,
        feedback.ghost.root,
        parent,
        Rect::from_corners(origin, origin + size),
    )?;
    Ok(Target {
        destination: if guide_rects.is_some() && !images_ready {
            None
        } else {
            target
        },
        insertion: insertion.is_some(),
    })
}
fn set_glyph(world: &mut World, entity: Entity, images: &Glyph) -> Result<(), String> {
    use kedra_ui::authored::components::{UiImage, UiImageDensity};
    // The retained authored adapter owns ImageNode. Update its source variant,
    // never fight it by replacing ImageNode every other frame. Wait for the
    // delivered native raster before publishing a new dependency to that adapter.
    for image in images {
        if !world.resource::<Assets<Image>>().contains(image.id()) {
            if let Some(bevy::asset::LoadState::Failed(error)) = world
                .get_resource::<AssetServer>()
                .and_then(|server| server.get_load_state(image.id()))
            {
                return Err(format!("Dock guide image failed to load: {error}"));
            }
            return Ok(());
        }
    }
    if let Some(mut source) = world.get_mut::<UiImage>(entity)
        && (source.variants.len() != 4
            || !source
                .variants
                .iter()
                .zip(images)
                .all(|(variant, image)| variant.image == *image))
    {
        source.variants = images
            .iter()
            .enumerate()
            .map(|(index, image)| UiImageDensity {
                density: (index + 1) as f32,
                image: image.clone(),
            })
            .collect();
    }
    Ok(())
}
fn insertion_index(boxes: &[Rect], x: f32) -> usize {
    boxes
        .iter()
        .position(|bounds| x < bounds.center().x)
        .unwrap_or(boxes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guide_state_updates_the_authored_image_source_not_a_competing_node_writer() {
        use kedra_ui::authored::components::{UiImage, UiImageDensity};
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let old = world.resource_mut::<Assets<Image>>().add(Image::default());
        let next: Glyph =
            std::array::from_fn(|_| world.resource_mut::<Assets<Image>>().add(Image::default()));
        let entity = world
            .spawn((
                UiImage {
                    variants: vec![UiImageDensity {
                        density: 1.,
                        image: old.clone(),
                    }],
                    color: Color::WHITE,
                },
                ImageNode::new(old.clone()),
            ))
            .id();
        set_glyph(&mut world, entity, &next).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            world
                .get::<UiImage>(entity)
                .unwrap_or_else(|| panic!("source missing"))
                .variants[0]
                .image,
            next[0]
        );
        assert_eq!(
            world
                .get::<ImageNode>(entity)
                .unwrap_or_else(|| panic!("native image missing"))
                .image,
            old
        );
        set_glyph(
            &mut world,
            entity,
            &std::array::from_fn(|_| Handle::default()),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            world
                .get::<UiImage>(entity)
                .unwrap_or_else(|| panic!("source missing"))
                .variants[0]
                .image,
            next[0]
        );
    }
    #[test]
    fn insertion_uses_full_tab_boundaries_including_the_close_slot() {
        let boxes = [
            Rect::from_corners(Vec2::new(10., 4.), Vec2::new(110., 32.)),
            Rect::from_corners(Vec2::new(114., 4.), Vec2::new(214., 32.)),
        ];
        assert_eq!(insertion_index(&boxes, 9.), 0);
        assert_eq!(insertion_index(&boxes, 65.), 1);
        assert_eq!(insertion_index(&boxes, 213.), 2);
        let scrolled: Vec<_> = boxes
            .iter()
            .map(|r| Rect::from_corners(r.min - Vec2::X * 80., r.max - Vec2::X * 80.))
            .collect();
        assert_eq!(insertion_index(&scrolled, 100.), 2);
    }
    #[test]
    fn compass_has_five_nonoverlapping_28px_targets_at_32px_pitch() {
        let center = Vec2::new(600., 400.);
        for i in 0..5 {
            let bounds = guide_bounds(center, i);
            assert_eq!(bounds.size(), Vec2::splat(28.));
            assert!(bounds.contains(bounds.center()));
            for j in 0..i {
                assert!(!guide_bounds(center, j).contains(bounds.center()));
            }
        }
    }
}
