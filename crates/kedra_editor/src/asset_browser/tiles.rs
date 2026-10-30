//! A bounded virtual window over native scroll geometry; authored tiles are reused.
use super::*;
use bevy::input_focus::InputFocus;
use bevy_flair::prelude::InlineStyle;
use kedra_ui::{
    ElementId,
    docking::{class, rect},
};
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Window {
    pub first: usize,
    pub end: usize,
    pub columns: usize,
    pub width: usize,
}
impl Panel {
    pub(super) fn window(&self, world: &World) -> Result<Window, String> {
        let entries = self.element("entries")?;
        let bounds = rect(world, entries);
        let width = bounds.map_or(240., |r| r.width()).max(128.);
        let columns = if self.model.mode == model::Mode::List {
            1
        } else {
            ((width - 12.) / 84.).floor().max(1.) as usize
        };
        let height = if self.model.mode == model::Mode::List {
            22.
        } else {
            100.
        };
        let scroll = world.get::<ScrollPosition>(entries).map_or(0., |p| p.y);
        let count = self.model.visible().count();
        let first = ((scroll / height).floor().max(0.) as usize).saturating_sub(1) * columns;
        let rows = (bounds.map_or(320., |r| r.height()) / height)
            .ceil()
            .max(1.) as usize
            + 3;
        Ok(Window {
            first: first.min(count),
            end: (first + rows * columns).min(count),
            columns,
            width: width as usize,
        })
    }
    pub(super) fn render_tiles(&mut self, world: &mut World, id: ViewId) -> Result<(), String> {
        let entries = self.element("entries")?;
        if self.restore_scroll {
            if let Some(mut scroll) = world.get_mut::<ScrollPosition>(entries) {
                scroll.y = self.model.scroll();
            }
            self.restore_scroll = false;
        }
        let window = self.window(world)?;
        self.viewport = window;
        let desired: Vec<_> = self
            .model
            .visible()
            .enumerate()
            .skip(window.first)
            .take(window.end - window.first)
            .map(|(i, e)| (i, e.clone()))
            .collect();
        self.rows.retain(|(name, row)| {
            if desired.iter().any(|(_, e)| &e.name == name) {
                true
            } else {
                world.despawn(row.root);
                false
            }
        });
        let canvas = self.element("tile-canvas")?;
        let count = self.model.visible().count();
        let row_height = if self.model.mode == model::Mode::List {
            22
        } else {
            100
        };
        inline(
            world,
            canvas,
            [(
                "height",
                format!("{}px", count.div_ceil(window.columns) * row_height + 8),
            )],
        );
        class(world, canvas, "list", self.model.mode == model::Mode::List);
        for (index, entry) in desired {
            if !self.rows.iter().any(|(name, _)| name == &entry.name) {
                let row = crate::views::mount_asset(
                    world,
                    canvas,
                    "editor://prefabs/asset-source.prefab.ron",
                    &format!("browser/{id:?}/row/{}", uuid::Uuid::new_v4()),
                )?;
                self.rows.push((entry.name.clone(), row));
            }
            let row = &self
                .rows
                .iter()
                .find(|(name, _)| name == &entry.name)
                .ok_or("Browser tile missing")?
                .1;
            self.bind(world, row.root, id, Action::Select(entry.name.clone()));
            world
                .entity_mut(row.root)
                .insert(ElementId::new(format!("browser/{id:?}/item/{index}")));
            inline(
                world,
                row.root,
                [
                    ("left", format!("{}px", 8 + (index % window.columns) * 84)),
                    (
                        "top",
                        format!("{}px", 4 + (index / window.columns) * row_height),
                    ),
                    (
                        "width",
                        format!(
                            "{}px",
                            if self.model.mode == model::Mode::List {
                                window.width.saturating_sub(16)
                            } else {
                                80
                            }
                        ),
                    ),
                ],
            );
            world
                .get_mut::<Text>(row.element("name").map_err(|e| e.to_string())?)
                .ok_or("Source name missing")?
                .0 = entry.name.to_string_lossy().into_owned();
            let kind = if entry.kind == Kind::File {
                entry
                    .name
                    .to_str()
                    .and_then(|n| n.rsplit_once('.'))
                    .map(|(_, e)| e.to_uppercase())
                    .unwrap_or_else(|| "File".into())
            } else {
                entry.kind.label().into()
            };
            world
                .get_mut::<Text>(row.element("kind").map_err(|e| e.to_string())?)
                .ok_or("Source type missing")?
                .0 = format!(
                "{kind}{}",
                entry
                    .bytes
                    .map_or_else(String::new, |b| if self.model.mode == model::Mode::List {
                        format!(" · {b} B")
                    } else {
                        String::new()
                    })
            );
            world
                .get_mut::<Text>(row.element("badge").map_err(|e| e.to_string())?)
                .ok_or("Source badge missing")?
                .0 = if entry.kind == Kind::Folder {
                String::new()
            } else {
                kind.chars().take(6).collect()
            };
            class(
                world,
                row.element("folder").map_err(|e| e.to_string())?,
                "hidden",
                entry.kind != Kind::Folder,
            );
            class(
                world,
                row.root,
                "selected",
                self.model.selected.as_ref() == Some(&entry.name),
            );
            view::disable(
                world,
                row.root,
                self.model.loading || self.model.error.is_some(),
            );
            if self.focus_selection && self.model.selected.as_ref() == Some(&entry.name) {
                world
                    .resource_mut::<InputFocus>()
                    .set(row.root, bevy::input_focus::FocusCause::Navigated);
                self.focus_selection = false;
            }
        }
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
