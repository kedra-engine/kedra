//! Editor-only adaptation of an authored Snapshot to the live main host and tool identities.
use super::*;
use kedra_docking::{
    model::{Node, View},
    snapshot::Snapshot,
};
use std::collections::BTreeSet;

pub(super) struct Preset(Snapshot);

impl Preset {
    pub(super) fn load(world: &World) -> Result<Self, String> {
        let scene = crate::views::compiled(world, "editor://scenes/editor.scene.ron")?;
        let root = scene
            .source_entities()
            .get(&scene.graph().root)
            .ok_or("Editor source root missing")?;
        let source = scene
            .authoring_world()
            .get::<kedra_ui::docking::scene::InitialLayout>(*root)
            .ok_or("Editor scene has no initial docking layout")?;
        Self::new(
            source
                .workspace()
                .map_err(|e| e.to_string())?
                .snapshot()
                .clone(),
        )
    }

    fn new(snapshot: Snapshot) -> Result<Self, String> {
        snapshot.encode().map_err(|e| e.to_string())?;
        if snapshot.hosts.len() != 1
            || !snapshot.hidden.is_empty()
            || snapshot.hosts.values().any(|host| host.maximized.is_some())
        {
            return Err("Editor preset requires one unmaximized host and visible tools".into());
        }
        let kinds: BTreeSet<_> = snapshot
            .views
            .values()
            .map(|view| view.kind.as_str())
            .collect();
        if kinds.len() != snapshot.views.len()
            || !kinds.contains("preview")
            || !kinds.contains("assets")
            || kinds.iter().any(|kind| super::panels::retired(kind))
        {
            return Err(
                "Editor preset requires unique tool kinds and preview/assets anchors".into(),
            );
        }
        Ok(Self(snapshot))
    }

    /// Template view IDs are source aliases, not replacements for persisted identities.
    /// Only missing tools receive new ViewIds. All other descriptors survive, with
    /// source documents joining preview and additional tools joining Assets as before.
    pub(super) fn arrange(
        &self,
        host: HostId,
        mut records: BTreeMap<ViewId, View>,
    ) -> Result<(Workspace, NodeId), String> {
        let missing = self
            .0
            .views
            .values()
            .filter(|template| !records.values().any(|view| view.kind == template.kind))
            .count();
        if records.len().saturating_add(missing) > kedra_docking::snapshot::MAX_VIEWS {
            return Err(
                "Reset layout would exceed the view limit; existing layout was kept".into(),
            );
        }
        let mut candidate = self.0.clone();
        let mut identities = BTreeMap::new();
        let mut views = BTreeMap::new();
        for (alias, template) in &self.0.views {
            let existing = records
                .iter()
                .find_map(|(id, view)| (view.kind == template.kind).then_some(*id));
            let id = existing.unwrap_or_else(ViewId::new);
            if views.contains_key(&id) || (existing.is_none() && records.contains_key(&id)) {
                return Err("New preset view identity collides with a retained record".into());
            }
            let descriptor = records.remove(&id).unwrap_or_else(|| template.clone());
            identities.insert(*alias, id);
            views.insert(id, descriptor);
        }
        candidate.views = views;
        // Reset has always replaced topology nodes, not content identities. Fresh
        // nodes prevent old splitter measurements/gesture IDs from aliasing defaults.
        let nodes: BTreeMap<_, _> = candidate
            .nodes
            .keys()
            .map(|id| (*id, NodeId::new()))
            .collect();
        if nodes.values().collect::<BTreeSet<_>>().len() != nodes.len() {
            return Err("New preset topology identities collide".into());
        }
        let mut main = candidate
            .hosts
            .values()
            .next()
            .cloned()
            .ok_or("Preset host missing")?;
        main.root = main
            .root
            .map(|id| nodes.get(&id).copied().ok_or("Preset root alias missing"))
            .transpose()?;
        candidate.hosts = BTreeMap::from([(host, main)]);
        let mut active = Vec::new();
        for node in candidate.nodes.values_mut() {
            if let Node::Group {
                tabs,
                active: selected,
            } = node
            {
                for tab in tabs {
                    *tab = identities
                        .get(&*tab)
                        .copied()
                        .ok_or("Preset view alias missing")?;
                }
                *selected = identities
                    .get(&*selected)
                    .copied()
                    .ok_or("Preset active alias missing")?;
                active.push(*selected);
            } else if let Node::Split { first, second, .. } = node {
                *first = nodes
                    .get(&*first)
                    .copied()
                    .ok_or("Preset child alias missing")?;
                *second = nodes
                    .get(&*second)
                    .copied()
                    .ok_or("Preset child alias missing")?;
            }
        }
        candidate.nodes = candidate
            .nodes
            .into_iter()
            .map(|(id, node)| (nodes[&id], node))
            .collect();
        let mut layout = candidate.into_workspace().map_err(|e| e.to_string())?;
        let group = |layout: &Workspace, kind: &str| -> Result<NodeId, String> {
            let view = layout
                .snapshot()
                .views
                .iter()
                .find_map(|(id, view)| (view.kind == kind).then_some(*id))
                .ok_or_else(|| format!("Preset anchor missing: {kind}"))?;
            layout
                .snapshot()
                .location(view)
                .map(|(group, _)| group)
                .ok_or("Preset anchor is hidden".into())
        };
        let preview = group(&layout, "preview")?;
        let assets = group(&layout, "assets")?;
        for (id, view) in records {
            let group = if matches!(view.kind.as_str(), "document" | "asset-preview") {
                preview
            } else {
                assets
            };
            let index = match &layout.snapshot().nodes[&group] {
                Node::Group { tabs, .. } => tabs.len(),
                _ => return Err("Preset anchor is not a tab group".into()),
            };
            layout
                .execute(Command::Open {
                    id,
                    view,
                    destination: Destination::Tabs { group, index },
                })
                .map_err(|e| e.to_string())?;
        }
        for id in active {
            layout
                .execute(Command::Activate(id))
                .map_err(|e| e.to_string())?;
        }
        layout.snapshot().encode().map_err(|e| e.to_string())?;
        Ok((layout, preview))
    }

    #[cfg(test)]
    pub(super) fn test_source() -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct SourceLayout {
            snapshot: String,
        }
        let scene = kedra_scene::document::PrefabDocument::parse(include_str!(
            "../../../../assets/editor/scenes/editor.scene.ron"
        ))
        .map_err(|e| e.to_string())?;
        let root = scene
            .nodes
            .iter()
            .find(|node| node.id == scene.root)
            .ok_or("Editor root missing")?;
        let source: SourceLayout = ron::from_str(
            root.components
                .get("ui.docking")
                .ok_or("Editor layout missing")?
                .value
                .get_ron(),
        )
        .map_err(|e| e.to_string())?;
        let layout = kedra_ui::docking::scene::InitialLayout {
            snapshot: source.snapshot,
        };
        Self::new(
            layout
                .workspace()
                .map_err(|e| e.to_string())?
                .snapshot()
                .clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kedra_docking::model::Host;

    #[test]
    fn authored_snapshot_reconciles_without_losing_descriptors_or_host_identity()
    -> Result<(), String> {
        let preset = Preset::test_source()?;
        let source = preset.0.clone();
        let host = HostId::new();
        let assets = ViewId::new();
        let hierarchy = ViewId::new();
        let second_hierarchy = ViewId::new();
        let document = ViewId::new();
        let extension = ViewId::new();
        let descriptor = |kind: &str, key: Option<&str>| View {
            kind: kind.into(),
            restore_key: key.map(str::to_owned),
        };
        let records = BTreeMap::from([
            (assets, descriptor("assets", Some("source-root"))),
            (hierarchy, descriptor("hierarchy", None)),
            (second_hierarchy, descriptor("hierarchy", Some("second"))),
            (document, descriptor("document", Some("Main.scene.ron"))),
            (extension, descriptor("extension.tool", Some("日本語"))),
        ]);
        let (layout, focused) = preset.arrange(host, records.clone())?;
        assert_eq!(
            layout.snapshot().hosts.keys().copied().collect::<Vec<_>>(),
            vec![host]
        );
        for (id, record) in &records {
            assert_eq!(layout.snapshot().views.get(id), Some(record));
        }
        assert_eq!(
            layout.snapshot().location(document).map(|(group, _)| group),
            Some(focused)
        );
        assert_eq!(
            layout
                .snapshot()
                .location(extension)
                .map(|(group, _)| group),
            layout.snapshot().location(assets).map(|(group, _)| group)
        );
        assert_eq!(preset.0, source);
        let stale = layout
            .prepare(Command::Activate(assets))
            .map_err(|e| e.to_string())?;
        let (mut independent, _) = preset.arrange(host, layout.snapshot().views.clone())?;
        assert!(
            independent
                .snapshot()
                .nodes
                .keys()
                .all(|id| !layout.snapshot().nodes.contains_key(id))
        );
        assert_eq!(independent.snapshot().views, layout.snapshot().views);
        assert_eq!(independent.commit(stale), Err(kedra_docking::Error::Stale));
        // The topology itself remains source-owned, including an alternate valid fraction.
        let mut alternate = source;
        let split = alternate
            .nodes
            .values_mut()
            .find(|node| matches!(node, Node::Split { .. }))
            .ok_or("Split missing")?;
        if let Node::Split { fraction, .. } = split {
            *fraction = 0.33;
        }
        let alternate = Preset::new(alternate)?;
        let (changed, _) = alternate.arrange(host, records)?;
        assert!(
            changed
                .snapshot()
                .nodes
                .values()
                .any(|node| matches!(node, Node::Split { fraction, .. } if *fraction == 0.33))
        );
        Ok(())
    }

    #[test]
    fn ambiguous_hidden_foreign_and_invalid_templates_fail_without_publication()
    -> Result<(), String> {
        let preset = Preset::test_source()?;
        let before = preset.0.clone();
        let mut foreign = before.clone();
        foreign.hosts.insert(
            HostId::new(),
            Host {
                root: None,
                maximized: None,
            },
        );
        assert!(Preset::new(foreign).is_err());
        let mut duplicate = before.clone();
        let hierarchy = duplicate
            .views
            .values_mut()
            .find(|view| view.kind == "hierarchy")
            .ok_or("Hierarchy missing")?;
        hierarchy.kind = "preview".into();
        assert!(Preset::new(duplicate).is_err());
        let mut retired = before.clone();
        retired
            .views
            .values_mut()
            .find(|view| view.kind == "hierarchy")
            .ok_or("Hierarchy missing")?
            .kind = "notes".into();
        assert!(Preset::new(retired).is_err());
        let mut missing = before.clone();
        missing
            .views
            .values_mut()
            .find(|view| view.kind == "assets")
            .ok_or("Assets missing")?
            .kind = "other".into();
        assert!(Preset::new(missing).is_err());
        let mut hidden = before.clone().into_workspace().map_err(|e| e.to_string())?;
        let view = *hidden
            .snapshot()
            .views
            .keys()
            .next()
            .ok_or("View missing")?;
        hidden
            .execute(Command::Hide(view))
            .map_err(|e| e.to_string())?;
        assert!(Preset::new(hidden.snapshot().clone()).is_err());
        let mut invalid = before.clone();
        invalid.nodes.clear();
        assert!(Preset::new(invalid).is_err());
        assert_eq!(preset.0, before);
        Ok(())
    }
}
