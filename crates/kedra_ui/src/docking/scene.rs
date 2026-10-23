//! Authored initial topology using the existing bounded docking snapshot schema.
use bevy::prelude::*;
use kedra_scene::reflection::ComponentPolicies;

/// Inert initial layout. Native hosts explicitly choose controllers and content
/// hydration. Loading this component never restores windows or executes scripts.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct InitialLayout {
    /// Versioned JSON accepted by `Snapshot::decode`, not a second layout format.
    /// View restore keys are host-owned content locators, not executable code.
    pub snapshot: String,
}
impl InitialLayout {
    /// Decode the existing bounded snapshot into a new, independent workspace
    /// lineage on every call. This borrows the source without changing it and
    /// does not publish to any host or allocate native resources.
    ///
    /// Returns the snapshot decoder's errors for oversized input, malformed JSON,
    /// duplicate/unknown fields, unsupported versions or invalid topology/ownership.
    pub fn workspace(&self) -> kedra_docking::Result<kedra_docking::model::Workspace> {
        kedra_docking::snapshot::Snapshot::decode(self.snapshot.as_bytes())
    }
}

/// Register inert layout data before the host installs its PrefabPlugin.
/// Invalid snapshots fail source compilation, without mutating live topology.
/// Requires AuthoredUiPlugin and AppTypeRegistry. Duplicate registration returns
/// an error; this never changes live layout, native resources or script authority.
///
/// # Panics
/// Panics if the host has not installed `AppTypeRegistry` and `ComponentPolicies`.
pub fn register(app: &mut App) -> kedra_scene::Result<()> {
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    app.world_mut()
        .resource_mut::<ComponentPolicies>()
        .register::<InitialLayout>(&mut registry.write(), "ui.docking", 1, |value| {
            let layout = InitialLayout::from_reflect(value)
                .ok_or_else(|| kedra_scene::Error("Invalid initial docking layout".into()))?;
            layout
                .workspace()
                .map(|_| ())
                .map_err(|e| kedra_scene::Error(e.to_string()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct SourceLayout {
        snapshot: String,
    }

    #[test]
    fn authored_topologies_preserve_initial_geometry_and_active_tabs()
    -> Result<(), Box<dyn std::error::Error>> {
        use kedra_docking::model::{Axis, Node};
        for (source, horizontal, vertical) in [
            (
                include_str!("../../../../assets/editor/fixtures/docking.scene.ron"),
                1. - 894. / 1194.,
                1. - 296. / 762.,
            ),
            (
                include_str!("../../../../assets/editor/fixtures/projection.scene.ron"),
                0.25,
                0.6,
            ),
        ] {
            let document: kedra_scene::document::PrefabDocument = ron::from_str(source)?;
            let source: SourceLayout =
                ron::from_str(document.nodes[0].components["ui.docking"].value.get_ron())?;
            let initial = InitialLayout {
                snapshot: source.snapshot,
            };
            let workspace = initial.workspace()?;
            let snapshot = workspace.snapshot().clone();
            assert_eq!(snapshot.hosts.len(), 1);
            assert_eq!(snapshot.views.len(), 4);
            let mut splits = 0;
            let mut paired_groups = 0;
            for node in snapshot.nodes.values() {
                match node {
                    Node::Split { axis, fraction, .. } => {
                        splits += 1;
                        let expected = if *axis == Axis::Horizontal {
                            horizontal
                        } else {
                            vertical
                        };
                        assert!((fraction - expected).abs() < 0.000001);
                    }
                    Node::Group { tabs, active } if tabs.len() == 2 => {
                        paired_groups += 1;
                        assert_eq!(snapshot.views[active].kind, "C");
                        assert_eq!(snapshot.views[&tabs[1]].kind, "D");
                    }
                    _ => (),
                }
            }
            assert_eq!(splits, 2);
            assert_eq!(paired_groups, 1);
            let view = *snapshot.views.keys().next().ok_or("fixture view missing")?;
            let candidate = workspace.prepare(kedra_docking::command::Command::Activate(view))?;
            let mut independent = initial.workspace()?;
            assert_eq!(
                independent.commit(candidate),
                Err(kedra_docking::Error::Stale)
            );
            assert_eq!(independent.snapshot(), &snapshot);
            let invalid = InitialLayout {
                snapshot: initial
                    .snapshot
                    .replace("\"version\":1", "\"version\":1,\"version\":1"),
            };
            assert!(invalid.workspace().is_err());
            assert_eq!(workspace.snapshot(), &snapshot);
        }
        Ok(())
    }
    #[test]
    fn invalid_initial_layout_cannot_publish() {
        for snapshot in ["{}", "{\"version\":999}", "not json"] {
            assert!(
                InitialLayout {
                    snapshot: snapshot.into()
                }
                .workspace()
                .is_err()
            );
        }
    }

    #[test]
    fn bounded_source_policy_rejects_invalid_layout_before_live_allocation()
    -> Result<(), Box<dyn std::error::Error>> {
        use bevy::asset::AssetApp;
        use kedra_docking::{
            model::HostId,
            snapshot::{MAX_BYTES, Snapshot},
        };
        use kedra_scene::{
            composition::Catalog,
            document::{ComponentData, PrefabDocument},
            instance::CompiledPrefab,
        };
        use std::collections::BTreeMap;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .init_resource::<AppTypeRegistry>()
            .add_plugins(crate::authored::AuthoredUiPlugin);
        register(&mut app)?;
        assert!(
            register(&mut app).is_err(),
            "Duplicate registration must fail"
        );
        let workspace = kedra_docking::model::Workspace::new(HostId::new());
        let valid = String::from_utf8(workspace.snapshot().encode()?)?;
        let oversized = InitialLayout {
            snapshot: " ".repeat(MAX_BYTES + 1),
        };
        assert!(oversized.workspace().is_err());
        // The scene component's own smaller byte bound also rejects this before
        // it can reach the snapshot policy during source compilation.
        assert!(ComponentData::ron(1, &format!("(snapshot:{:?})", oversized.snapshot)).is_err());
        let before = app.world().entities().count_spawned();
        for (snapshot, accepted) in [
            (valid.clone(), true),
            (valid.replace("\"version\":1", "\"version\":999"), false),
            (
                valid.replace("\"version\":1", "\"version\":1,\"version\":1"),
                false,
            ),
            (
                valid.replace("\"nodes\":{}", "\"nodes\":{},\"unexpected\":0"),
                false,
            ),
            (
                valid.replace(
                    "\"root\":null",
                    "\"root\":\"00000000-0000-4000-8000-000000000001\"",
                ),
                false,
            ),
        ] {
            assert_eq!(Snapshot::decode(snapshot.as_bytes()).is_ok(), accepted);
            let layout = InitialLayout { snapshot };
            assert_eq!(layout.workspace().is_ok(), accepted);
            // Raw RON bypasses encode-time validation to exercise compilation's
            // registered policy against untrusted source, not only the helper.
            let mut source = PrefabDocument::new("Initial docking policy");
            source.nodes[0].components.insert(
                "ui.docking".into(),
                ComponentData::ron(1, &format!("(snapshot:{:?})", layout.snapshot))?,
            );
            let id = source.id;
            let mut catalog = Catalog::default();
            catalog.insert(source)?;
            let registry = app.world().resource::<AppTypeRegistry>();
            let mut loader = app.world().resource::<AssetServer>();
            let result = CompiledPrefab::compile(
                catalog.resolve(id)?,
                &registry.read(),
                app.world().resource::<ComponentPolicies>(),
                &mut loader,
                &BTreeMap::new(),
            );
            assert_eq!(result.is_ok(), accepted);
            assert_eq!(app.world().entities().count_spawned(), before);
            assert_eq!(workspace.snapshot().encode()?, valid.as_bytes());
        }
        Ok(())
    }
}
