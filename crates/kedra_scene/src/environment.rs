//! Versioned environment data. Loading executes no rendering or gameplay systems.
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use serde::{Deserialize, Serialize};

/// Reusable environment. Version 1 supports solid background and ambient light.
/// Skyboxes/IBL are deliberately not accepted until native dependency preparation
/// and view isolation are verified. No exposure, lights or entity graph lives here.
#[derive(Asset, Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    /// Exact source schema version, currently 1.
    pub version: u32,
    /// Linear/sRGB Bevy color; finite normalized channels, including alpha.
    pub background: Color,
    /// Native ambient color, finite normalized channels.
    pub ambient_color: Color,
    /// Native Bevy ambient brightness, finite and in 0..=1000000.
    pub ambient_brightness: f32,
}
impl Environment {
    /// Decode at most 64 KiB. Reject unknown/versioned fields and invalid values
    /// before publication; a broken reference must not become an unlit fallback.
    pub fn parse(text: &str) -> crate::Result<Self> {
        if text.len() > 65_536 {
            return Err(crate::failure("environment exceeds 64 KiB"));
        }
        let value: Self = ron::from_str(text)?;
        value.validate()?;
        Ok(value)
    }
    /// Validate programmatically created values with the same source contract.
    pub fn validate(&self) -> crate::Result<()> {
        let color = |v: Color| {
            v.to_linear()
                .to_f32_array()
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
        };
        if self.version != 1
            || !color(self.background)
            || !color(self.ambient_color)
            || !self.ambient_brightness.is_finite()
            || !(0.0..=1_000_000.0).contains(&self.ambient_brightness)
        {
            return Err(crate::failure(
                "environment requires version 1, finite normalized colors and ambient brightness in 0..=1000000",
            ));
        }
        Ok(())
    }
}

/// Inert scene-root environment reference. Hosts opt into materialization;
/// inserting it does not modify cameras or global ambient resources.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct SceneEnvironment {
    /// Retained native handle. Its asset readiness is distinct from frame readiness.
    pub asset: Handle<Environment>,
}

pub(crate) fn validate_assignment(graph: &crate::composition::ResolvedGraph) -> crate::Result<()> {
    for (address, node) in &graph.nodes {
        if node.components.contains_key("scene.environment")
            && (*address != graph.root
                || graph.sources[&graph.document].kind != crate::document::DocumentKind::Scene)
        {
            return Err(crate::failure(
                "scene.environment is only valid on the scene root; prefabs cannot replace the host environment",
            ));
        }
    }
    Ok(())
}

/// Bounded native RON loader used by runtime, importer and editor alike.
#[derive(Default, TypePath)]
pub struct EnvironmentLoader;
impl AssetLoader for EnvironmentLoader {
    type Asset = Environment;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> std::io::Result<Environment> {
        let mut bytes = Vec::new();
        futures_lite::AsyncReadExt::read_to_end(
            &mut futures_lite::AsyncReadExt::take(reader, 65_537),
            &mut bytes,
        )
        .await?;
        let text = std::str::from_utf8(&bytes).map_err(std::io::Error::other)?;
        Environment::parse(text).map_err(std::io::Error::other)
    }
    fn extensions(&self) -> &[&str] {
        &["environment.ron"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "(version:1,background:Srgba((red:0.1,green:0.2,blue:0.3,alpha:1.0)),ambient_color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),ambient_brightness:100.0)";
    #[test]
    fn prefab_and_nonroot_environment_assignments_are_rejected() -> crate::Result<()> {
        use crate::{
            composition::Catalog,
            document::{ComponentData, DocumentKind, PrefabDocument},
        };
        let mut source = PrefabDocument::new("Environment assignment");
        source.nodes[0].components.insert(
            "scene.environment".into(),
            ComponentData::ron(1, "(asset:Path(\"game://studio.environment.ron\"))")?,
        );
        let id = source.id;
        let mut catalog = Catalog::default();
        catalog.insert(source.clone())?;
        assert!(validate_assignment(&catalog.resolve(id)?).is_err());
        source.kind = DocumentKind::Scene;
        let mut catalog = Catalog::default();
        catalog.insert(source.clone())?;
        assert!(validate_assignment(&catalog.resolve(id)?).is_ok());
        let mut child = source.nodes[0].clone();
        child.id = crate::document::NodeId::new();
        child.parent = Some(source.root.into());
        source.nodes.push(child);
        let mut catalog = Catalog::default();
        catalog.insert(source)?;
        assert!(validate_assignment(&catalog.resolve(id)?).is_err());
        Ok(())
    }
    #[test]
    fn exact_boundaries_versions_and_invalid_values_are_rejected() -> crate::Result<()> {
        let value = Environment::parse(SOURCE)?;
        assert_eq!(value.ambient_brightness, 100.);
        assert!(Environment::parse(&SOURCE.replace("version:1", "version:2")).is_err());
        assert!(Environment::parse(&SOURCE.replace("100.0", "-1.0")).is_err());
        assert!(Environment::parse(&SOURCE.replace("100.0", "NaN")).is_err());
        assert!(
            Environment::parse(&SOURCE.replace("version:1,", "version:1,skybox:None,")).is_err()
        );
        let padded = format!("{SOURCE}{}", " ".repeat(65_536 - SOURCE.len()));
        assert!(Environment::parse(&padded).is_ok());
        assert!(Environment::parse(&(padded + " ")).is_err());
        Ok(())
    }
}
