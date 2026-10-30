//! Explicit viewport commands share the existing authored popup/button family.
use super::input::Action;
use super::*;
use crate::scene_view::camera::Command as Camera;
pub(super) fn open(
    world: &mut World,
    state: &mut EditorWorkspace,
    view: ViewId,
    anchor: Entity,
) -> Result<(), String> {
    scene_input::cancel(world);
    let popup = popup::attach(world, anchor)?;
    state.menu.push(popup);
    let items = vec![
        ("Frame all", Action::SceneCommand(view, Camera::FrameAll)),
        ("Clear selection", Action::SceneClear(view)),
        ("Toggle selection outlines", Action::SceneOverlays(view)),
        ("Toggle grid", Action::SceneGrid(view)),
        ("Scene lighting", Action::SceneLight(view, false)),
        ("Editor lighting", Action::SceneLight(view, true)),
    ];
    for (index, (title, action)) in items.into_iter().enumerate() {
        if [1, 4].contains(&index) {
            super::popup::separator(world, popup);
        }
        let button = crate::views::mount_asset(
            world,
            popup,
            "editor://prefabs/button.prefab.ron",
            &format!("scene-menu/{index}"),
        )?;
        let caption = world
            .query::<(Entity, &ChildOf, &Text)>()
            .iter(world)
            .find_map(|(entity, parent, _)| (parent.parent() == button.root).then_some(entity))
            .ok_or("Menu caption missing")?;
        world
            .get_mut::<Text>(caption)
            .ok_or("Menu caption retired")?
            .0 = title.into();
        world
            .entity_mut(button.root)
            .remove::<(bevy::ui_widgets::Button, bevy::a11y::AccessibilityNode)>()
            .insert(bevy::ui_widgets::MenuItem);
        world.entity_mut(button.root).insert((
            action.clone(),
            bevy::input_focus::tab_navigation::TabIndex(index as i32),
            kedra_ui::ElementId::new(format!("scene-menu/{index}")),
        ));
        if matches!(
            action,
            Action::SceneCommand(..) | Action::SceneLight(..) | Action::SceneGrid(..)
        ) && state
            .panels
            .scenes
            .get(&view)
            .and_then(|scene| scene.navigation(world))
            .is_none()
        {
            world
                .entity_mut(button.root)
                .insert(bevy::ui::InteractionDisabled);
        }
        if matches!(action, Action::SceneClear(..))
            && !state
                .documents
                .active
                .and_then(|document| state.documents.opened.get(&document))
                .is_some_and(|document| {
                    document.view == view
                        && !state.documents.selected_nodes(document.document).is_empty()
                })
        {
            world
                .entity_mut(button.root)
                .insert(bevy::ui::InteractionDisabled);
        }
        world
            .entity_mut(caption)
            .insert(kedra_ui::ElementId::new(format!(
                "scene-menu/{index}/caption"
            )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_menu_without_document_selection_does_not_advertise_an_active_clear_command()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, _, _) = super::super::test_support::setup()?;
        let world = app.world_mut();
        super::super::test_support::sources(world)?;
        let root = world.resource::<EditorWorkspace>().root;
        let anchor = world.spawn((Node::default(), ChildOf(root))).id();
        let view = ViewId::new();
        let document = kedra_scene::document::PrefabDocument::new("Remembered context");
        let mut catalog = kedra_scene::composition::Catalog::default();
        catalog.insert(document.clone())?;
        {
            let mut state = world.resource_mut::<EditorWorkspace>();
            state.documents.active = Some(document.id);
            state.documents.opened.insert(
                document.id,
                crate::documents::Opened {
                    #[cfg(debug_assertions)]
                    path: "remembered.prefab.ron".into(),
                    document: document.id,
                    view,
                    selected: Some(document.root.into()),
                    graph: std::sync::Arc::new(catalog.resolve(document.id)?),
                },
            );
            assert!(state.documents.selection.items().is_empty());
        }
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            open(world, &mut state, view, anchor)
        })?;
        let item = world
            .query::<(Entity, &kedra_ui::ElementId)>()
            .iter(world)
            .find(|(_, id)| id.0 == "scene-menu/1")
            .ok_or("Clear command missing")?
            .0;
        assert!(world.get::<bevy::ui::InteractionDisabled>(item).is_some());
        Ok(())
    }
    #[test]
    fn settings_keeps_scene_options_without_duplicating_navigation_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, _, _) = super::super::test_support::setup()?;
        let world = app.world_mut();
        super::super::test_support::sources(world)?;
        let root = world.resource::<EditorWorkspace>().root;
        let anchor = world.spawn((Node::default(), ChildOf(root))).id();
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            open(world, &mut state, ViewId::new(), anchor)
        })?;
        let captions = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>();
        for caption in [
            "Frame all",
            "Clear selection",
            "Toggle selection outlines",
            "Toggle grid",
            "Scene lighting",
            "Editor lighting",
        ] {
            assert!(
                captions.contains(&caption),
                "Missing View command: {caption}"
            );
        }
        for caption in [
            "Frame selection",
            "Front",
            "Back",
            "Left",
            "Right",
            "Top",
            "Bottom",
            "Perspective",
            "Orthographic",
        ] {
            assert!(
                !captions.contains(&caption),
                "Duplicated navigation command: {caption}"
            );
        }
        Ok(())
    }
}
