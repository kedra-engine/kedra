//! Explicit viewport commands share the existing authored popup/button family.
use super::input::Action;
use super::*;
use crate::scene_view::camera::Command as Camera;
#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Views,
    Lighting,
    Selection,
}
pub(super) fn open(
    world: &mut World,
    state: &mut EditorWorkspace,
    view: ViewId,
    anchor: Entity,
    kind: Kind,
) -> Result<(), String> {
    scene_input::cancel(world);
    let popup = popup::attach(world, anchor)?;
    state.menu.push(popup);
    let items = match kind {
        Kind::Lighting => vec![
            ("Scene lighting", Action::SceneLight(view, false)),
            ("Editor lighting", Action::SceneLight(view, true)),
        ],
        Kind::Views => vec![
            (
                "Frame selection",
                Action::SceneCommand(view, Camera::FrameSelection),
            ),
            ("Frame all", Action::SceneCommand(view, Camera::FrameAll)),
            ("Front", Action::SceneCommand(view, Camera::Front)),
            ("Back", Action::SceneCommand(view, Camera::Back)),
            ("Left", Action::SceneCommand(view, Camera::Left)),
            ("Right", Action::SceneCommand(view, Camera::Right)),
            ("Top", Action::SceneCommand(view, Camera::Top)),
            ("Bottom", Action::SceneCommand(view, Camera::Bottom)),
            ("Perspective", Action::SceneProjection(view, false)),
            ("Orthographic", Action::SceneProjection(view, true)),
            ("Clear selection", Action::SceneClear(view)),
        ],
        Kind::Selection => vec![("Clear selection", Action::SceneClear(view))],
    };
    for (index, (title, action)) in items.into_iter().enumerate() {
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
            action,
            bevy::input_focus::tab_navigation::TabIndex(index as i32),
            kedra_ui::ElementId::new(format!("scene-menu/{index}")),
        ));
        if matches!(
            action,
            Action::SceneCommand(..) | Action::SceneProjection(..)
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
                .is_some_and(|document| document.view == view && document.selected.is_some())
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
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            open(world, &mut state, ViewId::new(), anchor, Kind::Selection)
        })?;
        let item = world
            .query::<(Entity, &kedra_ui::ElementId)>()
            .iter(world)
            .find(|(_, id)| id.0 == "scene-menu/0")
            .ok_or("Clear command missing")?
            .0;
        assert!(world.get::<bevy::ui::InteractionDisabled>(item).is_some());
        Ok(())
    }
    #[test]
    fn view_menu_exposes_projection_and_clear_selection_without_a_second_toolbar_select()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, _, _) = super::super::test_support::setup()?;
        let world = app.world_mut();
        super::super::test_support::sources(world)?;
        let root = world.resource::<EditorWorkspace>().root;
        let anchor = world.spawn((Node::default(), ChildOf(root))).id();
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            open(world, &mut state, ViewId::new(), anchor, Kind::Views)
        })?;
        let captions = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>();
        for caption in [
            "Frame selection",
            "Frame all",
            "Front",
            "Top",
            "Perspective",
            "Orthographic",
            "Clear selection",
        ] {
            assert!(
                captions.contains(&caption),
                "Missing View command: {caption}"
            );
        }
        Ok(())
    }
}
