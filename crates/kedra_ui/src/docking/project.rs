//! Keyed reconciliation: chrome may retire; registered content never does.
use super::{Control, Divider, GroupTarget, TabLabel, TabRail};
use crate::authored::view::UiView;
use bevy::prelude::*;
use bevy_flair::{prelude::InlineStyle, style::components::ClassList};
use kedra_docking::{
    model::{Axis, HostId, Node, NodeId, ViewId},
    snapshot::Snapshot,
};
use kedra_scene::instance::CompiledPrefab;
use std::{collections::BTreeMap, sync::Arc};
#[path = "staging.rs"]
mod staging;

/// Compiled, dependency-ready authored chrome shared by product and measurement.
pub struct Skin {
    /// Group with `tabs`, `body` and one common `menu` binding.
    pub group: Arc<CompiledPrefab>,
    /// Tab with `label`, `title` and `close` bindings.
    pub tab: Arc<CompiledPrefab>,
    /// Split with `divider` binding; children are inserted around the divider.
    pub split: Arc<CompiledPrefab>,
}
enum Chrome {
    Group {
        view: UiView,
        tabs: Entity,
        body: Entity,
    },
    Split {
        view: UiView,
        divider: Entity,
    },
}
impl Chrome {
    fn root(&self) -> Entity {
        match self {
            Self::Group { view, .. } | Self::Split { view, .. } => view.root,
        }
    }
}
struct Tab {
    view: UiView,
    title: Entity,
    label: Entity,
    fitting: super::title::Title,
}

/// One retained projection, independent of model authority and native lifecycle.
///
/// Register content once, then call `sync` with committed or candidate topology.
/// A separate measurement projection uses identical chrome without content roots.
/// Content retirement remains feature-owned, including approved document closure.
#[derive(Default)]
pub struct Projection {
    nodes: BTreeMap<NodeId, Chrome>,
    tabs: BTreeMap<ViewId, Tab>,
    content: BTreeMap<ViewId, Entity>,
    focus: BTreeMap<ViewId, Entity>,
    active: BTreeMap<NodeId, ViewId>,
}
impl Projection {
    /// Register a feature root exactly once. Does not reparent until sync.
    pub fn register(&mut self, id: ViewId, root: Entity) -> Result<(), String> {
        if self.content.contains_key(&id) || self.content.values().any(|e| *e == root) {
            return Err("Docking content is already registered".into());
        }
        self.content.insert(id, root);
        Ok(())
    }
    /// Borrow a registered root for feature updates or lifetime assertions.
    pub fn content(&self, id: ViewId) -> Option<Entity> {
        self.content.get(&id).copied()
    }
    /// Laid-out group root, used for candidate/committed geometry comparison.
    pub fn group(&self, id: NodeId) -> Option<Entity> {
        self.nodes.get(&id).map(Chrome::root)
    }
    /// Retained draggable label for a registered visible instance.
    pub fn label(&self, id: ViewId) -> Option<Entity> {
        self.tabs.get(&id).map(|tab| tab.label)
    }
    /// Full tab box, including its stable action slot, for insertion boundaries.
    pub fn tab(&self, id: ViewId) -> Option<Entity> {
        self.tabs.get(&id).map(|tab| tab.view.root)
    }
    /// The committed tab rail, excluding any other projection's measurement tree.
    pub fn rail(&self, id: NodeId) -> Option<Entity> {
        match self.nodes.get(&id) {
            Some(Chrome::Group { tabs, .. }) => Some(*tabs),
            _ => None,
        }
    }
    /// Recursive minimum constraints from resolved authored CSS, not pixel layout.
    /// Bevy/Taffy still computes all resulting boxes and rounding.
    pub fn minimum(&self, world: &World, state: &Snapshot, id: NodeId) -> Vec2 {
        let maximized = state
            .host_of(id)
            .and_then(|host| state.hosts[&host].maximized);
        if maximized.is_some_and(|target| !subtree_contains(state, id, target)) {
            return Vec2::ZERO;
        }
        match state.nodes.get(&id) {
            Some(Node::Group { .. }) => self
                .nodes
                .get(&id)
                .and_then(|chrome| world.get::<bevy::ui::Node>(chrome.root()))
                .map(|node| Vec2::new(pixels(node.min_width), pixels(node.min_height)))
                .unwrap_or(Vec2::ZERO),
            Some(Node::Split {
                axis,
                first,
                second,
                ..
            }) => {
                let a = self.minimum(world, state, *first);
                let b = self.minimum(world, state, *second);
                let divider = if maximized.is_some() {
                    0.
                } else {
                    self.divider_size(world, id, *axis)
                };
                match axis {
                    Axis::Horizontal => Vec2::new(a.x + b.x + divider, a.y.max(b.y)),
                    Axis::Vertical => Vec2::new(a.x.max(b.x), a.y + b.y + divider),
                }
            }
            _ => Vec2::ZERO,
        }
    }
    /// Resolved divider extent on the split axis. Zero until CSS is ready.
    pub fn divider_size(&self, world: &World, id: NodeId, axis: Axis) -> f32 {
        match self.nodes.get(&id) {
            Some(Chrome::Split { divider, .. }) => world
                .get::<bevy::ui::Node>(*divider)
                .map(|node| {
                    pixels(if axis == Axis::Horizontal {
                        node.width
                    } else {
                        node.height
                    })
                })
                .unwrap_or(0.),
            _ => 0.,
        }
    }
    /// Propagate minimum constraints through native flex containers. Idempotent;
    /// does not modify ratios or maintain a competing layout solver.
    pub fn constrain(&self, world: &mut World, state: &Snapshot) {
        for tab in self.tabs.values() {
            tab.fitting.fit(world, tab.title, tab.label);
        }
        for (id, node) in &state.nodes {
            if !matches!(node, Node::Split { .. }) {
                continue;
            }
            let minimum = self.minimum(world, state, *id);
            if let Some(chrome) = self.nodes.get(id) {
                set_minimum(world, chrome.root(), minimum);
            }
        }
    }
    /// Detach approved content from this registry without despawning it.
    pub fn unregister(&mut self, id: ViewId) -> Option<Entity> {
        self.focus.remove(&id);
        self.content.remove(&id)
    }

    /// Reconcile topology without reconstructing content or existing tab labels.
    ///
    /// `hosts` maps logical hosts to live styled roots. `stash` is a styled hidden
    /// container owned by the consumer, keeping inactive/hidden roots alive.
    /// Missing bindings, retired roots or authored mounting errors are explicit.
    /// Callers must retain host roots until content transfer completes.
    /// Rejection preserves published attachments, content and focus. New chrome
    /// is staged under a temporary owner and retired on failure; successful sync
    /// retains existing entities. Arbitrary external ECS observer side effects are
    /// not a rollback transaction and must not retire/reparent projection owners.
    #[allow(
        clippy::too_many_arguments,
        reason = "Explicit projection ownership inputs"
    )]
    pub fn sync(
        &mut self,
        world: &mut World,
        state: &Snapshot,
        hosts: &BTreeMap<HostId, Entity>,
        stash: Entity,
        skin: &Skin,
        titles: &BTreeMap<ViewId, String>,
        scope: &str,
        density: f32,
    ) -> Result<(), String> {
        state.validate().map_err(|e| e.to_string())?;
        for entity in hosts
            .values()
            .chain(std::iter::once(&stash))
            .chain(self.content.values())
        {
            if !world.entities().contains(*entity) {
                return Err("Docking projection contains a retired root".into());
            }
        }
        for host in state.hosts.keys() {
            if !hosts.contains_key(host) {
                return Err("Docking host is not ready for projection".into());
            }
        }
        let prepared = self.prepare(world, state, stash, skin, scope, density)?;
        // All fallible validation/allocation precedes the publication boundary.
        prepared.publish(world, self, stash);
        self.reconcile_focus(world, state);
        // Park feature roots before retiring any chrome ancestor. This is the
        // lifetime boundary: despawning a stale group must never despawn content.
        for root in self.content.values() {
            world.entity_mut(*root).insert(ChildOf(stash));
        }
        for tab in self.tabs.values() {
            world.entity_mut(tab.view.root).insert(ChildOf(stash));
        }
        // Detach all node roots before retiring splits whose children may survive.
        for chrome in self.nodes.values() {
            world.entity_mut(chrome.root()).insert(ChildOf(stash));
        }
        let stale: Vec<_> = self
            .nodes
            .keys()
            .filter(|id| !state.nodes.contains_key(id))
            .copied()
            .collect();
        for id in stale {
            if let Some(chrome) = self.nodes.remove(&id) {
                world.despawn(chrome.root());
            }
        }
        let stale: Vec<_> = self
            .tabs
            .keys()
            .filter(|id| !state.views.contains_key(id))
            .copied()
            .collect();
        for id in stale {
            if let Some(tab) = self.tabs.remove(&id) {
                world.despawn(tab.view.root);
            }
        }
        for (host_id, host) in &state.hosts {
            if let Some(root) = host.root {
                self.attach(
                    world,
                    state,
                    root,
                    hosts[host_id],
                    *host_id,
                    host.maximized,
                    titles,
                    scope,
                );
            }
        }
        Ok(())
    }

    fn reconcile_focus(&mut self, world: &mut World, state: &Snapshot) {
        use bevy::input_focus::{FocusCause, InputFocus};
        let focused = world.get_resource::<InputFocus>().and_then(InputFocus::get);
        let owner = focused.and_then(|focus| {
            self.content
                .iter()
                .find_map(|(view, root)| is_under(world, focus, *root).then_some(*view))
        });
        if let (Some(view), Some(focus)) = (owner, focused) {
            self.focus.insert(view, focus);
        }
        let visible = |view: ViewId| {
            state.location(view).is_some_and(|(group, _)| {
                matches!(state.nodes.get(&group),Some(Node::Group{active,..})if *active==view)
                    && state.host_of(group).is_some_and(|host| {
                        state.hosts[&host].maximized.is_none_or(|max| max == group)
                    })
            })
        };
        if owner.is_some_and(|view| !visible(view))
            && let Some(mut focus) = world.get_resource_mut::<InputFocus>()
        {
            focus.clear();
        }
        let active: BTreeMap<_, _> = state
            .nodes
            .iter()
            .filter_map(|(group, node)| match node {
                Node::Group { active, .. } => Some((*group, *active)),
                _ => None,
            })
            .collect();
        if world
            .get_resource::<InputFocus>()
            .is_some_and(|focus| focus.get().is_none())
        {
            let restore = active
                .iter()
                .find_map(|(group, view)| {
                    (self.active.get(group) != Some(view) && visible(*view))
                        .then(|| self.focus.get(view).copied())
                        .flatten()
                })
                .filter(|entity| world.entities().contains(*entity));
            if let Some(entity) = restore
                && let Some(mut focus) = world.get_resource_mut::<InputFocus>()
            {
                focus.set(entity, FocusCause::Navigated);
            }
        }
        self.active = active;
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Recursive projection carries the same explicit ownership inputs"
    )]
    fn attach(
        &mut self,
        world: &mut World,
        state: &Snapshot,
        id: NodeId,
        parent: Entity,
        host: HostId,
        maximized: Option<NodeId>,
        titles: &BTreeMap<ViewId, String>,
        scope: &str,
    ) {
        // prepare validated every kind/binding and mounted all missing keys.
        let chrome = &self.nodes[&id];
        let root = chrome.root();
        world.entity_mut(root).insert(ChildOf(parent));
        let visible =
            maximized.is_none_or(|group| subtree_contains(state, id, group) || id == group);
        class(world, root, "hidden", !visible);
        match (&state.nodes[&id], chrome) {
            (
                Node::Group {
                    tabs: order,
                    active,
                },
                Chrome::Group { tabs, body, .. },
            ) => {
                let (tabs, body) = (*tabs, *body);
                world.entity_mut(tabs).insert(TabRail { host, group: id });
                if world.get::<ScrollPosition>(tabs).is_none() {
                    world.entity_mut(tabs).insert(ScrollPosition::default());
                }
                world
                    .entity_mut(root)
                    .insert(crate::ElementId::new(format!("{scope}/group/{id:?}")));
                world
                    .entity_mut(root)
                    .insert(GroupTarget { group: id, host });
                for view_id in order {
                    let tab = &self.tabs[view_id];
                    world
                        .entity_mut(tab.view.root)
                        .insert(crate::ElementId::new(format!("{scope}/tab/{view_id:?}")));
                    world.entity_mut(tab.view.root).insert(ChildOf(tabs));
                    world.entity_mut(tab.label).insert(TabLabel {
                        view: *view_id,
                        group: id,
                        host,
                    });
                    class(world, tab.view.root, "active", *view_id == *active);
                    tab.fitting.set(
                        world,
                        titles.get(view_id).unwrap_or(&state.views[view_id].kind),
                    );
                    if let Some(mut text) = world.get_mut::<Text>(tab.title) {
                        let next = titles
                            .get(view_id)
                            .cloned()
                            .unwrap_or_else(|| state.views[view_id].kind.clone());
                        if text.0 != next {
                            text.0 = next;
                        }
                    }
                    if *view_id == *active
                        && let Some(content) = self.content.get(view_id)
                    {
                        world.entity_mut(*content).insert(ChildOf(body));
                    }
                }
                let children: Vec<_> = order.iter().map(|id| self.tabs[id].view.root).collect();
                world.entity_mut(tabs).replace_children(&children);
                flex(world, root, 1.);
            }
            (
                Node::Split {
                    axis,
                    fraction,
                    first,
                    second,
                },
                Chrome::Split { divider, .. },
            ) => {
                let divider = *divider;
                if world
                    .get::<bevy::picking::hover::Hovered>(divider)
                    .is_none()
                {
                    world
                        .entity_mut(divider)
                        .insert(bevy::picking::hover::Hovered::default());
                }
                world.entity_mut(divider).insert((
                    Divider { split: id, host },
                    Control::ResetSplit(id),
                    bevy::ui_widgets::Button,
                    bevy::input_focus::tab_navigation::TabIndex(0),
                    bevy::picking::cursor::EntityCursor::System(if *axis == Axis::Horizontal {
                        bevy::window::SystemCursorIcon::EwResize
                    } else {
                        bevy::window::SystemCursorIcon::NsResize
                    }),
                ));
                class(world, root, "vertical", *axis == Axis::Vertical);
                class(world, root, "collapsed", maximized.is_some());
                self.attach(world, state, *first, root, host, maximized, titles, scope);
                self.attach(world, state, *second, root, host, maximized, titles, scope);
                let first = self.nodes[first].root();
                let second = self.nodes[second].root();
                // Flex grow below one deliberately leaves unused space when
                // its sibling is display:none. A maximized path must fill each
                // ancestor, not retain its former fraction of that ancestor.
                let (a, b) = if maximized.is_some() {
                    (1., 1.)
                } else {
                    (*fraction, 1. - fraction)
                };
                flex(world, first, a);
                flex(world, second, b);
                world
                    .entity_mut(root)
                    .replace_children(&[first, divider, second]);
            }
            _ => unreachable!("Chrome kinds were checked before publication"),
        }
    }
}
fn subtree_contains(state: &Snapshot, root: NodeId, target: NodeId) -> bool {
    root == target
        || match state.nodes.get(&root) {
            Some(Node::Split { first, second, .. }) => {
                subtree_contains(state, *first, target) || subtree_contains(state, *second, target)
            }
            _ => false,
        }
}
fn element(view: &UiView, key: &str) -> Result<Entity, String> {
    view.element(key).map_err(|e| e.to_string())
}
/// Set a transient selector without replacing authored visual values.
pub fn class(world: &mut World, entity: Entity, name: &'static str, value: bool) {
    if let Some(mut classes) = world.get_mut::<ClassList>(entity)
        && classes.contains(name) != value
    {
        classes.toggle(name);
    }
}
/// The dynamic docking-property vocabulary. No arbitrary CSS or visual tokens
/// are accepted; authored styles own all remaining presentation.
pub enum Geometry {
    /// Native flex weight in `[0,1]`, with a zero-pixel basis.
    Flex(f32),
    /// A logical rectangle relative to its parent, with finite nonnegative extent.
    Rectangle(Rect),
}
/// Apply validated geometry through Flair's inline-property path. Invalid numbers
/// and retired entities fail before mutation. CSS declaration strings cannot enter
/// this API; Flair's lenient InlineStyle parser is not a validation boundary.
///
/// ```compile_fail
/// # use bevy::prelude::*;
/// # let mut world=World::new(); let entity=world.spawn_empty().id();
/// kedra_ui::docking::geometry(&mut world,entity,"not valid css{".into());
/// ```
pub fn geometry(world: &mut World, entity: Entity, value: Geometry) -> Result<(), String> {
    if !world.entities().contains(entity) {
        return Err("Dynamic docking target was retired".into());
    }
    let mut style = world
        .get::<InlineStyle>(entity)
        .cloned()
        .unwrap_or_default();
    match value {
        Geometry::Flex(weight) => {
            if !weight.is_finite() || !(0. ..=1.).contains(&weight) {
                return Err("Docking flex weight is outside [0,1]".into());
            }
            style.set("flex-grow", weight.to_string());
            style.set("flex-basis", "0px");
        }
        Geometry::Rectangle(rect) => {
            let size = rect.size();
            if !rect.min.is_finite()
                || !rect.max.is_finite()
                || !size.is_finite()
                || size.min_element() < 0.
            {
                return Err("Docking rectangle is not finite and nonnegative".into());
            }
            for (name, value) in [
                ("left", rect.min.x),
                ("top", rect.min.y),
                ("width", size.x),
                ("height", size.y),
            ] {
                style.set(name, format!("{value}px"));
            }
        }
    }
    if world.get::<InlineStyle>(entity) != Some(&style) {
        world.entity_mut(entity).insert(style);
    }
    Ok(())
}

// Internal publication only receives live prepared roots and validated snapshot
// fractions. Public arbitrary geometry continues to use the checked API above.
fn flex(world: &mut World, entity: Entity, weight: f32) {
    let mut style = world
        .get::<InlineStyle>(entity)
        .cloned()
        .unwrap_or_default();
    style.set("flex-grow", weight.to_string());
    style.set("flex-basis", "0px");
    if world.get::<InlineStyle>(entity) != Some(&style) {
        world.entity_mut(entity).insert(style);
    }
}
fn pixels(value: Val) -> f32 {
    match value {
        Val::Px(value) => value,
        _ => 0.,
    }
}
fn is_under(world: &World, mut entity: Entity, root: Entity) -> bool {
    for _ in 0..128 {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
fn set_minimum(world: &mut World, entity: Entity, minimum: Vec2) {
    let mut style = world
        .get::<InlineStyle>(entity)
        .cloned()
        .unwrap_or_default();
    style.set("min-width", format!("{}px", minimum.x));
    style.set("min-height", format!("{}px", minimum.y));
    if world.get::<InlineStyle>(entity) != Some(&style) {
        world.entity_mut(entity).insert(style);
    }
}
