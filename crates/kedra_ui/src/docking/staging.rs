//! Unpublished chrome allocation. A single temporary owner retires every new
//! hierarchy on failure; published chrome/content/focus are never rollback data.
use super::*;
use crate::authored::components::UiLayout;

pub(super) struct Prepared {
    owner: Option<Entity>,
    nodes: BTreeMap<NodeId, Chrome>,
    tabs: BTreeMap<ViewId, Tab>,
}

impl Prepared {
    pub(super) fn publish(self, world: &mut World, projection: &mut Projection, stash: Entity) {
        for chrome in self.nodes.values() {
            world.entity_mut(chrome.root()).insert(ChildOf(stash));
        }
        for tab in self.tabs.values() {
            world.entity_mut(tab.view.root).insert(ChildOf(stash));
        }
        projection.nodes.extend(self.nodes);
        projection.tabs.extend(self.tabs);
        if let Some(owner) = self.owner {
            world.despawn(owner);
        }
    }
}

impl Projection {
    #[allow(
        clippy::too_many_arguments,
        reason = "Explicit unpublished mount inputs"
    )]
    pub(super) fn prepare(
        &self,
        world: &mut World,
        state: &Snapshot,
        stash: Entity,
        skin: &Skin,
        scope: &str,
        density: f32,
    ) -> Result<Prepared, String> {
        if !density.is_finite() || density <= 0. {
            return Err("UI density must be finite and positive".into());
        }
        for (compiled, keys) in [
            (&skin.group, &["tabs", "body", "menu"][..]),
            (&skin.tab, &["label", "title", "close"][..]),
            (&skin.split, &["divider"][..]),
        ] {
            bindings(compiled, keys)?;
        }
        for (id, chrome) in &self.nodes {
            match (state.nodes.get(id), chrome) {
                (Some(Node::Group { .. }), Chrome::Split { .. })
                | (Some(Node::Split { .. }), Chrome::Group { .. }) => {
                    return Err("Docking chrome kind changed under stable identity".into());
                }
                _ => {}
            }
            let view = match chrome {
                Chrome::Group { view, .. } | Chrome::Split { view, .. } => view,
            };
            live_bindings(world, view)?;
        }
        for tab in self.tabs.values() {
            live_bindings(world, &tab.view)?;
        }
        let needs_mount = state.nodes.keys().any(|id| !self.nodes.contains_key(id))
            || state.nodes.values().any(|node| {
                matches!(node,
                Node::Group { tabs, .. } if tabs.iter().any(|id| !self.tabs.contains_key(id)))
            });
        if !needs_mount {
            return Ok(Prepared {
                owner: None,
                nodes: BTreeMap::new(),
                tabs: BTreeMap::new(),
            });
        }
        // Parent scope is needed for inherited stylesheet checks during mounting.
        // Nothing is detached from the committed tree until every mount succeeds.
        let had_children = world.get::<Children>(stash).is_some();
        let owner = world
            .spawn((bevy::ui::Node::default(), ChildOf(stash)))
            .id();
        let mut prepared = Prepared {
            owner: Some(owner),
            nodes: BTreeMap::new(),
            tabs: BTreeMap::new(),
        };
        let result = (|| {
            for (id, node) in &state.nodes {
                if !self.nodes.contains_key(id) {
                    let compiled = match node {
                        Node::Group { .. } => &skin.group,
                        Node::Split { .. } => &skin.split,
                    };
                    let view = UiView::mount(
                        world,
                        compiled,
                        Some(owner),
                        &BTreeMap::new(),
                        &format!("{scope}/node/{id:?}"),
                        density,
                    )
                    .map_err(|e| e.to_string())?;
                    let chrome = match node {
                        Node::Group { .. } => {
                            let tabs = element(&view, "tabs")?;
                            let body = element(&view, "body")?;
                            let menu = element(&view, "menu")?;
                            let tabs_parent = world
                                .get::<ChildOf>(tabs)
                                .ok_or("Tabs lost their authored parent")?
                                .parent();
                            let menu_parent = world
                                .get::<ChildOf>(menu)
                                .ok_or("Menu lost its authored parent")?
                                .parent();
                            world.entity_mut(menu).insert(Control::Menu(*id));
                            Chrome::Group {
                                view,
                                tabs,
                                body,
                                menu,
                                tabs_parent,
                                menu_parent,
                            }
                        }
                        Node::Split { .. } => {
                            let divider = element(&view, "divider")?;
                            Chrome::Split { view, divider }
                        }
                    };
                    prepared.nodes.insert(*id, chrome);
                }
            }
            for node in state.nodes.values() {
                let Node::Group { tabs, .. } = node else {
                    continue;
                };
                for id in tabs {
                    if self.tabs.contains_key(id) {
                        continue;
                    }
                    let view = UiView::mount(
                        world,
                        &skin.tab,
                        Some(owner),
                        &BTreeMap::new(),
                        &format!("{scope}/view/{id:?}"),
                        density,
                    )
                    .map_err(|e| e.to_string())?;
                    let label = element(&view, "label")?;
                    let title = element(&view, "title")?;
                    world
                        .entity_mut(view.root)
                        .insert(bevy::picking::hover::Hovered::default());
                    world.entity_mut(label).insert(Control::Tab(*id));
                    world
                        .entity_mut(element(&view, "close")?)
                        .insert(Control::Close(*id));
                    let fitting = super::super::title::Title::new(world, label);
                    prepared.tabs.insert(
                        *id,
                        Tab {
                            view,
                            title,
                            label,
                            fitting,
                        },
                    );
                }
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            world.despawn(owner);
            if !had_children && world.get::<Children>(stash).is_some_and(|c| c.is_empty()) {
                world.entity_mut(stash).remove::<Children>();
            }
            return Err(error);
        }
        Ok(prepared)
    }
}

fn bindings(compiled: &CompiledPrefab, keys: &[&str]) -> Result<(), String> {
    let source = compiled.authoring_world();
    for key in keys {
        let mut matches = compiled
            .source_entities()
            .values()
            .copied()
            .filter(|entity| {
                source
                    .get::<crate::ElementId>(*entity)
                    .is_some_and(|id| id.0 == *key)
            });
        let entity = matches
            .next()
            .ok_or_else(|| format!("required authored UI element is absent: {key}"))?;
        if matches.next().is_some() || source.get::<UiLayout>(entity).is_none() {
            return Err(format!(
                "Docking binding must identify one authored UI layout: {key}"
            ));
        }
    }
    Ok(())
}

fn live_bindings(world: &World, view: &UiView) -> Result<(), String> {
    for entity in std::iter::once(&view.root).chain(view.elements().values()) {
        if world.get::<bevy::ui::Node>(*entity).is_none() {
            return Err("Docking projection contains retired or non-UI chrome".into());
        }
    }
    Ok(())
}
