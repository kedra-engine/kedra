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
        documents: &Documents,
    ) -> Result<(), String> {
        let selected = documents
            .active
            .and_then(|id| documents.opened.get(&id))
            .and_then(|d| {
                d.selected
                    .clone()
                    .and_then(|node| documents.target(d.document, node))
            });
        if self.state.follow(selected)? {
            self.clear(world);
            self.dirty = true;
        }
        let filter = world
            .get::<bevy::text::EditableText>(self.element("search")?)
            .map(|t| t.value().to_string())
            .unwrap_or_default();
        if filter != self.filter {
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
            self.text(
                world,
                "message",
                "Select a node in Hierarchy or Scene View.",
            )?;
            class(world, self.element("message")?, "hidden", false);
            class(world, self.element("entries")?, "hidden", true);
            return Ok(());
        };
        let snapshot = match documents.inspect(target) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.text(world, "message", &format!("Target unavailable: {error}"))?;
                class(world, self.element("message")?, "hidden", false);
                class(world, self.element("entries")?, "hidden", true);
                return Ok(());
            }
        };
        self.text(world, "name", snapshot.name)?;
        self.text(
            world,
            "source",
            &format!("{}\nRead-only source inspection", snapshot.source_path),
        )?;
        class(world, self.element("entries")?, "hidden", false);
        if !self.dirty {
            self.status(world, snapshot.components.is_empty())?;
            return Ok(());
        }
        let registry = world.resource::<AppTypeRegistry>().clone();
        let types = registry.read();
        let policies = world
            .resource::<kedra_scene::reflection::ComponentPolicies>()
            .clone();
        let access = FieldAccess::ReadOnly {
            reason: "Source editing is not enabled".into(),
        };
        let area = self.element("entries")?;
        let mut budget = RenderBudget::default();
        self.limited = false;
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
            let metadata = snapshot.metadata(key)?;
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
                access: &access,
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
        self.status(world, snapshot.components.is_empty())?;
        Ok(())
    }
    fn status(&self, world: &mut World, empty: bool) -> Result<(), String> {
        let message = if self.limited {
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
