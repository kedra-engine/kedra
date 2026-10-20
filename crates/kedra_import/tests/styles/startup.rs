//! Editor theme consumers must share the approved palette after delivery import.
use super::*;

#[test]
fn loading_recovery_actions_fit_the_minimum_native_client_size() -> Result<()> {
    use bevy::{
        ecs::system::SystemState,
        ui::{LayoutContext, ui_surface::UiSurface},
    };
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(
        source.join("startup.css"),
        format!(
            "{}\n{}",
            include_str!("../../../../assets/editor/styles/controls.css"),
            include_str!("../../../../assets/editor/styles/startup.css")
        ),
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    let mut app = consumer(&output, manifest);
    let sheet = load(&mut app, "game://startup.css")?;
    for (failed, height) in [(false, 360), (false, 320), (true, 360), (true, 320)] {
        let root = app
            .world_mut()
            .spawn((
                Node::default(),
                Styled::new(sheet.clone()),
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
fn editor_controls_share_the_purple_interaction_palette() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    // Font loading is not part of this color fixture; use the real theme rules.
    let tokens = include_str!("../../../../assets/editor/styles/tokens.css")
        .lines()
        .filter(|line| !line.starts_with("@font-face"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        source.join("theme.css"),
        format!(
            "{tokens}\n{}\n{}\n{}",
            include_str!("../../../../assets/editor/styles/controls.css"),
            include_str!("../../../../assets/editor/styles/docking.css"),
            include_str!("../../../../assets/editor/styles/startup.css"),
        ),
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    let mut app = consumer(&output, manifest);
    let sheet = load(&mut app, "game://theme.css")?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(sheet), ClassList::new("theme")))
        .id();
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
    for (hovered, pressed, expected) in [
        (false, false, Color::srgb_u8(121, 80, 188)),
        (true, false, Color::srgb_u8(134, 92, 195)),
        (true, true, Color::srgb_u8(96, 57, 147)),
    ] {
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
        assert_eq!(
            app.world()
                .get::<BackgroundColor>(button)
                .ok_or("button fill")?
                .0,
            expected
        );
    }
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(selected)
            .ok_or("selection fill")?
            .0,
        Color::srgb_u8(87, 65, 111)
    );
    assert_eq!(
        app.world()
            .get::<BorderColor>(preview)
            .ok_or("preview border")?
            .top,
        Color::srgb_u8(208, 139, 255)
    );
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(preview)
            .ok_or("preview fill")?
            .0,
        Color::srgba_u8(121, 80, 188, 26)
    );
    assert_eq!(
        app.world().get::<Node>(frame).ok_or("frame node")?.border,
        UiRect::ZERO,
        "Even a transparent border leaves a canvas-colored perimeter stripe"
    );
    Ok(())
}
