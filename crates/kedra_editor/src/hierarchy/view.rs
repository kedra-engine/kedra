//! Authored row virtualization over native scroll/layout geometry.
use super::*;
use bevy_flair::prelude::InlineStyle;
use kedra_ui::{
    ElementId,
    docking::{class, rect},
};
impl Panel {
    pub(super) fn focus_style(&self, world: &mut World) -> Result<(), String> {
        use bevy::input_focus::{InputFocus, IsFocused};
        let valid_focus = world
            .get_resource::<InputFocus>()
            .and_then(InputFocus::get)
            .is_some_and(|entity| world.entities().contains(entity));
        let active = valid_focus && world.is_focus_within(self.view.root);
        class(world, self.view.root, "inactive", !active);
        for (address, view) in &self.rows {
            let hovered = ["select", "toggle"]
                .into_iter()
                .filter_map(|key| view.element(key).ok())
                .any(|entity| {
                    world
                        .get::<bevy::picking::hover::Hovered>(entity)
                        .is_some_and(|hover| hover.0)
                });
            class(world, view.root, "row-hover", hovered);
            class(
                world,
                view.root,
                "keyboard-focus",
                valid_focus && world.is_focus_within_visible(view.root),
            );
            let Some(row) = self
                .tree
                .as_ref()
                .and_then(|tree| tree.indices.get(address).map(|index| &tree.rows[*index]))
            else {
                continue;
            };
            let generic = row.parent.is_some() && !row.children && !row.boundary;
            let selected = active && self.selected.as_ref() == Some(address);
            class(
                world,
                view.element("object").map_err(|e| e.to_string())?,
                "hidden",
                !generic || selected,
            );
            class(
                world,
                view.element("object-selected").map_err(|e| e.to_string())?,
                "hidden",
                !generic || !selected,
            );
        }
        Ok(())
    }
    pub(super) fn viewport(&self, world: &World) -> Result<(usize, usize, usize), String> {
        let area = self.element("entries")?;
        let bounds = rect(world, area);
        let scroll = world.get::<ScrollPosition>(area).map_or(0., |p| p.y);
        let first = ((scroll / 22.).floor().max(0.) as usize)
            .saturating_sub(1)
            .min(self.visible.len());
        let end = (first + (bounds.map_or(520., |r| r.height()) / 22.).ceil().max(1.) as usize + 3)
            .min(self.visible.len());
        Ok((
            first,
            end,
            bounds.map_or(240., |r| r.width()).max(0.) as usize,
        ))
    }
    pub(super) fn render(
        &mut self,
        world: &mut World,
        id: ViewId,
        window: (usize, usize, usize),
    ) -> Result<(), String> {
        let selected = self.selected.clone();
        let filter = self
            .document
            .and_then(|d| self.navigation.get(&d))
            .map(|n| n.filter.as_str())
            .unwrap_or("");
        class(
            world,
            self.element("placeholder")?,
            "hidden",
            !filter.is_empty(),
        );
        class(world, self.element("toolbar")?, "hidden", !self.search_open);
        class(world, self.element("status")?, "hidden", !self.details_open);
        world
            .entity_mut(self.element("close-search")?)
            .insert(Binding {
                view: id,
                document: self.document,
                action: Action::CloseSearch,
            });
        world.entity_mut(self.view.root).insert(Binding {
            view: id,
            document: self.document,
            action: Action::FocusSearch,
        });
        let canvas = self.element("canvas")?;
        let width = window.2.saturating_sub(8).max(
            self.visible
                .iter()
                .map(|(i, _)| {
                    self.tree
                        .as_ref()
                        .map_or(0, |t| t.rows[*i].depth * 12 + 160)
                })
                .max()
                .unwrap_or(0),
        );
        inline(
            world,
            canvas,
            [
                ("height", format!("{}px", self.visible.len() * 22 + 8)),
                ("min-width", format!("{width}px")),
            ],
        );
        let desired: Vec<_> = self.visible[window.0..window.1]
            .iter()
            .enumerate()
            .map(|(offset, (index, context))| (window.0 + offset, *index, *context))
            .collect();
        self.rows.retain(|address, row| {
            if desired.iter().any(|(_, index, _)| {
                self.tree
                    .as_ref()
                    .is_some_and(|t| &t.rows[*index].address == address)
            }) {
                true
            } else {
                world.despawn(row.root);
                false
            }
        });
        for (position, index, context) in desired {
            let tree = self.tree.as_ref().ok_or("Hierarchy graph missing")?;
            let row = &tree.rows[index];
            if !self.rows.contains_key(&row.address) {
                let view = crate::views::mount(
                    world,
                    canvas,
                    crate::views::document(0x11e10000000040008000000000000001),
                    &format!("hierarchy/{id:?}/row/{}", uuid::Uuid::new_v4()),
                )?;
                self.rows.insert(row.address.clone(), view);
            }
            let view = &self.rows[&row.address];
            world
                .entity_mut(view.root)
                .insert(ElementId::new(format!("hierarchy/{id:?}/row/{index}")));
            inline(
                world,
                view.root,
                [
                    ("top", format!("{}px", 4 + position * 22)),
                    ("left", format!("{}px", 4 + row.depth * 12)),
                    (
                        "width",
                        format!("{}px", width.saturating_sub(row.depth * 12)),
                    ),
                ],
            );
            let node = view.element("select").map_err(|e| e.to_string())?;
            let toggle = view.element("toggle").map_err(|e| e.to_string())?;
            for (entity, action) in [
                (node, Action::Select(row.address.clone())),
                (toggle, Action::Toggle(row.address.clone())),
            ] {
                world.entity_mut(entity).insert(Binding {
                    view: id,
                    document: self.document,
                    action,
                });
            }
            world
                .entity_mut(node)
                .insert(ElementId::new(format!("hierarchy/{id:?}/node/{index}")));
            world
                .entity_mut(toggle)
                .insert(ElementId::new(format!("hierarchy/{id:?}/toggle/{index}")));
            let name = view.element("name").map_err(|e| e.to_string())?;
            world
                .get_mut::<Text>(name)
                .ok_or("Hierarchy name missing")?
                .0 = row.name.clone();
            let collapsed = self
                .document
                .and_then(|d| self.navigation.get(&d))
                .is_some_and(|n| n.filter.is_empty() && n.collapsed.contains(&row.address));
            for (key, hidden) in [
                // Keep the disclosure slot for leaves; only its glyph is hidden.
                ("toggle", false),
                ("down", !row.children || collapsed),
                ("right", !row.children || !collapsed),
                ("scene", index != 0),
                ("object", index == 0 || row.boundary || row.children),
                ("object-selected", true),
                ("folder", index == 0 || row.boundary || !row.children),
                ("prefab", !row.boundary),
            ] {
                class(
                    world,
                    view.element(key).map_err(|e| e.to_string())?,
                    "hidden",
                    hidden,
                );
            }
            class(
                world,
                view.root,
                "selected",
                selected.as_ref() == Some(&row.address),
            );
            class(world, view.root, "context", context);
            if row.children {
                world
                    .entity_mut(toggle)
                    .remove::<bevy::ui::InteractionDisabled>();
            } else {
                world
                    .entity_mut(toggle)
                    .insert(bevy::ui::InteractionDisabled);
            }
            if self.focus.as_ref() == Some(&row.address) {
                if let Some(mut visible) =
                    world.get_resource_mut::<bevy::input_focus::InputFocusVisible>()
                {
                    visible.0 = true;
                }
                world
                    .resource_mut::<bevy::input_focus::InputFocus>()
                    .set(node, bevy::input_focus::FocusCause::Navigated);
                self.focus = None;
            }
        }
        let count = self.tree.as_ref().map_or(0, |t| t.rows.len());
        let description = selected
            .as_ref()
            .and_then(|address| {
                self.tree
                    .as_ref()
                    .and_then(|t| t.indices.get(address).map(|i| &t.rows[*i]))
            })
            .map(|r| r.provenance.clone())
            .unwrap_or_default();
        self.text(
            world,
            "status",
            if count == 0 {
                String::new()
            } else {
                format!(
                    "Read-only · {}/{} nodes\n{description}",
                    self.visible.len(),
                    count
                )
            },
        )?;
        self.text(
            world,
            "empty",
            if self.document.is_some() && self.visible.is_empty() {
                "No nodes match this search.".into()
            } else {
                String::new()
            },
        )?;
        class(
            world,
            self.element("empty")?,
            "hidden",
            self.document.is_none() || !self.visible.is_empty(),
        );
        // No source edits or runtime entity mutation are performed by this view.
        Ok(())
    }
}
fn inline<const N: usize>(world: &mut World, entity: Entity, values: [(&str, String); N]) {
    let mut style = world
        .get::<InlineStyle>(entity)
        .cloned()
        .unwrap_or_default();
    for (key, value) in values {
        style.set(key, value);
    }
    world.entity_mut(entity).insert(style);
}
