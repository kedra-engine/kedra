//! Retained source hierarchy. Shared documents own selection and structural history.
mod input;
mod model;
#[cfg(test)]
mod tests;
mod view;
use crate::documents::Documents;
use bevy::prelude::*;
use kedra_docking::model::ViewId;
use kedra_scene::document::{DocumentId, NodeAddress};
use kedra_ui::authored::view::UiView;
use model::{Navigation, Tree};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) enum Action {
    Select(NodeAddress),
    Toggle(NodeAddress),
    Key(NodeAddress, KeyCode),
    Reveal,
    CloseSearch,
    Details,
    FocusSearch,
}
#[derive(Component, Clone)]
pub(crate) struct Binding {
    pub view: ViewId,
    pub document: Option<DocumentId>,
    pub action: Action,
}
pub(crate) struct Panel {
    view: UiView,
    document: Option<DocumentId>,
    tree: Option<Tree>,
    graph: Option<std::sync::Arc<kedra_scene::composition::ResolvedGraph>>,
    navigation: BTreeMap<DocumentId, Navigation>,
    rows: BTreeMap<NodeAddress, UiView>,
    visible: Vec<(usize, bool)>,
    dirty: bool,
    window: (usize, usize, usize),
    selected: Option<NodeAddress>,
    selected_set: Vec<NodeAddress>,
    range_anchor: Option<NodeAddress>,
    focus: Option<NodeAddress>,
    search_open: bool,
    details_open: bool,
    filter_hover: Option<std::time::Duration>,
}
impl Panel {
    pub fn mount(world: &mut World, parent: Entity, id: ViewId) -> Result<Self, String> {
        let view = crate::views::mount_asset(
            world,
            parent,
            "editor://prefabs/hierarchy.prefab.ron",
            &format!("hierarchy/{id:?}"),
        )?;
        world
            .entity_mut(view.element("close-search").map_err(|e| e.to_string())?)
            .insert(Binding {
                view: id,
                document: None,
                action: Action::CloseSearch,
            });
        world.entity_mut(view.root).insert(Binding {
            view: id,
            document: None,
            action: Action::FocusSearch,
        });
        let search = view.element("search").map_err(|e| e.to_string())?;
        if let Some(mut text) = world.get_mut::<bevy::text::EditableText>(search) {
            text.max_characters = Some(256);
            text.allow_newlines = false;
        }
        world
            .entity_mut(search)
            .insert(bevy::text::EditableTextFilter::new(|c| !c.is_control()));
        world
            .entity_mut(view.element("placeholder").map_err(|e| e.to_string())?)
            .insert(bevy::picking::Pickable::IGNORE);
        world
            .entity_mut(view.element("search-icon").map_err(|e| e.to_string())?)
            .insert(bevy::picking::Pickable::IGNORE);
        world
            .entity_mut(view.element("filter").map_err(|e| e.to_string())?)
            .insert(bevy::ui::InteractionDisabled);
        world
            .entity_mut(view.element("hint").map_err(|e| e.to_string())?)
            .insert(bevy::picking::Pickable::IGNORE);
        let area = view.element("entries").map_err(|e| e.to_string())?;
        world
            .entity_mut(area)
            .insert(crate::asset_browser::ScrollRegion(0));
        world
            .entity_mut(view.element("status").map_err(|e| e.to_string())?)
            .insert(crate::asset_browser::ScrollRegion(1));
        world
            .entity_mut(view.element("message").map_err(|e| e.to_string())?)
            .insert(crate::asset_browser::ScrollRegion(1));
        world
            .entity_mut(view.element("track").map_err(|e| e.to_string())?)
            .insert(bevy::ui_widgets::Scrollbar::new(
                area,
                bevy::ui_widgets::ControlOrientation::Vertical,
                24.,
            ));
        world
            .entity_mut(view.element("thumb").map_err(|e| e.to_string())?)
            .remove::<Node>()
            .insert(bevy::ui_widgets::ScrollbarThumb::default());
        Ok(Self {
            view,
            document: None,
            tree: None,
            graph: None,
            navigation: BTreeMap::new(),
            rows: BTreeMap::new(),
            visible: Vec::new(),
            dirty: true,
            window: (0, 0, 0),
            selected: None,
            selected_set: Vec::new(),
            range_anchor: None,
            focus: None,
            search_open: false,
            details_open: false,
            filter_hover: None,
        })
    }
    pub fn root(&self) -> Entity {
        self.view.root
    }
    fn element(&self, key: &str) -> Result<Entity, String> {
        self.view.element(key).map_err(|e| e.to_string())
    }
    fn text(&self, world: &mut World, key: &str, value: String) -> Result<(), String> {
        let mut text = world
            .get_mut::<Text>(self.element(key)?)
            .ok_or("Hierarchy caption missing")?;
        if text.0 != value {
            text.0 = value;
        }
        Ok(())
    }
    fn search(&self, world: &mut World, value: &str) -> Result<(), String> {
        if let Some(mut text) = world.get_mut::<bevy::text::EditableText>(self.element("search")?)
            && text.value() != value
        {
            text.editor_mut().set_text(value);
            text.queue_edit(bevy::text::TextEdit::TextEnd(false));
        }
        Ok(())
    }
    pub fn tick(
        &mut self,
        world: &mut World,
        id: ViewId,
        documents: &Documents,
    ) -> Result<(), String> {
        let area = self.element("entries")?;
        let active = documents.active.filter(|id| documents.contains(*id));
        let graph = active
            .and_then(|id| documents.opened.get(&id))
            .map(|doc| doc.graph.clone());
        if self.document != active {
            self.range_anchor = None;
            if let Some(previous) = self.document
                && let Some(navigation) = self.navigation.get_mut(&previous)
            {
                navigation.scroll = world.get::<ScrollPosition>(area).map_or(0., |p| p.y);
            }
            self.document = active;
            self.tree = active
                .and_then(|doc| documents.opened.get(&doc))
                .map(|doc| Tree::new(&doc.graph));
            for (_, row) in std::mem::take(&mut self.rows) {
                world.despawn(row.root);
            }
            let (filter, scroll) = if let Some(doc) = self.document {
                let nav = self.navigation.entry(doc).or_default();
                (nav.filter.clone(), nav.scroll)
            } else {
                (String::new(), 0.)
            };
            self.search(world, &filter)?;
            self.search_open = !filter.is_empty();
            if let Some(mut position) = world.get_mut::<ScrollPosition>(area) {
                position.y = scroll;
            }
            self.focus = None;
            self.dirty = true;
        }
        let changed = match (&self.graph, &graph) {
            (Some(old), Some(new)) => !std::sync::Arc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };
        if changed {
            self.tree = graph.as_ref().map(|graph| Tree::new(graph));
            self.graph = graph;
            for (_, row) in std::mem::take(&mut self.rows) {
                world.despawn(row.root);
            }
            self.range_anchor = self.range_anchor.take().filter(|address| {
                self.tree
                    .as_ref()
                    .is_some_and(|tree| tree.indices.contains_key(address))
            });
            self.focus = self.focus.take().filter(|address| {
                self.tree
                    .as_ref()
                    .is_some_and(|tree| tree.indices.contains_key(address))
            });
            self.dirty = true;
        }
        let selected_set = documents
            .active
            .map(|document| documents.selected_nodes(document))
            .unwrap_or_default();
        let selected = selected_set.last().cloned();
        if self.selected != selected || self.selected_set != selected_set {
            self.selected = selected;
            self.selected_set = selected_set;
            self.dirty = true;
        }
        if let Some(doc) = self.document {
            let query = world
                .get::<bevy::text::EditableText>(self.element("search")?)
                .map(|t| t.value().to_string())
                .unwrap_or_default();
            let navigation = self.navigation.entry(doc).or_default();
            if query != navigation.filter {
                if navigation.filter.is_empty() {
                    navigation.unfiltered_scroll =
                        world.get::<ScrollPosition>(area).map_or(0., |p| p.y);
                }
                let scroll = if query.is_empty() {
                    navigation.unfiltered_scroll
                } else {
                    0.
                };
                navigation.filter = query;
                if let Some(mut position) = world.get_mut::<ScrollPosition>(area) {
                    position.y = scroll;
                }
                self.dirty = true;
            }
        }
        if self.window.2 > 0
            && world.contains_resource::<bevy::render::renderer::RenderDevice>()
            && world
                .get::<ComputedNode>(self.view.root)
                .is_some_and(ComputedNode::is_empty)
        {
            return Ok(());
        }
        if self.dirty {
            self.visible = match (&self.tree, self.document) {
                (Some(tree), Some(doc)) => tree.visible(&self.navigation[&doc]),
                _ => Vec::new(),
            };
        }
        let window = self.viewport(world)?;
        if self.dirty || self.window != window {
            self.render(world, id, window)?;
            self.window = window;
            self.dirty = false;
        }
        self.focus_style(world)?;
        let hovered = self.search_open
            && world
                .get::<bevy::picking::hover::Hovered>(self.element("filter")?)
                .is_some_and(|h| h.0);
        let now = world
            .get_resource::<Time<Real>>()
            .map_or(std::time::Duration::ZERO, Time::elapsed);
        if hovered {
            self.filter_hover.get_or_insert(now);
        } else {
            self.filter_hover = None;
        }
        let show = self
            .filter_hover
            .is_some_and(|start| now.saturating_sub(start).as_millis() >= 350);
        kedra_ui::docking::class(world, self.element("hint")?, "hidden", !show);
        let message = if let Some(error) = &documents.failure {
            format!(
                "Open failed: {error}{}",
                if documents.active.is_some() {
                    "\nShowing the previously admitted document."
                } else {
                    ""
                }
            )
        } else if documents.active.is_some() {
            String::new()
        } else if documents.opening() {
            "Opening source document…".into()
        } else {
            "Open a scene or prefab from Assets to see its nodes.".into()
        };
        self.text(world, "message", message.clone())?;
        kedra_ui::docking::class(
            world,
            self.element("message")?,
            "hidden",
            message.is_empty(),
        );
        let overflowing = world
            .get::<ComputedNode>(area)
            .is_some_and(|node| node.content_size().y > node.size().y + 1.);
        world
            .entity_mut(self.element("thumb")?)
            .insert(if overflowing {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        Ok(())
    }
}
pub(crate) fn install(app: &mut App) {
    input::install(app);
}
