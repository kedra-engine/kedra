//! Canonical dock-group/panel menu composition; feature controls share this host.
use super::*;

pub(super) fn menu(
    world: &mut World,
    state: &mut EditorWorkspace,
    group: Option<NodeId>,
    anchor: Entity,
) -> Result<(), String> {
    cancel(world, state);
    dismiss(world, state);
    let host = group
        .and_then(|group| state.layout.snapshot().host_of(group))
        .unwrap_or(state.host);
    let _surface = state
        .surfaces
        .get(&host)
        .ok_or("Menu host was retired")?
        .root;
    let menu = super::super::popup::attach(world, anchor)?;
    state.menu.push(menu);
    let mut items = Vec::new();
    if state.documents.can_save() {
        items.push((
            if state.documents.save_pending() {
                "Retry Save"
            } else {
                "Save active document"
            }
            .into(),
            Action::SaveSource,
        ));
    }
    if !state.documents.update_clean() {
        items.push((
            "Discard all unsaved source changes".into(),
            Action::DiscardSources,
        ));
    }
    if state.documents.can_history(false) {
        items.push(("Undo source edit".into(), Action::SourceHistory(false)));
    }
    if state.documents.can_history(true) {
        items.push(("Redo source edit".into(), Action::SourceHistory(true)));
    }
    if let Some(group) = group {
        let maximized = state.layout.snapshot().hosts[&host].maximized == Some(group);
        items.push((
            if maximized {
                "Restore group"
            } else {
                "Maximize group"
            }
            .into(),
            Action::Maximize(group),
        ));
        items.extend([
            ("Float group".into(), Action::Float(group)),
            ("Hide group".into(), Action::Hide(group)),
        ]);
        if let Some(Node::Group { active, .. }) = state.layout.snapshot().nodes.get(&group) {
            if state
                .layout
                .snapshot()
                .views
                .get(active)
                .is_some_and(|v| v.kind == "assets")
            {
                items.push(("Open selected source".into(), Action::BrowserOpen(*active)));
                items.push(("Refresh sources".into(), Action::BrowserRefresh(*active)));
            }
            if state
                .layout
                .snapshot()
                .views
                .get(active)
                .is_some_and(|v| v.kind == "hierarchy")
            {
                items.push(("Search nodes".into(), Action::HierarchySearch(*active)));
                items.push((
                    "Reveal selected node".into(),
                    Action::HierarchyReveal(*active),
                ));
                items.push((
                    "Toggle source details".into(),
                    Action::HierarchyDetails(*active),
                ));
            }
            if let Some(panel) = state.panels.inspectors.get(active) {
                items.push(("Search properties".into(), Action::InspectorSearch(*active)));
                if panel.state.target.is_some() {
                    items.push((
                        if panel.state.locked {
                            "Unlock Inspector"
                        } else {
                            "Lock Inspector"
                        }
                        .into(),
                        Action::InspectorLock(*active),
                    ));
                }
                items.push((
                    if panel.state.raw {
                        "Use custom presentations"
                    } else {
                        "Use default reflected view"
                    }
                    .into(),
                    Action::InspectorRaw(*active),
                ));
                items.push((
                    "Toggle source details".into(),
                    Action::InspectorDetails(*active),
                ));
            }
            items.push(("Float active tab".into(), Action::FloatView(*active)));
        }
        if host != state.host {
            items.push(("Redock to main window".into(), Action::Redock(group)));
        }
        if let Some(Node::Group { tabs, .. }) = state.layout.snapshot().nodes.get(&group) {
            for view in tabs {
                items.push((
                    state.titles.get(view).cloned().unwrap_or_default(),
                    Action::Select(*view),
                ));
            }
        }
    } else {
        for (view, record) in &state.layout.snapshot().views {
            let hidden = state.layout.snapshot().hidden.contains_key(view);
            items.push((
                format!(
                    "{}{}",
                    if hidden { "Show " } else { "Select " },
                    state.titles.get(view).unwrap_or(&record.kind)
                ),
                if hidden {
                    Action::Show(*view)
                } else {
                    Action::Select(*view)
                },
            ));
        }
    }
    for (index, (title, action)) in items.into_iter().enumerate() {
        let view = crate::views::mount_asset(
            world,
            menu,
            "editor://prefabs/button.prefab.ron",
            &format!("dock-menu/{index}"),
        )?;
        // The button prefab deliberately exposes its caption via its native graph.
        let text = world
            .query::<(Entity, &ChildOf, &Text)>()
            .iter(world)
            .find_map(|(id, parent, _)| (parent.parent() == view.root).then_some(id))
            .ok_or("Menu button caption is absent")?;
        if let Some(mut value) = world.get_mut::<Text>(text) {
            value.0 = title;
        }
        world
            .entity_mut(view.root)
            .remove::<(bevy::ui_widgets::Button, bevy::a11y::AccessibilityNode)>()
            .insert(bevy::ui_widgets::MenuItem);
        world.entity_mut(view.root).insert((
            action,
            TabIndex(index as i32),
            kedra_ui::ElementId::new(format!("dock-menu/{index}")),
        ));
        world
            .entity_mut(text)
            .insert(kedra_ui::ElementId::new(format!(
                "dock-menu/{index}/caption"
            )));
    }
    Ok(())
}
