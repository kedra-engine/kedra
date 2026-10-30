//! Asset-root authorization uses the existing scoped source catalog.
use super::*;
use kedra_editor_context::{AssetRef, SelectionItem};
#[cfg(test)]
mod tests;
impl Documents {
    pub(crate) fn environment_choices(&self) -> Vec<String> {
        self.scope
            .as_ref()
            .map(|scope| {
                scope
                    .source
                    .environments
                    .keys()
                    .filter_map(|path| {
                        let logical = format!("{}://{path}", scope.source.namespace);
                        let parsed = bevy::asset::AssetPath::try_parse(&logical).ok()?;
                        kedra_authoring_assets::environments::accepts(&scope.source, &parsed)
                            .then_some(logical)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(crate) fn scene_root(&self, target: &inspection::Target) -> bool {
        self.compiled.get(&target.document).is_some_and(|compiled| {
            let graph = compiled.graph();
            graph.root == target.node
                && self
                    .scope
                    .as_ref()
                    .and_then(|scope| scope.working.documents().get(&target.document))
                    .is_some_and(|document| {
                        document.document.kind == kedra_scene::document::DocumentKind::Scene
                    })
        })
    }
    pub(crate) fn assign_environment(
        &mut self,
        target: &inspection::Target,
        expected: &BTreeMap<DocumentId, u64>,
        path: &str,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if self.revisions() != *expected
            || !self.scene_root(target)
            || self.creating()
            || self.opening()
            || self.save_pending()
            || self.save_busy()
        {
            return Err("Scene environment target is stale or unavailable".into());
        }
        if self
            .inspect(target)?
            .components
            .contains_key("scene.environment")
        {
            return Err("Scene environment already exists; use its asset field".into());
        }
        if !self
            .environment_choices()
            .iter()
            .any(|choice| choice == path)
        {
            return Err("Choose an environment from the admitted project source catalog".into());
        }
        let scope = self.scope.as_ref().ok_or("Source workspace unavailable")?;
        let logical = format!(
            "{}://{}",
            scope.source.namespace,
            scope
                .path(target.document)
                .ok_or("Scene path unavailable")?
        );
        let mut edit =
            SourceEdit::begin(scope, "Assign environment", std::slice::from_ref(&logical))?;
        let node = edit
            .nodes(scope, &logical)?
            .into_iter()
            .find(|node| node.address == target.node)
            .ok_or("Scene root unavailable")?;
        let text = format!(
            "(asset:Path({}))",
            ron::to_string(path).map_err(|e| e.to_string())?
        );
        let data =
            kedra_scene::document::ComponentData::ron(1, &text).map_err(|e| e.to_string())?;
        edit.add_component(scope, &node, "scene.environment", data, types, policies)?;
        self.commit_edit(&mut edit, types, policies)?;
        Ok(())
    }
    pub(crate) fn refresh_environments(&mut self, world: &mut World) -> Result<(), String> {
        if let Some(sources) = world.get_resource::<kedra_authoring_assets::environments::Sources>()
        {
            sources.publish(self.scope.as_ref().map(|scope| &scope.source))?;
        }
        let Some(scope) = &self.scope else {
            return Ok(());
        };
        for path in scope.source.environments.keys() {
            let logical = format!("{}://{path}", scope.source.namespace);
            if world
                .resource::<AssetServer>()
                .get_handle::<kedra_scene::environment::Environment>(logical)
                .is_none()
            {
                continue;
            }
            self.environments.open(&scope.source, path)?;
            let (value, _) = self
                .environments
                .value(path)
                .ok_or("Environment working value unavailable")?;
            kedra_authoring_assets::refresh_environment(world, &scope.source, path, value)?;
        }
        Ok(())
    }
    pub(crate) fn environment_path(&self, asset: &AssetRef) -> Result<String, String> {
        if !self
            .library()
            .is_some_and(|library| library.contains_asset(asset))
        {
            return Err("Environment source scope was retired".into());
        }
        let path = asset
            .path()
            .split_once("://")
            .ok_or("Source path is not qualified")?
            .1;
        if !path.ends_with(".environment.ron") {
            return Err("Asset has no editable environment root".into());
        }
        Ok(path.into())
    }
    pub(crate) fn selected_environment(&self) -> Option<String> {
        let SelectionItem::Asset(asset) = self.selection.active()? else {
            return None;
        };
        self.environment_path(asset).ok()
    }
    pub(crate) fn inspect_environment(
        &mut self,
        asset: &AssetRef,
    ) -> Result<(kedra_scene::environment::Environment, u64), String> {
        let path = self.environment_path(asset)?;
        self.environments.open(
            &self
                .scope
                .as_ref()
                .ok_or("Source workspace unavailable")?
                .source,
            &path,
        )?;
        self.environments
            .value(&path)
            .map(|(value, revision)| (value.clone(), revision))
            .ok_or("Environment root unavailable".into())
    }
    pub(crate) fn edit_environment(
        &mut self,
        asset: &AssetRef,
        revision: u64,
        field: &bevy::reflect::ParsedPath,
        value: &dyn PartialReflect,
    ) -> Result<(), String> {
        if self.creating() || self.opening() || self.save_busy() || self.save_pending() {
            return Err("Wait for the current source operation".into());
        }
        let path = self.environment_path(asset)?;
        self.environments.edit(&path, revision, field, value)
    }
}
