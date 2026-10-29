//! Production CSS interaction recovery and usable nested layout, without palette snapshots.
use super::*;
use bevy::input_focus::{FocusCause, InputFocus, InputFocusVisible};

fn fixture() -> Result<(App, Entity)> {
    let tokens = include_str!("../../../../../assets/editor/styles/tokens.css")
        .lines()
        .filter(|line| !line.starts_with("@font-face"))
        .collect::<Vec<_>>()
        .join("\n");
    stylesheet(
        [
            tokens.as_str(),
            include_str!("../../../../../assets/editor/styles/controls.css"),
            include_str!("../../../../../assets/editor/styles/startup.css"),
            include_str!("../../../../../assets/editor/styles/docking.css"),
            include_str!("../../../../../assets/editor/styles/asset-browser.css"),
            include_str!("../../../../../assets/editor/styles/hierarchy.css"),
            include_str!("../../../../../assets/editor/styles/scene-view.css"),
            include_str!("../../../../../assets/editor/styles/properties.css"),
            include_str!("../../../../../assets/editor/styles/inspector.css"),
        ]
        .join("\n"),
    )
}

fn child(app: &mut App, parent: Entity, class: &str) -> Entity {
    app.world_mut()
        .spawn((Node::default(), ClassList::new(class), ChildOf(parent)))
        .id()
}

#[test]
fn restored_shadow_details_become_visible_when_the_header_expands() -> Result<()> {
    let (mut app, root) = fixture()?;
    let row = child(
        &mut app,
        root,
        "property-row property-collection-shadow collapsed",
    );
    let editor = child(&mut app, row, "property-editor");
    frames(&mut app);
    assert_eq!(
        app.world().get::<Node>(editor).ok_or("editor")?.display,
        Display::None
    );
    kedra_ui::docking::class(app.world_mut(), row, "collapsed", false);
    frames(&mut app);
    assert_eq!(
        app.world().get::<Node>(editor).ok_or("editor")?.display,
        Display::Flex
    );
    Ok(())
}

#[test]
fn invalid_and_disabled_feedback_restores_without_restyling_unrelated_text() -> Result<()> {
    let (mut app, root) = fixture()?;
    let field = child(&mut app, root, "field");
    let button = child(&mut app, root, "button primary");
    let label = child(&mut app, button, "text strong");
    let muted = child(&mut app, root, "text muted");
    let icon_button = child(&mut app, root, "button flat browser-icon");
    let icon = child(&mut app, icon_button, "icon");
    for text in [label, muted] {
        app.world_mut()
            .entity_mut(text)
            .insert(TextColor::default());
    }
    app.world_mut()
        .entity_mut(icon)
        .insert(ImageNode::default());
    frames(&mut app);
    let border = app.world().get::<BorderColor>(field).ok_or("border")?.top;
    let fill = app.world().get::<BackgroundColor>(button).ok_or("fill")?.0;
    let text = app.world().get::<TextColor>(label).ok_or("label")?.0;
    let muted_color = app.world().get::<TextColor>(muted).ok_or("muted")?.0;
    let ink = app.world().get::<ImageNode>(icon).ok_or("icon")?.color;
    kedra_ui::docking::class(app.world_mut(), field, "invalid", true);
    for entity in [button, icon_button] {
        app.world_mut()
            .entity_mut(entity)
            .insert(bevy::ui::InteractionDisabled);
    }
    frames(&mut app);
    assert_ne!(
        app.world().get::<BorderColor>(field).ok_or("border")?.top,
        border
    );
    assert_ne!(
        app.world().get::<BackgroundColor>(button).ok_or("fill")?.0,
        fill
    );
    assert_ne!(app.world().get::<TextColor>(label).ok_or("label")?.0, text);
    assert_ne!(app.world().get::<ImageNode>(icon).ok_or("icon")?.color, ink);
    assert_eq!(
        app.world().get::<TextColor>(muted).ok_or("muted")?.0,
        muted_color
    );
    kedra_ui::docking::class(app.world_mut(), field, "invalid", false);
    for entity in [button, icon_button] {
        app.world_mut()
            .entity_mut(entity)
            .remove::<bevy::ui::InteractionDisabled>();
    }
    frames(&mut app);
    assert_eq!(
        app.world().get::<BorderColor>(field).ok_or("border")?.top,
        border
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(button).ok_or("fill")?.0,
        fill
    );
    assert_eq!(app.world().get::<TextColor>(label).ok_or("label")?.0, text);
    assert_eq!(app.world().get::<ImageNode>(icon).ok_or("icon")?.color, ink);
    assert_eq!(
        app.world().get::<TextColor>(muted).ok_or("muted")?.0,
        muted_color
    );
    Ok(())
}

#[test]
fn authored_keyboard_focus_rings_and_blur_restore_the_control() -> Result<()> {
    for class in ["button primary", "field"] {
        let (mut app, root) = fixture()?;
        app.init_resource::<InputFocus>()
            .init_resource::<InputFocusVisible>();
        app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
        let control = child(&mut app, root, class);
        kedra_ui::docking::class(app.world_mut(), control, "authored-focus", true);
        let ring = child(&mut app, control, "control-focus");
        let target = if class == "field" {
            child(&mut app, control, "input")
        } else {
            control
        };
        frames(&mut app);
        let border = app.world().get::<Node>(control).ok_or("control")?.border;
        let outline = app.world().get::<Outline>(control).map(|o| o.width);
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(target, FocusCause::Navigated);
        frames(&mut app);
        let node = app.world().get::<Node>(ring).ok_or("ring")?;
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.position_type, PositionType::Absolute);
        assert!(matches!(node.border.left, Val::Px(width) if width > 0.));
        app.world_mut().resource_mut::<InputFocus>().clear();
        frames(&mut app);
        assert_eq!(
            app.world().get::<Node>(ring).ok_or("ring")?.display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(control).ok_or("control")?.border,
            border
        );
        assert_eq!(
            app.world().get::<Outline>(control).map(|o| o.width),
            outline
        );
    }
    Ok(())
}

#[test]
fn initially_focused_controls_without_authored_rings_have_a_fallback_outline() -> Result<()> {
    let (mut app, root) = fixture()?;
    app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
    for class in ["button", "button flat", "field", "project-row"] {
        let control = child(&mut app, root, class);
        let ring = child(&mut app, control, "control-focus");
        let target = if class == "field" {
            child(&mut app, control, "input")
        } else {
            control
        };
        // Initial hydration is the fallback contract; native focus transitions
        // and authored-ring recovery have separate owners.
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(target, FocusCause::Navigated);
        frames(&mut app);
        assert!(
            matches!(app.world().get::<Outline>(control).ok_or("outline")?.width, Val::Px(width) if width > 0.),
            "Missing fallback for {class}"
        );
        assert_eq!(
            app.world().get::<Node>(ring).ok_or("ring")?.display,
            Display::None
        );
    }
    Ok(())
}

// Exercise Bevy's layout with the loaded production CSS. This is synthetic
// geometry, not text shaping, renderer output or physical input acceptance.
fn layout(app: &mut App, root: Entity, size: UVec2) -> Result<bevy::ui::ui_surface::UiSurface> {
    use bevy::{
        ecs::system::SystemState,
        ui::{LayoutContext, ui_surface::UiSurface},
    };
    let mut surface = UiSurface::default();
    let context = LayoutContext {
        physical_size: size.as_vec2(),
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
    surface.compute_layout(root, size, &mut buffers, &mut bevy::text::FontCx::default());
    Ok(surface)
}

#[test]
fn nested_fields_keep_their_allocated_height_and_groups_do_not_overlap() -> Result<()> {
    let (mut app, root) = fixture()?;
    let stack = child(&mut app, root, "inspector-entries");
    let mut rows = Vec::new();
    let mut fields = Vec::new();
    for count in [4, 3, 1] {
        let section = child(&mut app, stack, "property-section");
        rows.push(section);
        child(&mut app, section, "property-heading");
        let body = child(&mut app, section, "property-body");
        for _ in 0..count {
            let row = child(&mut app, body, "property-row");
            child(&mut app, row, "property-label");
            let editor = child(&mut app, row, "property-editor");
            let field = child(&mut app, editor, "field property-value");
            let input = child(&mut app, field, "input");
            fields.push((field, input));
        }
    }
    frames(&mut app);
    for width in [320, 480] {
        let mut surface = layout(&mut app, stack, UVec2::new(width, 900))?;
        for (field, input) in &fields {
            let Val::Px(requested) = app.world().get::<Node>(*field).ok_or("field")?.height else {
                return Err("Field requires an allocated height".into());
            };
            let field = surface.get_layout(*field, true)?.0;
            let input = surface.get_layout(*input, true)?.0;
            assert!(requested > 0.);
            assert_eq!(
                field.size.height, requested,
                "Nested field shrank at {width}px"
            );
            assert!(
                input.size.height > 0. && input.location.y + input.size.height <= field.size.height
            );
            assert!(field.size.width > 0. && field.size.width <= width as f32);
        }
        for pair in rows.windows(2) {
            let a = surface.get_layout(pair[0], true)?.0;
            let b = surface.get_layout(pair[1], true)?.0;
            assert!(a.location.y + a.size.height < b.location.y);
        }
    }
    Ok(())
}

#[test]
fn focus_ring_tracks_the_outer_control_instead_of_its_content_border() -> Result<()> {
    let (mut app, root) = fixture()?;
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
    for class in ["button quiet", "button raised", "button primary", "field"] {
        let control = child(&mut app, root, class);
        kedra_ui::docking::class(app.world_mut(), control, "authored-focus", true);
        app.world_mut()
            .get_mut::<Node>(control)
            .ok_or("control")?
            .width = px(172);
        let target = if class == "field" {
            child(&mut app, control, "input")
        } else {
            control
        };
        let ring = child(&mut app, control, "control-focus");
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(target, FocusCause::Navigated);
        frames(&mut app);
        let mut surface = layout(&mut app, control, UVec2::new(172, 100))?;
        let outer = surface.get_layout(control, true)?.0;
        let ring = surface.get_layout(ring, true)?.0;
        assert!(ring.location.x < 0. && ring.location.y < 0., "{class}");
        assert_eq!(
            ring.size.width + 2. * ring.location.x,
            outer.size.width,
            "{class}"
        );
        assert_eq!(
            ring.size.height + 2. * ring.location.y,
            outer.size.height,
            "{class}"
        );
    }
    Ok(())
}
