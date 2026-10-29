//! Editor recovery geometry and reversible production-theme interaction states.
use super::*;

#[test]
fn loading_recovery_actions_fit_the_minimum_native_client_size() -> Result<()> {
    use bevy::{
        ecs::system::SystemState,
        ui::{LayoutContext, ui_surface::UiSurface},
    };
    let (mut app, theme) = stylesheet(format!(
        "{}\n{}",
        include_str!("../../../../../assets/editor/styles/controls.css"),
        include_str!("../../../../../assets/editor/styles/startup.css")
    ))?;
    for (failed, height) in [(false, 360), (false, 320), (true, 360), (true, 320)] {
        let root = app
            .world_mut()
            .spawn((
                Node::default(),
                ChildOf(theme),
                ClassList::new("window-frame"),
            ))
            .id();
        app.world_mut()
            .spawn((Node::default(), ClassList::new("titlebar"), ChildOf(root)));
        let body = app
            .world_mut()
            .spawn((
                Node::default(),
                ClassList::new(if failed {
                    "loading-body failed"
                } else {
                    "loading-body"
                }),
                ChildOf(root),
            ))
            .id();
        app.world_mut().spawn((
            Node::default(),
            ClassList::new("engine-identity"),
            ChildOf(body),
        ));
        let details = app
            .world_mut()
            .spawn((
                Node::default(),
                ClassList::new("loading-details"),
                ChildOf(body),
            ))
            .id();
        for class in ["opening-project", "startup-status", "spacer"] {
            app.world_mut()
                .spawn((Node::default(), ClassList::new(class), ChildOf(details)));
        }
        let actions = app
            .world_mut()
            .spawn((
                Node::default(),
                ClassList::new("loading-actions"),
                ChildOf(details),
            ))
            .id();
        frames(&mut app);
        let mut surface = UiSurface::default();
        let context = LayoutContext {
            physical_size: Vec2::new(560., height as f32),
            ..default()
        };
        let nodes = app
            .world_mut()
            .query::<(Entity, &Node, Option<&Children>)>()
            .iter(app.world())
            .map(|(entity, node, children)| {
                (
                    entity,
                    node.clone(),
                    children.map(|c| c.to_vec()).unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        for (entity, node, _) in &nodes {
            surface.upsert_node(&context, *entity, node, None);
        }
        for (entity, _, children) in &nodes {
            surface.update_children(*entity, children.iter().copied());
        }
        let mut state =
            SystemState::<Query<&mut bevy::text::ComputedTextBlock>>::new(app.world_mut());
        let mut buffers = state.get_mut(app.world_mut())?;
        surface.compute_layout(
            root,
            UVec2::new(560, height),
            &mut buffers,
            &mut bevy::text::FontCx::default(),
        );
        let (body_layout, _) = surface.get_layout(body, true)?;
        let (details_layout, _) = surface.get_layout(details, true)?;
        let (actions_layout, _) = surface.get_layout(actions, true)?;
        let bottom = body_layout.location.y
            + details_layout.location.y
            + actions_layout.location.y
            + actions_layout.size.height;
        assert!(
            actions_layout.size.height >= 26. && bottom <= height as f32 - 1.,
            "Recovery clipped: failed={failed}, client height={height}, action bottom={bottom}"
        );
        app.world_mut().despawn(root);
    }
    Ok(())
}

#[test]
fn hover_and_press_feedback_is_distinct_and_reversible() -> Result<()> {
    // Font loading is not part of this color fixture; use the real theme rules.
    let tokens = include_str!("../../../../../assets/editor/styles/tokens.css")
        .lines()
        .filter(|line| !line.starts_with("@font-face"))
        .collect::<Vec<_>>()
        .join("\n");
    let (mut app, root) = stylesheet(format!(
        "{tokens}\n{}\n{}\n{}",
        include_str!("../../../../../assets/editor/styles/controls.css"),
        include_str!("../../../../../assets/editor/styles/docking.css"),
        include_str!("../../../../../assets/editor/styles/startup.css"),
    ))?;
    let button = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("button primary"),
            bevy::picking::hover::Hovered(false),
            ChildOf(root),
        ))
        .id();
    let selected = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("button flat selected"),
            ChildOf(root),
        ))
        .id();
    let preview = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("dock-preview"),
            ChildOf(root),
        ))
        .id();
    let frame = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("window-frame"),
            ChildOf(root),
        ))
        .id();
    let mut fills = Vec::new();
    for (hovered, pressed) in [(false, false), (true, false), (true, true), (false, false)] {
        app.world_mut()
            .entity_mut(button)
            .insert(bevy::picking::hover::Hovered(hovered));
        if pressed {
            app.world_mut().entity_mut(button).insert(bevy::ui::Pressed);
        } else {
            app.world_mut()
                .entity_mut(button)
                .remove::<bevy::ui::Pressed>();
        }
        frames(&mut app);
        fills.push(
            app.world()
                .get::<BackgroundColor>(button)
                .ok_or("button fill")?
                .0,
        );
    }
    assert_ne!(fills[0], fills[1]);
    assert_ne!(fills[1], fills[2]);
    assert_ne!(fills[0], fills[2]);
    assert_eq!(fills[0], fills[3]);
    assert!(
        app.world()
            .get::<BackgroundColor>(selected)
            .ok_or("selection fill")?
            .0
            .alpha()
            > 0.
    );
    assert!(
        app.world()
            .get::<BorderColor>(preview)
            .ok_or("preview border")?
            .top
            .alpha()
            > 0.
    );
    let alpha = app
        .world()
        .get::<BackgroundColor>(preview)
        .ok_or("preview fill")?
        .0
        .alpha();
    assert!(
        alpha > 0. && alpha < 1.,
        "Preview must preserve visibility of covered content"
    );
    assert_eq!(
        app.world().get::<Node>(frame).ok_or("frame node")?.border,
        UiRect::ZERO,
        "Even a transparent border leaves a canvas-colored perimeter stripe"
    );
    Ok(())
}
