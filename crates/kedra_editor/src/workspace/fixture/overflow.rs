//! One authored overflow specimen, applied through the existing docking command model.
use super::*;
use kedra_docking::model::View;

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub(crate) struct OverflowSource {
    view: String,
    target: String,
    index: usize,
    kind: String,
    restore_key: Option<String>,
    title: String,
    content_path: String,
    scope: String,
    identity: String,
}

#[derive(Clone)]
pub(super) struct Config {
    view: ViewId,
    target: ViewId,
    index: usize,
    descriptor: View,
    title: String,
    content_path: String,
    scope: String,
    identity: String,
}

pub(crate) fn validate_source(
    value: &dyn bevy::reflect::PartialReflect,
) -> kedra_scene::Result<()> {
    OverflowSource::from_reflect(value)
        .ok_or_else(|| kedra_scene::Error("Invalid docking overflow specimen".into()))?
        .config()
        .map(|_| ())
        .map_err(kedra_scene::Error)
}

impl OverflowSource {
    pub(super) fn config(&self) -> Result<Config, String> {
        let view = serde_json::from_value(json!(self.view)).map_err(|e| e.to_string())?;
        let target = serde_json::from_value(json!(self.target)).map_err(|e| e.to_string())?;
        let path =
            bevy::asset::AssetPath::try_parse(&self.content_path).map_err(|e| e.to_string())?;
        if view == target
            || self.index >= kedra_docking::snapshot::MAX_VIEWS
            || !matches!(path.source(), bevy::asset::io::AssetSourceId::Name(_))
            || path.label().is_some()
            || [
                &self.kind,
                &self.title,
                &self.content_path,
                &self.scope,
                &self.identity,
            ]
            .iter()
            .any(|text| text.is_empty() || text.len() > 4096)
            || self
                .restore_key
                .as_ref()
                .is_some_and(|key| key.len() > 4096)
        {
            return Err("Invalid docking overflow identity, placement, text or source-qualified content path".into());
        }
        Ok(Config {
            view,
            target,
            index: self.index,
            descriptor: View {
                kind: self.kind.clone(),
                restore_key: self.restore_key.clone(),
            },
            title: self.title.clone(),
            content_path: self.content_path.clone(),
            scope: self.scope.clone(),
            identity: self.identity.clone(),
        })
    }
}

impl Config {
    pub(super) fn validate_for(&self, snapshot: &Snapshot) -> Result<NodeId, String> {
        if snapshot.views.contains_key(&self.view) {
            return Err("Overflow view identity is already owned".into());
        }
        let group = snapshot
            .location(self.target)
            .ok_or("Overflow target is not visible")?
            .0;
        if !matches!(snapshot.nodes.get(&group), Some(kedra_docking::model::Node::Group { tabs, .. }) if self.index <= tabs.len())
        {
            return Err("Overflow insertion index is outside the target group".into());
        }
        Ok(group)
    }

    pub(super) fn check_content(&self, world: &World) -> Result<(), String> {
        crate::views::compiled(world, &self.content_path).map(|_| ())
    }
}

pub(super) fn apply(world: &mut World) -> Result<(), String> {
    require_binding(world)?;
    let config = world.resource::<Fixture>().overflow.clone();
    config.check_content(world)?;
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if let Some(existing) = state.layout.snapshot().views.get(&config.view) {
            return if existing == &config.descriptor {
                Ok(())
            } else {
                Err("Overflow view identity is already owned".into())
            };
        }
        let group = config.validate_for(state.layout.snapshot())?;
        let candidate = state
            .layout
            .prepare(Command::Open {
                id: config.view,
                view: config.descriptor.clone(),
                destination: Destination::Tabs {
                    group,
                    index: config.index,
                },
            })
            .map_err(|e| e.to_string())?;
        let mut allocated = None;
        if state.renderer.content(config.view).is_none() {
            let content =
                crate::views::mount_asset(world, state.stash, &config.content_path, &config.scope)?;
            let result = (|| {
                let identity = content.element("identity").map_err(|e| e.to_string())?;
                world
                    .get_mut::<Text>(identity)
                    .ok_or("Overflow identity text missing")?
                    .0 = config.identity.clone();
                state.renderer.register(config.view, content.root)
            })();
            if let Err(error) = result {
                world.despawn(content.root);
                return Err(error);
            }
            allocated = Some(content.root);
        }
        if let Err(error) = state.layout.commit(candidate) {
            if let Some(root) = allocated {
                state.renderer.unregister(config.view);
                world.despawn(root);
            }
            return Err(error.to_string());
        }
        state.titles.insert(config.view, config.title);
        state.focused_group = None;
        redraw(world, &mut state)
    })
}

#[cfg(test)]
pub(super) fn test_source() -> Result<OverflowSource, String> {
    let scene = kedra_scene::document::PrefabDocument::parse(include_str!(
        "../../../../../assets/editor/fixtures/docking.scene.ron"
    ))
    .map_err(|e| e.to_string())?;
    let root = scene
        .nodes
        .iter()
        .find(|node| node.id == scene.root)
        .ok_or("Dock source root missing")?;
    ron::from_str(
        root.components
            .get("editor.dock_overflow")
            .ok_or("Overflow source missing")?
            .value
            .get_ron(),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_specimen_and_target_placement_fail_before_model_changes() -> Result<(), String> {
        let source = test_source()?;
        let config = source.config()?;
        let host = HostId::new();
        let mut layout = Workspace::new(host);
        assert!(config.validate_for(layout.snapshot()).is_err());
        layout
            .execute(Command::Open {
                id: config.target,
                view: View {
                    kind: "Target".into(),
                    restore_key: None,
                },
                destination: Destination::EmptyHost(host),
            })
            .map_err(|e| e.to_string())?;
        let before = layout.snapshot().clone();
        assert!(config.validate_for(layout.snapshot()).is_ok());
        let mut bad = source.clone();
        bad.index = 2;
        assert!(bad.config()?.validate_for(layout.snapshot()).is_err());
        bad = source.clone();
        bad.view = source.target.clone();
        assert!(bad.config().is_err());
        bad = source.clone();
        bad.content_path = "fixtures/content.prefab.ron".into();
        assert!(bad.config().is_err());
        bad = source.clone();
        bad.view = "not-an-identity".into();
        assert!(bad.config().is_err());
        layout
            .execute(Command::Open {
                id: config.view,
                view: config.descriptor.clone(),
                destination: Destination::Tabs {
                    group: config.validate_for(&before)?,
                    index: config.index,
                },
            })
            .map_err(|e| e.to_string())?;
        assert!(config.validate_for(layout.snapshot()).is_err());
        assert_eq!(before.views.len(), 1);
        Ok(())
    }

    #[test]
    fn unavailable_or_incompatible_content_keeps_live_layout_and_bindings() -> Result<(), String> {
        let mut app = crate::workspace::browser_test_app().map_err(|e| e.to_string())?;
        let config = test_source()?.config()?;
        let world = app.world_mut();
        let (root, before) = world.resource_scope(|_, mut state: Mut<EditorWorkspace>| {
            let mut layout = Workspace::new(state.host);
            layout
                .execute(Command::Open {
                    id: config.target,
                    view: View {
                        kind: "Target".into(),
                        restore_key: None,
                    },
                    destination: Destination::EmptyHost(state.host),
                })
                .map_err(|e| e.to_string())?;
            state.layout = layout;
            Ok::<_, String>((state.root, state.layout.snapshot().clone()))
        })?;
        world.insert_resource(Fixture {
            root,
            original: before.clone(),
            views: [config.target, ViewId::new(), ViewId::new(), ViewId::new()],
            fields: vec![],
            generation: 1,
            overflow: config.clone(),
        });
        for path in [
            "editor://prefabs/not-admitted.prefab.ron",
            "editor://prefabs/property-value.prefab.ron",
        ] {
            world.resource_mut::<Fixture>().overflow.content_path = path.into();
            assert!(apply(world).is_err());
            let state = world.resource::<EditorWorkspace>();
            assert!(state.layout.snapshot() == &before);
            assert!(state.renderer.content(config.view).is_none());
            assert!(!state.titles.contains_key(&config.view));
            assert_eq!(world.resource::<Fixture>().generation, 1);
        }
        Ok(())
    }
}
