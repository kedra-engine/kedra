//! Source previews may be resource-free; referenced resources still require delivery admission.
use super::*;
use kedra_scene::{
    composition::ResolvedGraph, instance::CompiledPrefab, reflection::ComponentPolicies,
};
use std::collections::BTreeMap;

/// Compile a passive source graph. Without a delivery, only resource-free graphs
/// and validated source environments are admitted. Environment consumers install
/// `environments::install` and publish their source snapshot before loading. With a delivery,
/// source scope/resource matching and the limited native loader apply. The graph
/// comes from authoring, so scene/prefab bytes need not match a runtime delivery.
/// This does not build assets, execute code, hydrate entities or establish frame readiness.
/// Hosts overlay admitted environment roots using `refresh_environment` before
/// materialization and after subsequent native load completion.
pub fn compile_preview(
    graph: ResolvedGraph,
    types: &bevy::reflect::TypeRegistry,
    policies: &ComponentPolicies,
    server: &AssetServer,
    source: &SourceSnapshot,
    root: &Path,
    delivery: Option<&Delivery>,
) -> Result<CompiledPrefab, String> {
    if let Some(delivery) = delivery {
        delivery.matches_resources(root, source)?;
        let mut loader = Loader {
            server: server.clone(),
            source,
            delivery,
            error: None,
        };
        let compiled =
            CompiledPrefab::compile(graph, types, policies, &mut loader, &BTreeMap::new())
                .map_err(|error| error.to_string())?;
        if let Some(error) = loader.error {
            return Err(error);
        }
        return Ok(compiled);
    }
    struct ResourceFree<'a> {
        requested: Option<String>,
        source: &'a SourceSnapshot,
        server: &'a AssetServer,
    }
    impl LoadFromPath for ResourceFree<'_> {
        fn load_from_path_erased(
            &mut self,
            type_id: TypeId,
            path: AssetPath<'static>,
        ) -> UntypedHandle {
            if type_id == TypeId::of::<kedra_scene::environment::Environment>()
                && environments::accepts(self.source, &path)
            {
                return self
                    .server
                    .load::<kedra_scene::environment::Environment>(path)
                    .untyped();
            }
            self.requested = Some(path.to_string());
            UntypedHandle::Uuid {
                type_id,
                uuid: uuid::Uuid::nil(),
            }
        }
    }
    let mut loader = ResourceFree {
        requested: None,
        source,
        server,
    };
    let compiled = CompiledPrefab::compile(graph, types, policies, &mut loader, &BTreeMap::new())
        .map_err(|error| error.to_string())?;
    if let Some(path) = loader.requested {
        return Err(format!(
            "A matching project asset delivery is required for {path}."
        ));
    }
    Ok(compiled)
}
