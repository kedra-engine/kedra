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
    state.documents.origin =
        group.and_then(|group| match state.layout.snapshot().nodes.get(&group) {
            Some(Node::Group { active, .. }) if state.panels.browsers.contains_key(active) => {
                Some(*active)
            }
            _ => None,
        });
    let mut items = Vec::new();
    let mut separators = std::collections::BTreeSet::new();
    if let Some(group) = group {
        let maximized = state.layout.snapshot().hosts[&host].maximized == Some(group);
        let mut layout = vec![(
            if maximized {
                "Restore group"
            } else {
                "Maximize group"
            }
            .into(),
            Action::Maximize(group),
        )];
        if let Some(Node::Group { tabs, .. }) = state.layout.snapshot().nodes.get(&group)
            && tabs.len() > 1
        {
            if host == state.host {
                layout.push(("Float group".into(), Action::Float(group)));
            }
            layout.push(("Close group".into(), Action::Hide(group)));
            items.push((
                "Tabs".into(),
                Action::Submenu(
                    tabs.iter()
                        .map(|view| kedra_ui::popup::Entry::Command {
                            label: state
                                .titles
                                .get(view)
                                .unwrap_or(&state.layout.snapshot().views[view].kind)
                                .clone(),
                            command: Action::Select(*view),
                        })
                        .collect(),
                ),
            ));
        }
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
                items.extend(super::super::document_menu::entry_items(*active)?);
                if let Some(path) = state
                    .panels
                    .browsers
                    .get(active)
                    .and_then(|panel| panel.selected_path())
                {
                    items.push((
                        "Rename".into(),
                        Action::Folder(
                            *active,
                            super::super::folders::Mode::Rename,
                            Some(path.clone()),
                        ),
                    ));
                    items.push((
                        "Delete".into(),
                        Action::Folder(*active, super::super::folders::Mode::Delete, Some(path)),
                    ));
                }
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
            if !items.is_empty() {
                separators.insert(items.len());
            }
            items.push(("Close tab".into(), Action::CloseView(*active)));
            if host == state.host {
                layout.push(("Float tab".into(), Action::FloatView(*active)));
            }
        }
        if host != state.host {
            layout.push(("Redock to main window".into(), Action::Redock(group)));
        }
        items.push((
            "Layout".into(),
            Action::Submenu(
                layout
                    .into_iter()
                    .map(|(label, command)| kedra_ui::popup::Entry::Command { label, command })
                    .collect(),
            ),
        ));
    } else {
        items.extend([
            (
                if state.documents.save_pending() {
                    "Retry Save"
                } else {
                    "Save"
                }
                .into(),
                Action::SaveSource,
            ),
            ("Save All".into(), Action::SaveAll),
            ("Undo".into(), Action::SourceHistory(false)),
            ("Redo".into(), Action::SourceHistory(true)),
        ]);
        let mut windows = Vec::new();
        for (view, record) in &state.layout.snapshot().views {
            let hidden = state.layout.snapshot().hidden.contains_key(view);
            windows.push(kedra_ui::popup::Entry::Command {
                label: state.titles.get(view).unwrap_or(&record.kind).clone(),
                command: if hidden {
                    Action::Show(*view)
                } else {
                    Action::Select(*view)
                },
            });
        }
        windows.push(kedra_ui::popup::Entry::Command {
            label: "Default layout".into(),
            command: Action::Reset,
        });
        separators.extend([2, 4, 5]);
        items.push(("Windows".into(), Action::Submenu(windows)));
        items.push(("Exit".into(), Action::Exit));
    }
    for (index, (title, action)) in items.into_iter().enumerate() {
        if separators.contains(&index) {
            super::super::popup::separator(world, menu);
        }
        let branch = matches!(action, Action::Submenu(_));
        let enabled = super::super::source_history::available(world, state, &action);
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
        if !enabled {
            world
                .entity_mut(view.root)
                .insert(bevy::ui::InteractionDisabled);
        }
        if branch {
            super::super::popup::branch_indicator(world, view.root, text);
            kedra_ui::popup::cascade::branch(world, view.root);
        }
        world
            .entity_mut(text)
            .insert(kedra_ui::ElementId::new(format!(
                "dock-menu/{index}/caption"
            )));
    }
    Ok(())
}
