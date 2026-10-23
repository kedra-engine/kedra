//! Dockable source target adaptation; generic value editors live in kedra_inspector.
mod input;
pub(crate) mod model;
mod view;
use bevy::prelude::*;
use kedra_docking::model::ViewId;
use kedra_inspector::{
    editors::EditorRegistry,
    view::{Disclosure, TreeView},
};
use kedra_ui::{authored::view::UiView, fields::FieldTemplates};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) enum Action {
    Search,
    CloseSearch,
    Lock,
    Raw,
    Details,
    Disclosure(Disclosure),
}
#[derive(Component, Clone)]
pub(crate) struct Binding {
    pub view: ViewId,
    pub action: Action,
}

pub(crate) struct Panel {
    view: UiView,
    pub(crate) state: model::State,
    templates: FieldTemplates,
    editors: EditorRegistry,
    components: BTreeMap<String, TreeView>,
    dirty: bool,
    filter: String,
    width: u32,
    limited: bool,
}
pub(crate) fn install(app: &mut App) {
    input::install(app);
}

pub(crate) fn templates(world: &World, density: f32) -> Result<FieldTemplates, String> {
    Ok(FieldTemplates {
        row: crate::views::compiled(world, "editor://prefabs/property-row.prefab.ron")?,
        value: crate::views::compiled(world, "editor://prefabs/property-value.prefab.ron")?,
        section: crate::views::compiled(world, "editor://prefabs/property-section.prefab.ron")?,
        density,
    })
}

impl Panel {
    pub fn mount(world: &mut World, parent: Entity, id: ViewId) -> Result<Self, String> {
        let density = world
            .resource::<kedra_desktop::DesktopSurface>()
            .window
            .and_then(|w| world.get::<Window>(w))
            .map_or(1., Window::scale_factor)
            * world.resource::<UiScale>().0;
        let templates = templates(world, density)?;
        let view = crate::views::mount_asset(
            world,
            parent,
            "editor://prefabs/inspector.prefab.ron",
            &format!("inspector/{id:?}"),
        )?;
        for (key, action) in [
            ("search", Action::Search),
            ("close-search", Action::CloseSearch),
        ] {
            world
                .entity_mut(view.element(key).map_err(|e| e.to_string())?)
                .insert(Binding { view: id, action });
        }
        world.entity_mut(view.root).insert(Binding {
            view: id,
            action: Action::Search,
        });
        let search = view.element("search").map_err(|e| e.to_string())?;
        if let Some(mut text) = world.get_mut::<bevy::text::EditableText>(search) {
            text.max_characters = Some(256);
            text.allow_newlines = false;
        }
        world
            .entity_mut(search)
            .insert(bevy::text::EditableTextFilter::new(|c| !c.is_control()));
        let area = view.element("entries").map_err(|e| e.to_string())?;
        world
            .entity_mut(area)
            .insert(crate::asset_browser::ScrollRegion(0));
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
            state: model::State::default(),
            templates,
            editors: EditorRegistry::default(),
            components: BTreeMap::new(),
            dirty: true,
            filter: String::new(),
            width: 0,
            limited: false,
        })
    }
    pub fn root(&self) -> Entity {
        self.view.root
    }
    fn element(&self, key: &str) -> Result<Entity, String> {
        self.view.element(key).map_err(|e| e.to_string())
    }
    fn text(&self, world: &mut World, key: &str, text: &str) -> Result<(), String> {
        let mut value = world
            .get_mut::<Text>(self.element(key)?)
            .ok_or("Inspector text binding is missing")?;
        if value.0 != text {
            value.0 = text.into();
        }
        Ok(())
    }
    fn clear(&mut self, world: &mut World) {
        for tree in self.components.values() {
            tree.retire(world);
        }
        self.components.clear();
    }
    pub fn owns(&self, disclosure: &Disclosure) -> bool {
        self.components
            .values()
            .any(|tree| tree.root() == disclosure.owner)
    }
    pub fn action(&mut self, world: &mut World, action: Action) -> Result<(), String> {
        match action {
            Action::Lock => self.state.toggle_lock(),
            Action::Raw => self.state.raw = !self.state.raw,
            Action::Details => self.state.details_open = !self.state.details_open,
            Action::Search => {
                self.state.search_open = true;
                kedra_ui::docking::class(world, self.element("toolbar")?, "hidden", false);
                world.resource_mut::<bevy::input_focus::InputFocus>().set(
                    self.element("search")?,
                    bevy::input_focus::FocusCause::Navigated,
                );
            }
            Action::CloseSearch => {
                self.state.search_open = false;
                if let Some(mut value) =
                    world.get_mut::<bevy::text::EditableText>(self.element("search")?)
                {
                    value.editor_mut().set_text("");
                }
                self.filter.clear();
                world
                    .resource_mut::<bevy::input_focus::InputFocus>()
                    .set(self.root(), bevy::input_focus::FocusCause::Navigated);
            }
            Action::Disclosure(disclosure) => {
                for tree in self.components.values_mut() {
                    tree.disclose(&disclosure);
                }
            }
        }
        self.dirty = true;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
