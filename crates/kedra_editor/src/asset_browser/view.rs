//! Authored panel projection. Browser/domain authority never lives in these rows.
use super::*;
use bevy::text::EditableText;
use kedra_ui::{ElementId, docking::class};
impl Panel {
    pub(super) fn render(&mut self, world: &mut World, id: ViewId) -> Result<(), String> {
        let location = if self.model.path.as_os_str().is_empty() {
            "Assets".into()
        } else {
            format!("Assets / {}", self.model.path.display())
        };
        self.text(world, "location", location)?;
        let message = if self.model.loading {
            "Reading source directory…".into()
        } else if let Some(error) = &self.model.error {
            format!("{error}\nPrevious listing is stale. Refresh or navigate to recover.")
        } else if let Some(notice) = &self.model.notice {
            notice.clone()
        } else if self.model.entries.is_empty() {
            "This source folder is empty.".into()
        } else if self.model.visible().next().is_none() {
            "No sources match this folder filter. Clear the search or type filter to see all entries.".into()
        } else {
            String::new()
        };
        self.text(world, "message", message.clone())?;
        class(
            world,
            self.element("message")?,
            "hidden",
            message.is_empty(),
        );
        let visible = self.model.visible().count();
        let selection = self
            .model
            .selected_entry()
            .map(|entry| {
                let hidden = !self.model.visible().any(|e| e.name == entry.name);
                format!(
                    "{}{} · {}{}",
                    crate::display_paths::asset(&self.model.path.join(&entry.name)),
                    if hidden { " (filtered selection)" } else { "" },
                    entry.kind.label(),
                    if entry.kind == Kind::Folder || entry.tracked {
                        ""
                    } else {
                        " · untracked (no sidecar)"
                    }
                )
            })
            .unwrap_or_else(|| {
                "Read-only sources · New/Import unavailable: source writing is not connected".into()
            });
        self.text(world, "status", selection)?;
        self.text(
            world,
            "count",
            format!(
                "{visible}/{} items{}",
                self.model.entries.len(),
                if self.model.truncated {
                    " · incomplete: first 512 entries"
                } else {
                    ""
                }
            ),
        )?;
        for (key, disabled) in [
            (
                "back",
                self.model.loading || self.model.history(false).is_none(),
            ),
            (
                "forward",
                self.model.loading || self.model.history(true).is_none(),
            ),
            (
                "up",
                self.model.loading || self.model.path.as_os_str().is_empty(),
            ),
            ("new", true),
            ("import", true),
        ] {
            disable(world, self.element(key)?, disabled);
        }
        class(
            world,
            self.element("grid")?,
            "selected",
            self.model.mode == model::Mode::Grid,
        );
        class(
            world,
            self.element("list")?,
            "selected",
            self.model.mode == model::Mode::List,
        );
        class(
            world,
            self.element("filter")?,
            "selected",
            self.model.documents_only,
        );
        class(
            world,
            self.element("search-placeholder")?,
            "hidden",
            !self.model.filter.is_empty(),
        );
        class(
            world,
            self.element("clear")?,
            "hidden",
            self.model.filter.is_empty(),
        );
        let search = self.element("search")?;
        if let Some(mut text) = world.get_mut::<EditableText>(search)
            && text.value() != self.model.filter.as_str()
        {
            text.editor_mut().set_text(&self.model.filter);
            text.queue_edit(bevy::text::TextEdit::TextEnd(false));
        }
        let bound: Vec<_> = world
            .query::<(Entity, &Binding)>()
            .iter(world)
            .filter_map(|(e, b)| (b.view == id).then_some(e))
            .collect();
        for entity in bound {
            if let Some(mut binding) = world.get_mut::<Binding>(entity) {
                binding.path = self.model.path.clone();
                binding.generation = self.model.generation();
                let text = self.hint(&binding.action);
                world
                    .entity_mut(entity)
                    .insert(kedra_ui::tooltip::Tooltip(text));
            }
        }
        self.render_tiles(world, id)?;
        self.render_tree(world, id)?;
        self.render_crumbs(world, id)?;
        Ok(())
    }
    fn render_crumbs(&mut self, world: &mut World, id: ViewId) -> Result<(), String> {
        if self.crumb_path.as_ref() == Some(&self.model.path) {
            return Ok(());
        }
        self.crumb_path = Some(self.model.path.clone());
        for crumb in self.crumbs.drain(..) {
            world.despawn(crumb.root);
        }
        let parent = self.element("breadcrumbs")?;
        let mut paths: Vec<_> = self
            .model
            .path
            .ancestors()
            .map(std::path::Path::to_path_buf)
            .collect();
        paths.reverse();
        for (index, path) in paths.into_iter().enumerate() {
            let row = crate::views::mount_asset(
                world,
                parent,
                "editor://prefabs/browser-crumb.prefab.ron",
                &format!("browser/{id:?}/crumb/{index}"),
            )?;
            world
                .get_mut::<Text>(row.element("name").map_err(|e| e.to_string())?)
                .ok_or("Breadcrumb label missing")?
                .0 = path
                .file_name()
                .map_or_else(|| "Assets".into(), |n| n.to_string_lossy().into_owned());
            class(world, row.root, "browser-crumb", true);
            self.bind(world, row.root, id, Action::Navigate(path));
            world
                .entity_mut(row.root)
                .insert(ElementId::new(format!("browser/{id:?}/breadcrumb/{index}")));
            self.crumbs.push(row);
        }
        Ok(())
    }
    fn render_tree(&mut self, world: &mut World, id: ViewId) -> Result<(), String> {
        let desired = self.tree.rows();
        self.folders.retain(|(path, row)| {
            if desired.iter().any(|(p, _)| p == path) {
                true
            } else {
                world.despawn(row.root);
                false
            }
        });
        let parent = self.element("folders")?;
        let mut order = Vec::new();
        for (index, (path, depth)) in desired.iter().enumerate() {
            if !self.folders.iter().any(|(p, _)| p == path) {
                let row = crate::views::mount_asset(
                    world,
                    parent,
                    "editor://prefabs/browser-folder.prefab.ron",
                    &format!("browser/{id:?}/folder/{}", uuid::Uuid::new_v4()),
                )?;
                self.folders.push((path.clone(), row));
            }
            let row = &self
                .folders
                .iter()
                .find(|(p, _)| p == path)
                .ok_or("Folder row missing")?
                .1;
            let toggle = row.element("toggle").map_err(|e| e.to_string())?;
            let navigate = row.element("navigate").map_err(|e| e.to_string())?;
            self.bind(world, toggle, id, Action::Toggle(path.clone()));
            self.bind(world, navigate, id, Action::TreeNavigate(path.clone()));
            world
                .entity_mut(navigate)
                .insert(ElementId::new(format!("browser/{id:?}/folder/{index}")));
            let caption = path
                .file_name()
                .map_or_else(|| "Assets".into(), |n| n.to_string_lossy().into_owned());
            world
                .get_mut::<Text>(row.element("name").map_err(|e| e.to_string())?)
                .ok_or("Folder caption missing")?
                .0 = format!(
                "{caption}{}",
                if self.tree.error(path).is_some() {
                    " ⚠"
                } else {
                    ""
                }
            );
            let mut style = world
                .get::<bevy_flair::prelude::InlineStyle>(row.root)
                .cloned()
                .unwrap_or_default();
            style.set("padding-left", format!("{}px", 6 + depth.min(&12) * 12));
            world.entity_mut(row.root).insert(style);
            class(world, row.root, "selected", path == &self.model.path);
            class(
                world,
                row.element("folder").map_err(|e| e.to_string())?,
                "hidden",
                path == &self.model.path,
            );
            class(
                world,
                row.element("selected-folder").map_err(|e| e.to_string())?,
                "hidden",
                path != &self.model.path,
            );
            class(
                world,
                row.element("right").map_err(|e| e.to_string())?,
                "hidden",
                self.tree.expanded(path),
            );
            class(
                world,
                row.element("down").map_err(|e| e.to_string())?,
                "hidden",
                !self.tree.expanded(path),
            );
            order.push(row.root);
        }
        world.entity_mut(parent).replace_children(&order);
        let tree_error = self
            .tree
            .rows()
            .iter()
            .find_map(|(path, _)| self.tree.error(path));
        self.text(
            world,
            "tree-status",
            tree_error
                .unwrap_or(if desired.len() == 512 {
                    "Tree limited to 512 rows"
                } else {
                    ""
                })
                .into(),
        )?;
        class(
            world,
            self.element("tree-status")?,
            "hidden",
            tree_error.is_none() && desired.len() < 512,
        );
        Ok(())
    }
    pub(super) fn bind(&self, world: &mut World, entity: Entity, id: ViewId, action: Action) {
        let hint = kedra_ui::tooltip::Tooltip(self.hint(&action));
        world.entity_mut(entity).insert((
            Binding {
                view: id,
                path: self.model.path.clone(),
                generation: self.model.generation(),
                action,
            },
            hint,
        ));
    }
    pub(super) fn element(&self, key: &str) -> Result<Entity, String> {
        self.view.element(key).map_err(|e| e.to_string())
    }
    pub(super) fn text(&self, world: &mut World, key: &str, value: String) -> Result<(), String> {
        world
            .get_mut::<Text>(self.element(key)?)
            .ok_or_else(|| format!("Browser binding {key} is not text"))?
            .0 = value;
        Ok(())
    }
}
pub(super) fn disable(world: &mut World, entity: Entity, disabled: bool) {
    if disabled {
        world
            .entity_mut(entity)
            .insert(bevy::ui::InteractionDisabled);
    } else {
        world
            .entity_mut(entity)
            .remove::<bevy::ui::InteractionDisabled>();
    }
}
