//! Source target resolution and composition, without renderer or filesystem authority.
use super::*;
use crate::documents::Documents;
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    editors::{EditorContext, RenderBudget},
};
use kedra_ui::{docking::class, fields::FieldAccess};
impl Panel {
    pub fn tick(
        &mut self,
        world: &mut World,
        id: ViewId,
        documents: &mut Documents,
    ) -> Result<(), String> {
        if !self.native_work_pending(world)
            && let Some((token, action)) = self.pending_presentation.take()
            && token == self.state.token
        {
            self.action(world, action)?;
        }
        let selected = documents.selection_target();
        let asset = match documents.selection.active() {
            Some(kedra_editor_context::SelectionItem::Asset(asset)) => Some(asset.clone()),
            _ => None,
        };
        self.blocked_follow = !self.state.locked
            && (self.state.target != selected || self.state.asset != asset)
            && self.has_drafts(world);
        if !self.blocked_follow && self.state.follow(selected, asset)? {
            self.clear(world);
            self.dirty = true;
        }
        // Install retained item mappings before source values/indexed fields refresh.
        // Otherwise one intermediate positional binding can overwrite native text.
        self.prepare_history(world, id);
        let filter = world
            .get::<bevy::text::EditableText>(self.element("search")?)
            .map(|t| t.value().to_string())
            .unwrap_or_default();
        if filter != self.filter && !self.has_drafts(world) {
            self.filter = filter;
            self.dirty = true;
        }
        let width = world
            .get::<ComputedNode>(self.root())
            .map_or(0, |n| (n.size().x * n.inverse_scale_factor()) as u32);
        if self.width != width {
            self.width = width;
            self.dirty = true;
        }
        class(
            world,
            self.element("toolbar")?,
            "hidden",
            !self.state.search_open,
        );
        class(world, self.element("locked")?, "hidden", !self.state.locked);
        class(
            world,
            self.element("identity")?,
            "hidden",
            self.state.target.is_none(),
        );
        class(
            world,
            self.element("source")?,
            "hidden",
            !self.state.details_open || self.state.target.is_none(),
        );
        let Some(target) = &self.state.target else {
            if let Some(asset) = &self.state.asset {
                let path = asset
                    .path()
                    .split_once("://")
                    .map_or(asset.path(), |(_, path)| path)
                    .to_owned();
                let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
                self.text(world, "name", &name)?;
                self.text(world, "source", &path)?;
                class(world, self.element("identity")?, "hidden", false);
                class(
                    world,
                    self.element("source")?,
                    "hidden",
                    !self.state.details_open,
                );
                class(world, self.element("message")?, "hidden", true);
                class(world, self.element("entries")?, "hidden", true);
                return Ok(());
            }
            self.text(
                world,
                "message",
                "Select a node in Hierarchy or Scene View.",
            )?;
            class(world, self.element("message")?, "hidden", false);
            class(world, self.element("entries")?, "hidden", true);
            return Ok(());
        };
        let registry = world.resource::<AppTypeRegistry>().clone();
        let types = registry.read();
        let policies = world
            .resource::<kedra_scene::reflection::ComponentPolicies>()
            .clone();
        let snapshot = match documents
            .refresh_inspection(&types, &policies)
            .and_then(|()| documents.inspect(target))
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.text(world, "message", &format!("Target unavailable: {error}"))?;
                class(world, self.element("message")?, "hidden", false);
                class(world, self.element("entries")?, "hidden", true);
                return Ok(());
            }
        };
        if self.source_revisions != *snapshot.revisions {
            self.edit_epoch = self
                .edit_epoch
                .checked_add(1)
                .ok_or("Inspector revision token exhausted")?;
            self.source_revisions.clone_from(snapshot.revisions);
            self.dirty = true;
        }
        self.text(world, "name", snapshot.name)?;
        self.text(world, "source", snapshot.source_path)?;
        class(world, self.element("entries")?, "hidden", false);
        let delivery = world
            .get_resource::<kedra_authoring_assets::Delivery>()
            .filter(|delivery| {
                world
                    .get_resource::<crate::model::ProjectManager>()
                    .and_then(|manager| manager.project.as_ref())
                    .is_some_and(|project| project.assets == delivery.source)
            })
            .cloned();
        let model_paths = if snapshot.components.contains_key("scene.model") {
            use kedra_inspector::binding::ValueMetadata;
            super::metadata::Metadata::new(snapshot.metadata("scene.model")?)
                .with_delivery(delivery.as_ref())
                .asset_choices(std::any::TypeId::of::<bevy::gltf::Gltf>())
        } else {
            Vec::new()
        };
        let (model_choices, changed) =
            kedra_preview::catalog::choices(world, self.root(), &model_paths);
        self.dirty |= changed;
        if !self.dirty {
            self.sync_fields(world, id, documents)?;
            self.status(world, snapshot.components.is_empty())?;
            self.history_tick(world, id);
            return Ok(());
        }
        let read_only = FieldAccess::ReadOnly {
            reason: "Source editing is not enabled".into(),
        };
        let area = self.element("entries")?;
        let mut budget = RenderBudget::default();
        self.limited = false;
        self.components.retain(|key, tree| {
            if snapshot.components.contains_key(key) {
                true
            } else {
                tree.retire(world);
                false
            }
        });
        for key in snapshot.components.keys() {
            if budget.exhausted() {
                self.limited = true;
                break;
            }
            let scope = format!("inspector/{id:?}/{}/{key}", self.state.token);
            let mut context = EditorContext {
                templates: &self.templates,
                types: &types,
                editors: &self.editors,
                budget: &mut budget,
                scope: &scope,
            };
            let failure;
            let metadata = super::metadata::Metadata::new(snapshot.metadata(key)?)
                .with_delivery(delivery.as_ref())
                .with_model_choices(&model_choices);
            let result = snapshot.value(key, &types, &policies);
            let (value, diagnostic): (&dyn bevy::reflect::PartialReflect, _) = match result {
                Ok(value) => (value, None),
                Err(error) => {
                    failure = error;
                    (&failure, Some(failure.as_str()))
                }
            };
            let binding = ValueBinding {
                token: BindingToken(self.state.token),
                value,
                label: key,
                access: if !self.state.raw
                    && ((key == "scene.transform"
                        && value.try_downcast_ref::<Transform>().is_some())
                        || (key == "ui.caption"
                            && value
                                .try_downcast_ref::<kedra_ui::authored::components::UiCaption>()
                                .is_some())
                        || (key == "scene.primitive"
                            && value
                                .try_downcast_ref::<kedra_scene::spatial::Primitive>()
                                .is_some())
                        || (key == "scene.surface"
                            && value
                                .try_downcast_ref::<kedra_scene::spatial::Surface>()
                                .is_some())
                        || (matches!(
                            key.as_str(),
                            "ui.layout"
                                | "ui.classes"
                                | "ui.image"
                                | "ui.stylesheet"
                                | "ui.background"
                                | "ui.border"
                                | "ui.shadow"
                                | "ui.z"
                                | "ui.global_z"
                                | "ui.outline"
                                | "ui.spin"
                                | "ui.editable"
                                | "ui.pickable"
                                | "ui.modal"
                                | "ui.id"
                                | "ui.screen"
                        ) && diagnostic.is_none())
                        || (key == "scene.sun"
                            && value
                                .try_downcast_ref::<kedra_scene::spatial::Sun>()
                                .is_some())
                        || (key == "scene.environment"
                            && value
                                .try_downcast_ref::<kedra_scene::environment::SceneEnvironment>()
                                .is_some())
                        || (key == "scene.model"
                            && value
                                .try_downcast_ref::<kedra_scene::spatial::Model>()
                                .is_some()))
                {
                    &FieldAccess::Editable
                } else {
                    &read_only
                },
                owner_type: value.get_represented_type_info().map(|t| t.type_id()),
                path: None,
                diagnostic,
                metadata: Some(&metadata),
            };
            if !self.components.contains_key(key) {
                let tree = TreeView::mount_filtered(
                    world,
                    area,
                    &mut context,
                    binding,
                    self.state.raw,
                    &self.filter,
                )?;
                self.components.insert(key.clone(), tree);
            } else if let Some(tree) = self.components.get_mut(key) {
                tree.filter(&self.filter);
                tree.update(world, &mut context, binding, self.state.raw)?;
            }
        }
        self.dirty = false;
        if self.history_waiting() {
            // A newly mounted target did not have collection owners at tick start.
            // Queue its mappings, but do not acknowledge temporary indexed fields.
            self.history_tick(world, id);
            self.dirty = true;
            return Ok(());
        }
        self.sync_fields(world, id, documents)?;
        self.status(world, snapshot.components.is_empty())?;
        self.history_tick(world, id);
        Ok(())
    }
    fn status(&self, world: &mut World, empty: bool) -> Result<(), String> {
        let message = if self.blocked_follow {
            Some(
                "Inspector retains the previous target. Accept the property draft or press Escape before following selection.",
            )
        } else if self.limited {
            Some("Inspector presentation limit reached; remaining components are not displayed.")
        } else if empty {
            Some("This source node has no authored components.")
        } else if !self.filter.is_empty()
            && self.components.values().any(TreeView::search_incomplete)
        {
            Some("Search incomplete: inspection limits omit some source values.")
        } else if !self.filter.is_empty() && !self.components.values().any(TreeView::has_matches) {
            Some("No properties match this search.")
        } else {
            None
        };
        class(world, self.element("message")?, "hidden", message.is_none());
        if let Some(message) = message {
            self.text(world, "message", message)?;
        }
        Ok(())
    }
}
