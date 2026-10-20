//! Explicit viewport commands share the existing authored popup/button family.
use super::input::Action;
use super::*;
use crate::scene_view::camera::Command as Camera;
#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Views,
    Projection,
    Lighting,
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
        Kind::Projection => vec![
            ("Perspective", Action::SceneProjection(view, false)),
            ("Orthographic", Action::SceneProjection(view, true)),
        ],
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
        ],
    };
    for (index, (title, action)) in items.into_iter().enumerate() {
        let button = crate::views::mount(
            world,
            popup,
            crate::views::document(0xc0100000000040008000000000000001),
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
        world
            .entity_mut(caption)
            .insert(kedra_ui::ElementId::new(format!(
                "scene-menu/{index}/caption"
            )));
    }
    Ok(())
}
