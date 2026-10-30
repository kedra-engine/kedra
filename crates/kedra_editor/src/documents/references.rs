//! Source/type/delivery admission before native loading. Publication still uses
//! the ordinary fully validated property transaction after readiness acknowledgment.
use super::*;
use kedra_authoring_assets::Delivery;
#[cfg(test)]
mod tests;
use bevy::{
    asset::AssetPath,
    reflect::{ParsedPath, PartialReflect, TypeRegistry},
};
pub(crate) struct ReferenceRequest<'a> {
    pub target: &'a inspection::Target,
    pub expected: &'a BTreeMap<DocumentId, u64>,
    pub component: &'a str,
    pub field: &'a ParsedPath,
    pub path: Option<&'a str>,
}
pub(crate) struct ResourceContext<'a> {
    pub root: &'a std::path::Path,
    pub delivery: Option<&'a Delivery>,
    pub server: &'a AssetServer,
}
impl Documents {
    pub(crate) fn reference_value(
        &self,
        request: ReferenceRequest<'_>,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        resources: ResourceContext<'_>,
    ) -> Result<Option<Box<dyn PartialReflect>>, String> {
        let snapshot = self.inspect(request.target)?;
        if self.revisions() != *request.expected {
            return Err("Resource source binding is stale".into());
        }
        let component = snapshot.value(request.component, types, policies)?;
        let value = request
            .field
            .reflect_element(component)
            .map_err(|e| e.to_string())?;
        let asset_type = if value
            .try_downcast_ref::<Vec<kedra_ui::authored::components::UiImageDensity>>()
            .is_some()
        {
            std::any::TypeId::of::<Image>()
        } else {
            kedra_assets::references::asset_type(value, types)?
        };
        let path = if let Some(text) = request.path {
            let path = AssetPath::try_parse(text)
                .map_err(|e| e.to_string())?
                .into_owned();
            let source = &self
                .scope
                .as_ref()
                .ok_or("Source workspace unavailable")?
                .source;
            if asset_type == std::any::TypeId::of::<kedra_scene::environment::Environment>()
                && kedra_authoring_assets::environments::accepts(source, &path)
            {
                let current = kedra_assets::references::referenced_handle(value, types)?;
                if current
                    .as_ref()
                    .is_some_and(|handle| snapshot.dependency_path(handle).as_deref() == Some(text))
                {
                    return Ok(None);
                }
                let mut server = resources.server.clone();
                return kedra_assets::references::proposal(value, Some(path), types, &mut server)
                    .map(Some);
            }
            let delivery = resources.delivery.ok_or(
                "A matching verified project delivery is required for resource assignment",
            )?;
            let source = &self
                .scope
                .as_ref()
                .ok_or("Source workspace unavailable")?
                .source;
            delivery.matches_resources(
                resources.root,
                self.scene_source()
                    .ok_or("Source admission baseline unavailable")?,
            )?;
            if !delivery.accepts(source, asset_type, &path) {
                return Err(
                    "Resource is incompatible or outside the matching admitted source/delivery"
                        .into(),
                );
            }
            Some(path)
        } else {
            None
        };
        if let Some(items) =
            value.try_downcast_ref::<Vec<kedra_ui::authored::components::UiImageDensity>>()
        {
            if !items.is_empty() {
                return Err("Choose an existing image field to replace its resource".into());
            }
            let path = path.ok_or("The first image requires a compatible resource")?;
            return Ok(Some(Box::new(vec![
                kedra_ui::authored::components::UiImageDensity {
                    density: 1.,
                    image: resources.server.load::<Image>(path),
                },
            ])));
        }
        let current = kedra_assets::references::referenced_handle(value, types)?;
        let unchanged = match (&path, current.as_ref()) {
            (Some(path), Some(handle)) => {
                snapshot.dependency_path(handle).as_deref() == Some(path.to_string().as_str())
            }
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return Ok(None);
        }
        if let Some(delivery) = resources.delivery {
            let mut loader = kedra_authoring_assets::Loader {
                server: resources.server.clone(),
                source: &self
                    .scope
                    .as_ref()
                    .ok_or("Source workspace unavailable")?
                    .source,
                delivery,
                error: None,
            };
            let value = kedra_assets::references::proposal(value, path, types, &mut loader)?;
            if let Some(error) = loader.error {
                return Err(error);
            }
            Ok(Some(value))
        } else {
            // Only optional Clear reaches this branch: paths require a delivery.
            let mut server = resources.server.clone();
            kedra_assets::references::proposal(value, path, types, &mut server).map(Some)
        }
    }
}
