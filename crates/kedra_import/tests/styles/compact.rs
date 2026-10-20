//! Imported production styles must agree on flat control geometry.
use super::*;

fn fixture() -> Result<(App, Entity, tempfile::TempDir)> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    let tokens = include_str!("../../../../assets/editor/styles/tokens.css")
        .lines()
        .filter(|line| !line.starts_with("@font-face"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        source.join("compact.css"),
        [
            tokens.as_str(),
            include_str!("../../../../assets/editor/styles/controls.css"),
            include_str!("../../../../assets/editor/styles/startup.css"),
            include_str!("../../../../assets/editor/styles/docking.css"),
            include_str!("../../../../assets/editor/styles/asset-browser.css"),
            include_str!("../../../../assets/editor/styles/hierarchy.css"),
            include_str!("../../../../assets/editor/styles/scene-view.css"),
            include_str!("../../../../assets/editor/styles/properties.css"),
            include_str!("../../../../assets/editor/styles/inspector.css"),
        ]
        .join("\n"),
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    let mut app = consumer(&output, manifest);
    let sheet = load(&mut app, "game://compact.css")?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(sheet), ClassList::new("theme")))
        .id();
    Ok((app, root, temp))
}

fn child(app: &mut App, parent: Entity, class: &str) -> Entity {
    app.world_mut()
        .spawn((Node::default(), ClassList::new(class), ChildOf(parent)))
        .id()
}

#[test]
fn compact_controls_keep_icons_and_chrome_but_remove_raised_depth() -> Result<()> {
    let (mut app, root, _temp) = fixture()?;
    let cases = [
        ("button raised", 22.),
        ("button primary", 22.),
        ("button flat dock-action", 22.),
        ("field", 22.),
        ("hierarchy-row", 22.),
        ("browser-tree-row", 22.),
        ("property-heading button", 24.),
        ("inspector-identity", 40.),
        ("icon", 16.),
        ("titlebar", 32.),
        ("dock-rail", 32.),
        ("dock-tab", 28.),
        ("window-control button", 32.),
        ("browser-entry", 96.),
    ];
    let nodes: Vec<_> = cases
        .iter()
        .map(|(class, _)| child(&mut app, root, class))
        .collect();
    let menu = child(&mut app, root, "dock-menu");
    let item = child(&mut app, menu, "button");
    frames(&mut app);
    for ((class, height), entity) in cases.iter().zip(nodes.iter()) {
        assert_eq!(
            app.world().get::<Node>(*entity).ok_or("node")?.height,
            px(*height),
            "{class}"
        );
    }
    assert_eq!(
        app.world().get::<Node>(item).ok_or("menu row")?.height,
        px(24.)
    );
    for entity in &nodes[..4] {
        assert_eq!(
            app.world()
                .get::<Node>(*entity)
                .ok_or("control")?
                .border_radius,
            BorderRadius::all(px(2))
        );
        assert!(
            app.world()
                .get::<BoxShadow>(*entity)
                .is_none_or(|s| s.0.is_empty())
        );
        assert!(
            app.world()
                .get::<BackgroundGradient>(*entity)
                .is_none_or(|g| g.0.is_empty())
        );
    }
    let border = app
        .world()
        .get::<BorderColor>(nodes[0])
        .ok_or("button border")?;
    assert_eq!(
        border.top, border.bottom,
        "Flat controls must not retain a top-edge highlight"
    );
    assert!(
        app.world()
            .get::<BoxShadow>(menu)
            .is_some_and(|s| !s.0.is_empty()),
        "Popup keeps its external shadow"
    );
    Ok(())
}

#[test]
fn nested_property_surfaces_keep_twenty_two_pixels_including_borders() -> Result<()> {
    use bevy::{
        ecs::system::SystemState,
        ui::{LayoutContext, ui_surface::UiSurface},
    };
    let (mut app, root, _temp) = fixture()?;
    let row = child(&mut app, root, "property-row");
    child(&mut app, row, "property-label");
    let editor = child(&mut app, row, "property-editor");
    let field = child(&mut app, editor, "field property-value");
    let input = child(&mut app, field, "input");
    frames(&mut app);
    for width in [320, 480] {
        let mut surface = UiSurface::default();
        let context = LayoutContext {
            physical_size: Vec2::new(width as f32, 22.),
            ..default()
        };
        let nodes = app
            .world_mut()
            .query::<(Entity, &Node, Option<&Children>)>()
            .iter(app.world())
            .map(|(e, n, c)| (e, n.clone(), c.map(|c| c.to_vec()).unwrap_or_default()))
            .collect::<Vec<_>>();
        for (e, n, _) in &nodes {
            surface.upsert_node(&context, *e, n, None);
        }
        for (e, _, c) in &nodes {
            surface.update_children(*e, c.iter().copied());
        }
        let mut state =
            SystemState::<Query<&mut bevy::text::ComputedTextBlock>>::new(app.world_mut());
        let mut buffers = state.get_mut(app.world_mut())?;
        surface.compute_layout(
            row,
            UVec2::new(width, 22),
            &mut buffers,
            &mut bevy::text::FontCx::default(),
        );
        assert_eq!(
            surface.get_layout(field, true)?.0.size.height,
            22.,
            "Nested field at {width}px"
        );
        assert_eq!(
            surface.get_layout(input, true)?.0.size.height,
            16.,
            "Text must not shrink"
        );
    }
    Ok(())
}

#[test]
fn flat_controls_retain_invalid_and_disabled_feedback() -> Result<()> {
    let (mut app, root, _temp) = fixture()?;
    let invalid = child(&mut app, root, "field invalid");
    let disabled = child(&mut app, root, "button primary");
    app.world_mut()
        .entity_mut(disabled)
        .insert(bevy::ui::InteractionDisabled);
    let label = child(&mut app, disabled, "text");
    app.world_mut()
        .entity_mut(label)
        .insert(TextColor::default());
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<BorderColor>(invalid)
            .ok_or("invalid border")?
            .top,
        Color::srgb_u8(240, 163, 163)
    );
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(disabled)
            .ok_or("disabled fill")?
            .0,
        Color::srgb_u8(48, 48, 48)
    );
    assert_eq!(
        app.world()
            .get::<TextColor>(label)
            .ok_or("disabled text")?
            .0,
        Color::srgb_u8(175, 175, 175)
    );
    Ok(())
}

#[test]
fn grouped_reference_stack_is_394_pixels_without_shrinking_values() -> Result<()> {
    use bevy::{
        ecs::system::SystemState,
        ui::{LayoutContext, ui_surface::UiSurface},
    };
    let (mut app, root, _temp) = fixture()?;
    let stack = child(&mut app, root, "inspector-entries");
    let source = child(&mut app, stack, "text caption");
    app.world_mut().entity_mut(source).insert(Node {
        height: px(16),
        flex_shrink: 0.,
        ..default()
    });
    let mut values = Vec::new();
    // The canonical Window glyph specimen: Image(4 densities+tint), ownership
    // (3 facts+action), Classes(1 value), collapsed Native layout header.
    for (count, trailing) in [(4, true), (3, true), (1, false)] {
        let section = child(&mut app, stack, "property-section");
        child(&mut app, section, "property-heading");
        let group = child(&mut app, section, "property-body");
        for _ in 0..count {
            values.push(child(&mut app, group, "field property-value"));
        }
        if trailing {
            values.push(child(&mut app, section, "field property-value"));
        }
    }
    let last = child(&mut app, stack, "property-heading");
    frames(&mut app);
    for width in [320, 480] {
        let mut surface = UiSurface::default();
        let context = LayoutContext {
            physical_size: Vec2::new(width as f32, 900.),
            ..default()
        };
        let nodes = app
            .world_mut()
            .query::<(Entity, &Node, Option<&Children>)>()
            .iter(app.world())
            .map(|(e, n, c)| (e, n.clone(), c.map(|c| c.to_vec()).unwrap_or_default()))
            .collect::<Vec<_>>();
        for (e, n, _) in &nodes {
            surface.upsert_node(&context, *e, n, None);
        }
        for (e, _, c) in &nodes {
            surface.update_children(*e, c.iter().copied());
        }
        let mut state =
            SystemState::<Query<&mut bevy::text::ComputedTextBlock>>::new(app.world_mut());
        let mut buffers = state.get_mut(app.world_mut())?;
        surface.compute_layout(
            stack,
            UVec2::new(width, 900),
            &mut buffers,
            &mut bevy::text::FontCx::default(),
        );
        let source_top = surface.get_layout(source, true)?.0.location.y;
        assert_eq!(source_top, 8., "Property stack top padding");
        let end = surface.get_layout(last, true)?.0;
        assert_eq!(
            end.location.y + end.size.height - source_top + 16.,
            394.,
            "Reference stack at {width}px"
        );
        for value in &values {
            assert_eq!(surface.get_layout(*value, true)?.0.size.height, 22.);
        }
    }
    Ok(())
}

#[test]
fn nested_groups_have_four_pixel_boundaries_and_two_pixel_related_rows() -> Result<()> {
    use bevy::{
        ecs::system::SystemState,
        ui::{LayoutContext, ui_surface::UiSurface},
    };
    let (mut app, root, _temp) = fixture()?;
    let body = child(&mut app, root, "property-body");
    let a = child(&mut app, body, "field property-value");
    let b = child(&mut app, body, "field property-value");
    let group = child(&mut app, body, "property-section");
    child(&mut app, group, "property-heading");
    let next = child(&mut app, body, "property-section");
    child(&mut app, next, "property-heading");
    let c = child(&mut app, body, "field property-value");
    frames(&mut app);
    let mut surface = UiSurface::default();
    let context = LayoutContext {
        physical_size: Vec2::new(320., 900.),
        ..default()
    };
    let nodes = app
        .world_mut()
        .query::<(Entity, &Node, Option<&Children>)>()
        .iter(app.world())
        .map(|(e, n, c)| (e, n.clone(), c.map(|c| c.to_vec()).unwrap_or_default()))
        .collect::<Vec<_>>();
    for (e, n, _) in &nodes {
        surface.upsert_node(&context, *e, n, None);
    }
    for (e, _, c) in &nodes {
        surface.update_children(*e, c.iter().copied());
    }
    let mut state = SystemState::<Query<&mut bevy::text::ComputedTextBlock>>::new(app.world_mut());
    let mut buffers = state.get_mut(app.world_mut())?;
    surface.compute_layout(
        body,
        UVec2::new(320, 900),
        &mut buffers,
        &mut bevy::text::FontCx::default(),
    );
    for (first, second, gap) in [(a, b, 2.), (b, group, 4.), (group, next, 4.), (next, c, 4.)] {
        let first = surface.get_layout(first, true)?.0;
        let second = surface.get_layout(second, true)?.0;
        assert_eq!(
            second.location.y - first.location.y - first.size.height,
            gap
        );
    }
    Ok(())
}
