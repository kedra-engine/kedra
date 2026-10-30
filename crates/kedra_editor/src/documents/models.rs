//! Model placement is one source transaction, guarded by the native delivery admission.
use super::*;
impl Documents {
    pub(crate) fn create_model(
        &mut self,
        target: &inspection::Target,
        expected: &BTreeMap<DocumentId, u64>,
        model: &Handle<bevy::world_serialization::WorldAsset>,
        position: Option<Vec3>,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if &self.revisions() != expected
            || self.opening()
            || self.creating()
            || self.save_busy()
            || self.save_pending()
        {
            return Err(
                "Model placement target changed or a document operation is pending.".into(),
            );
        }
        let inspection = self.inspect(target)?;
        if inspection.components.contains_key("ui.layout") {
            return Err("Choose a spatial parent for a 3D model.".into());
        }
        let model_path = model.path().ok_or("Model has no logical source path")?;
        let title = model_path
            .path()
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("Model name unavailable")?;
        let scope = self.scope.as_ref().ok_or("Source workspace unavailable")?;
        let path = format!("{}://{}", scope.source.namespace, inspection.source_path);
        let mut edit = SourceEdit::begin(scope, "Place model", std::slice::from_ref(&path))?;
        let parent = edit
            .nodes(scope, &path)?
            .into_iter()
            .find(|node| node.address == target.node)
            .ok_or("Model parent retired")?;
        let node = edit.add_node(scope, &parent, title, types, policies)?;
        let transform = position
            .map(|position| {
                super::structure::placement_transform(
                    self.compiled
                        .get(&target.document)
                        .ok_or("Source compilation missing")?,
                    &target.node,
                    position,
                )
            })
            .transpose()?
            .unwrap_or_default();
        for (key, value) in [
            (
                "scene.transform",
                Box::new(transform) as Box<dyn PartialReflect>,
            ),
            (
                "scene.model",
                Box::new(kedra_scene::spatial::Model {
                    scene: model.clone(),
                }) as Box<dyn PartialReflect>,
            ),
        ] {
            let data = policies
                .encode(key, value.as_ref(), types, &BTreeMap::new())
                .map_err(|error| error.to_string())?;
            edit.add_component(scope, &node, key, data, types, policies)?;
        }
        self.commit_edit(&mut edit, types, policies)?;
        self.select(target.document, node.address);
        Ok(())
    }
}
