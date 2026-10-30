//! Deterministic production input tests. Measured rectangles are synthetic, not frames.
use super::*;
use crate::workspace::test_support::{box_at, setup, sources, step, visible};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn point(world: &mut World, window: Entity, position: Vec2) {
    *world.resource_mut::<CursorSample>() = CursorSample {
        window: Some(window),
        position,
    };
}

#[test]
fn measurement_inherits_the_committed_stash_stylesheet_not_the_window_root() -> TestResult {
    let (mut app, _, view) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| -> TestResult {
        // The native fixture's window root is unstyled; its descendant host and
        // committed stash own the stylesheet used by caption-bearing dock skins.
        world
            .entity_mut(state.root)
            .remove::<bevy_flair::style::components::Styled>();
        let host = world
            .spawn((
                UiNode::default(),
                bevy_flair::style::components::Styled::new(Handle::default()),
                ChildOf(state.root),
            ))
            .id();
        let host_id = state.host;
        state.surfaces.get_mut(&host_id).ok_or("host")?.root = host;
        state.stash = world.spawn((UiNode::default(), ChildOf(host))).id();
        state.skin = Some(super::super::load_skin(world)?);
        let before = state.layout.snapshot().clone();
        measurement(world, &mut state, &before, Selection::View(view))?;
        let staged = state.measurements.as_ref().ok_or("measurement")?;
        assert_eq!(
            world.get::<ChildOf>(staged.stash).ok_or("parent")?.parent(),
            state.stash
        );
        assert_eq!(state.layout.snapshot(), &before);
        let owned = staged.stash;
        clear_measurement(world, &mut state);
        assert!(!world.entities().contains(owned));
        assert!(world.entities().contains(host));
        Ok(())
    })
}

#[test]
fn clipped_label_cannot_start_drag_outside_visible_rail() -> TestResult {
    let (mut app, a, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let group = state.layout.snapshot().location(a).ok_or("group")?.0;
            let rail = state.renderer.rail(group).ok_or("rail")?;
            let root = state.renderer.group(group).ok_or("group root")?;
            box_at(
                world,
                state.root,
                Vec2::new(400., 200.),
                Vec2::new(800., 400.),
            );
            box_at(world, root, Vec2::new(150., 200.), Vec2::new(300., 400.));
            box_at(world, rail, Vec2::new(100., 16.), Vec2::new(200., 32.));
            box_at(
                world,
                state.renderer.label(a).ok_or("a")?,
                Vec2::new(50., 16.),
                Vec2::new(80., 28.),
            );
            box_at(
                world,
                state.renderer.label(b).ok_or("b")?,
                Vec2::new(250., 16.),
                Vec2::new(80., 28.),
            );
            point(world, state.window, Vec2::new(220., 16.));
            visible(world);
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            tick(world, &mut state)?;
            assert!(
                state.gesture.is_none(),
                "Clipped label captured a press outside its rail"
            );
            Ok(())
        },
    )?;
    Ok(())
}

fn measured(world: &mut World) -> Result<NodeId, String> {
    world.resource_scope(|world, state: Mut<EditorWorkspace>| {
        let group = state
            .layout
            .snapshot()
            .nodes
            .iter()
            .find_map(|(id, node)| matches!(node, Node::Group { .. }).then_some(*id))
            .ok_or("group")?;
        box_at(
            world,
            state.root,
            Vec2::new(400., 300.),
            Vec2::new(800., 600.),
        );
        box_at(
            world,
            state.renderer.group(group).ok_or("root")?,
            Vec2::new(400., 300.),
            Vec2::new(800., 600.),
        );
        box_at(
            world,
            state.renderer.rail(group).ok_or("rail")?,
            Vec2::new(400., 16.),
            Vec2::new(800., 32.),
        );
        if let Node::Group { tabs, .. } = &state.layout.snapshot().nodes[&group] {
            for (index, view) in tabs.iter().enumerate() {
                let x = index as f32 * 104.;
                box_at(
                    world,
                    state.renderer.tab(*view).ok_or("tab")?,
                    Vec2::new(x + 50., 16.),
                    Vec2::new(100., 28.),
                );
                box_at(
                    world,
                    state.renderer.label(*view).ok_or("label")?,
                    Vec2::new(x + 40., 16.),
                    Vec2::new(80., 28.),
                );
            }
        }
        visible(world);
        Ok(group)
    })
}

#[test]
fn pointer_click_and_reorder_publish_only_on_release_and_keep_content() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    let content = world.spawn(Text::new("retained draft")).id();
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            state.renderer.register(a, content)?;
            redraw(world, &mut state)
        },
    )?;
    let group = measured(world)?;
    step(world, Vec2::new(40., 16.), Some(true));
    assert!(
        matches!(world.resource::<EditorWorkspace>().gesture, Some(Gesture::Pending { view, .. }) if view == a)
    );
    step(world, Vec2::new(40., 16.), Some(false));
    assert!(
        matches!(&world.resource::<EditorWorkspace>().layout.snapshot().nodes[&group], Node::Group { active, .. } if *active == a)
    );
    step(world, Vec2::new(40., 16.), Some(true));
    step(world, Vec2::new(300., 16.), None);
    let before = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .clone();
    step(world, Vec2::new(300., 16.), None);
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot(),
        &before
    );
    assert!(world.resource::<EditorWorkspace>().feedback.is_some());
    step(world, Vec2::new(300., 16.), Some(false));
    let state = world.resource::<EditorWorkspace>();
    assert!(
        matches!(&state.layout.snapshot().nodes[&group], Node::Group { tabs, active } if tabs == &[b, a] && *active == a)
    );
    assert!(state.gesture.is_none() && state.feedback.is_none() && state.measurements.is_none());
    assert!(state.preferences.dirty);
    assert_eq!(state.renderer.content(a), Some(content));
    assert_eq!(
        world.get::<Text>(content).map(|t| t.0.as_str()),
        Some("retained draft")
    );
    Ok(())
}

#[test]
fn structural_drag_needs_current_measurements_then_can_merge_back() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    let group = measured(world)?;
    let before = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .clone();
    for publish in [false, true] {
        step(world, Vec2::new(40., 16.), Some(true));
        step(world, Vec2::new(795., 300.), None);
        step(world, Vec2::new(795., 300.), None);
        assert_eq!(
            world.resource::<EditorWorkspace>().layout.snapshot(),
            &before
        );
        if world.resource::<EditorWorkspace>().measurements.is_none() {
            return Err("No structural measurement was created".into());
        }
        // These are measured test rectangles, not fabricated ready/frame counts.
        if !publish {
            step(world, Vec2::new(795., 300.), Some(false));
            assert_eq!(
                world.resource::<EditorWorkspace>().layout.snapshot(),
                &before
            );
            continue;
        }
        world.resource_scope(|world, state: Mut<EditorWorkspace>| -> Result<(), String> {
            let m = state.measurements.as_ref().ok_or("measurement")?;
            box_at(
                world,
                m.projection.group(m.target).ok_or("measured group")?,
                Vec2::new(600., 300.),
                Vec2::new(400., 600.),
            );
            Ok(())
        })?;
        for _ in 0..3 {
            step(world, Vec2::new(795., 300.), None);
        }
        step(world, Vec2::new(795., 300.), Some(false));
    }
    let moved = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .location(a)
        .ok_or("a moved")?
        .0;
    assert_ne!(moved, group);
    assert_eq!(
        world
            .resource::<EditorWorkspace>()
            .layout
            .snapshot()
            .location(b)
            .ok_or("b")?
            .0,
        group
    );
    // Re-measure the new committed geometry before a rail merge.
    world.resource_scope(|world, state: Mut<EditorWorkspace>| -> Result<(), String> {
        for (g, view, x) in [(group, b, 0.), (moved, a, 400.)] {
            box_at(
                world,
                state.renderer.group(g).ok_or("group")?,
                Vec2::new(x + 200., 300.),
                Vec2::new(400., 600.),
            );
            box_at(
                world,
                state.renderer.rail(g).ok_or("rail")?,
                Vec2::new(x + 200., 16.),
                Vec2::new(400., 32.),
            );
            box_at(
                world,
                state.renderer.tab(view).ok_or("tab")?,
                Vec2::new(x + 50., 16.),
                Vec2::new(100., 28.),
            );
            box_at(
                world,
                state.renderer.label(view).ok_or("label")?,
                Vec2::new(x + 40., 16.),
                Vec2::new(80., 28.),
            );
        }
        visible(world);
        Ok(())
    })?;
    step(world, Vec2::new(440., 16.), Some(true));
    step(world, Vec2::new(200., 16.), None);
    step(world, Vec2::new(200., 16.), None);
    step(world, Vec2::new(200., 16.), Some(false));
    let state = world.resource::<EditorWorkspace>();
    assert_eq!(
        state.layout.snapshot().location(a).map(|p| p.0),
        Some(group)
    );
    assert_eq!(state.layout.snapshot().nodes.len(), 1);
    Ok(())
}

#[test]
fn shift_drag_escape_and_focus_loss_cancel_without_publication() -> TestResult {
    let (mut app, _, _) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    let group = measured(world)?;
    let before = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .clone();
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    step(world, Vec2::new(40., 16.), Some(true));
    assert!(
        matches!(world.resource::<EditorWorkspace>().gesture, Some(Gesture::Pending { selection: Selection::Group(g), .. }) if g == group)
    );
    step(world, Vec2::new(795., 300.), None);
    step(world, Vec2::new(795., 300.), None);
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    step(world, Vec2::new(795., 300.), None);
    assert!(world.resource::<EditorWorkspace>().gesture.is_none());
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot(),
        &before
    );
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    step(world, Vec2::new(40., 16.), Some(false));
    step(world, Vec2::new(40., 16.), Some(true));
    let window = world.resource::<EditorWorkspace>().window;
    world.get_mut::<Window>(window).ok_or("window")?.focused = false;
    step(world, Vec2::new(50., 16.), None);
    assert!(world.resource::<EditorWorkspace>().gesture.is_none());
    // Regaining focus while the button remains down must not resurrect a
    // cancelled drag; another press is required to begin a new gesture.
    assert!(
        world
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );
    world.get_mut::<Window>(window).ok_or("window")?.focused = true;
    step(world, Vec2::new(795., 300.), None);
    assert!(world.resource::<EditorWorkspace>().gesture.is_none());
    step(world, Vec2::new(795., 300.), Some(false));
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot(),
        &before
    );
    Ok(())
}

#[test]
fn partial_label_clip_and_dpr_are_respected_without_disabling_visible_part() -> TestResult {
    let (mut app, a, _) = setup()?;
    let world = app.world_mut();
    measured(world)?;
    let label = world
        .resource::<EditorWorkspace>()
        .renderer
        .label(a)
        .ok_or("label")?;
    // At DPR 2 the logical label 0..80 is clipped to logical x=20..60.
    world
        .get_mut::<ComputedNode>(label)
        .ok_or("node")?
        .inverse_scale_factor = 0.5;
    world.get_mut::<ComputedNode>(label).ok_or("node")?.size = Vec2::new(160., 56.);
    world.entity_mut(label).insert(UiGlobalTransform::from(
        bevy::math::Affine2::from_translation(Vec2::new(80., 32.)),
    ));
    let mut clip = CalculatedClip::default();
    clip.push_rect(
        Rect::from_corners(Vec2::new(40., 0.), Vec2::new(120., 64.)),
        bevy::math::Affine2::IDENTITY,
    );
    world.entity_mut(label).insert(clip);
    step(world, Vec2::new(10., 16.), Some(true));
    assert!(world.resource::<EditorWorkspace>().gesture.is_none());
    step(world, Vec2::new(10., 16.), Some(false));
    step(world, Vec2::new(40., 16.), Some(true));
    assert!(
        matches!(world.resource::<EditorWorkspace>().gesture, Some(Gesture::Pending { view, .. }) if view == a)
    );
    Ok(())
}

#[test]
fn maximized_and_foreign_projection_labels_cannot_capture_stale_geometry() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    let group = measured(world)?;
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            apply(
                world,
                &mut state,
                Command::Move {
                    selection: Selection::View(b),
                    destination: Destination::Edge {
                        target: group,
                        edge: kedra_docking::model::Edge::Right,
                        fraction: 0.5,
                    },
                },
            )?;
            let other = state.layout.snapshot().location(b).ok_or("other")?.0;
            let host = state.host;
            apply(
                world,
                &mut state,
                Command::Maximize {
                    host,
                    group: Some(other),
                },
            )?;
            // Stale rectangles stay deliberately positive, including a visible-looking ancestor.
            for entity in [
                state.renderer.group(group).ok_or("group")?,
                state.renderer.rail(group).ok_or("rail")?,
                state.renderer.label(a).ok_or("label")?,
            ] {
                box_at(world, entity, Vec2::new(100., 16.), Vec2::new(200., 32.));
            }
            visible(world);
            let label = state.renderer.label(a).ok_or("label")?;
            let binding = *world.get::<TabLabel>(label).ok_or("binding")?;
            assert!(!super::super::hit::label(
                world,
                &state,
                label,
                &binding,
                Vec2::new(40., 16.)
            ));
            let clone = world
                .spawn((UiNode::default(), binding, InheritedVisibility::VISIBLE))
                .id();
            box_at(world, clone, Vec2::new(100., 16.), Vec2::new(200., 32.));
            assert!(!super::super::hit::label(
                world,
                &state,
                clone,
                &binding,
                Vec2::new(40., 16.)
            ));
            Ok(())
        },
    )?;
    step(world, Vec2::new(40., 16.), Some(true));
    assert!(world.resource::<EditorWorkspace>().gesture.is_none());
    Ok(())
}

#[test]
fn resize_pointer_preview_escape_release_keyboard_and_preferred_reset() -> TestResult {
    let (mut app, _, b) = setup()?;
    let world = app.world_mut();
    let group = measured(world)?;
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        apply(
            world,
            &mut state,
            Command::Move {
                selection: Selection::View(b),
                destination: Destination::Edge {
                    target: group,
                    edge: kedra_docking::model::Edge::Right,
                    fraction: 0.5,
                },
            },
        )
    })?;
    let split = world.resource::<EditorWorkspace>().layout.snapshot().hosts
        [&world.resource::<EditorWorkspace>().host]
        .root
        .ok_or("split")?;
    let root = world
        .resource::<EditorWorkspace>()
        .renderer
        .group(split)
        .ok_or("split root")?;
    let divider = world
        .query::<(Entity, &Divider)>()
        .iter(world)
        .find_map(|(e, d)| (d.split == split).then_some(e))
        .ok_or("divider")?;
    box_at(world, root, Vec2::new(400., 300.), Vec2::new(800., 600.));
    box_at(world, divider, Vec2::new(400., 300.), Vec2::new(4., 600.));
    visible(world);
    let before = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .clone();
    // A native scrollbar beside the divider owns presses in its visible bounds,
    // even where the divider's 3px hit slop overlaps it.
    let scrollbar = world
        .spawn((
            bevy::ui::Node::default(),
            bevy::ui_widgets::Scrollbar::new(
                root,
                bevy::ui_widgets::ControlOrientation::Vertical,
                24.,
            ),
            ChildOf(root),
        ))
        .id();
    box_at(world, scrollbar, Vec2::new(396., 300.), Vec2::new(4., 100.));
    visible(world);
    step(world, Vec2::new(397., 300.), Some(true));
    assert!(world.resource::<EditorWorkspace>().gesture.is_none());
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot(),
        &before
    );
    step(world, Vec2::new(397., 300.), Some(false));
    world.despawn(scrollbar);
    step(world, Vec2::new(400., 300.), Some(true));
    assert!(matches!(
        world.resource::<EditorWorkspace>().gesture,
        Some(Gesture::Resize { .. })
    ));
    step(world, Vec2::new(600., 300.), None);
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot(),
        &before
    );
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    step(world, Vec2::new(600., 300.), None);
    assert!(world.resource::<EditorWorkspace>().gesture.is_none());
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot(),
        &before
    );
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    step(world, Vec2::new(600., 300.), Some(false));
    // Retry immediately, inside the double-click interval: Escape must not
    // turn the new drag into a preferred-ratio reset.
    step(world, Vec2::new(400., 300.), Some(true));
    assert!(matches!(
        world.resource::<EditorWorkspace>().gesture,
        Some(Gesture::Resize { .. })
    ));
    step(world, Vec2::new(600., 300.), Some(false));
    assert!(
        matches!(world.resource::<EditorWorkspace>().layout.snapshot().nodes[&split], Node::Split { fraction, .. } if fraction > 0.7)
    );
    // A second press within the double-click interval restores the preferred ratio.
    step(world, Vec2::new(400., 300.), Some(true));
    assert!(
        matches!(world.resource::<EditorWorkspace>().layout.snapshot().nodes[&split], Node::Split { fraction, .. } if fraction == 0.5)
    );
    step(world, Vec2::new(400., 300.), Some(false));
    world
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(divider, bevy::input_focus::FocusCause::Navigated);
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowRight);
    step(world, Vec2::new(400., 300.), None);
    assert!(
        matches!(world.resource::<EditorWorkspace>().layout.snapshot().nodes[&split], Node::Split { fraction, .. } if fraction > 0.5)
    );
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    dispatch(world, Some(Control::ResetSplit(split)), None, divider);
    assert!(
        matches!(world.resource::<EditorWorkspace>().layout.snapshot().nodes[&split], Node::Split { fraction, .. } if fraction == 0.5)
    );
    Ok(())
}

#[test]
fn menus_maximize_restore_hide_show_and_tab_keyboard_use_product_dispatch() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    let group = measured(world)?;
    let full_title = "Panel with a very long title that does not fit its tab";
    world
        .resource_mut::<EditorWorkspace>()
        .titles
        .insert(b, full_title.into());
    let anchor = world
        .resource::<EditorWorkspace>()
        .renderer
        .label(a)
        .ok_or("anchor")?;
    dispatch(world, Some(Control::Menu(group)), None, anchor);
    assert_eq!(world.resource::<EditorWorkspace>().menu.len(), 1);
    assert!(world.query::<&Action>().iter(world).any(
        |action| matches!(action, Action::Submenu(entries) if entries.iter().any(|entry|
            matches!(entry, kedra_ui::popup::Entry::Command { label, command: Action::Select(view) }
                if label == full_title && *view == b)))
    ));
    let (branch, action) = world
        .query::<(Entity, &Action)>()
        .iter(world)
        .find(|(_, action)| {
            matches!(action, Action::Submenu(entries) if entries.iter().any(|entry|
            matches!(entry, kedra_ui::popup::Entry::Command { command: Action::Maximize(_), .. })))
        })
        .map(|(entity, action)| (entity, action.clone()))
        .ok_or("Layout submenu missing")?;
    dispatch(world, None, Some(action), branch);
    assert!(
        world
            .query::<&Action>()
            .iter(world)
            .any(|a| matches!(a, Action::FloatView(_)))
    );
    dispatch(world, None, Some(Action::Maximize(group)), anchor);
    let host = world.resource::<EditorWorkspace>().host;
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot().hosts[&host].maximized,
        Some(group)
    );
    dispatch(world, None, Some(Action::Maximize(group)), anchor);
    assert_eq!(
        world.resource::<EditorWorkspace>().layout.snapshot().hosts[&host].maximized,
        None
    );
    dispatch(world, Some(Control::Close(a)), None, anchor);
    assert!(
        world
            .resource::<EditorWorkspace>()
            .layout
            .snapshot()
            .hidden
            .contains_key(&a)
    );
    dispatch(world, None, Some(Action::Panels), anchor);
    let (branch, windows) = world
        .query::<(Entity, &Action)>()
        .iter(world)
        .find_map(|(entity, action)| {
            if let Action::Submenu(entries) = action {
                Some((entity, entries.clone()))
            } else {
                None
            }
        })
        .ok_or("Windows submenu missing")?;
    dispatch(world, None, Some(Action::Submenu(windows)), branch);
    assert!(
        world
            .query::<&Action>()
            .iter(world)
            .any(|action| matches!(action, Action::Show(v) if *v == a))
    );
    dispatch(world, None, Some(Action::Show(a)), anchor);
    assert!(
        !world
            .resource::<EditorWorkspace>()
            .layout
            .snapshot()
            .hidden
            .contains_key(&a)
    );
    let label = world
        .resource::<EditorWorkspace>()
        .renderer
        .label(a)
        .ok_or("label")?;
    world
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(label, bevy::input_focus::FocusCause::Navigated);
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::End);
    step(world, Vec2::new(40., 16.), None);
    assert!(
        matches!(&world.resource::<EditorWorkspace>().layout.snapshot().nodes[&group], Node::Group { active, .. } if *active == b)
    );
    dispatch(world, None, Some(Action::Hide(group)), label);
    assert_eq!(
        world
            .resource::<EditorWorkspace>()
            .layout
            .snapshot()
            .hidden
            .len(),
        2
    );
    dispatch(world, None, Some(Action::Show(a)), label);
    assert!(
        world
            .resource::<EditorWorkspace>()
            .layout
            .snapshot()
            .location(a)
            .is_some()
    );
    Ok(())
}

#[test]
fn stale_drag_release_cannot_publish_over_intervening_activation() -> TestResult {
    let (mut app, a, _) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    measured(world)?;
    step(world, Vec2::new(40., 16.), Some(true));
    step(world, Vec2::new(300., 16.), None);
    step(world, Vec2::new(300., 16.), None);
    world
        .resource_mut::<EditorWorkspace>()
        .layout
        .execute(Command::Activate(a))?;
    let current = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .clone();
    step(world, Vec2::new(300., 16.), Some(false));
    let state = world.resource::<EditorWorkspace>();
    assert_eq!(state.layout.snapshot(), &current);
    assert!(state.gesture.is_none() && state.feedback.is_none() && state.measurements.is_none());
    Ok(())
}

#[test]
fn hidden_disabled_and_stale_projection_origins_are_not_eligible() -> TestResult {
    let (mut app, a, _) = setup()?;
    let world = app.world_mut();
    measured(world)?;
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let entity = state.renderer.label(a).ok_or("label")?;
            let binding = *world.get::<TabLabel>(entity).ok_or("binding")?;
            let point = Vec2::new(40., 16.);
            assert!(super::super::hit::label(
                world, &state, entity, &binding, point
            ));
            world.entity_mut(entity).insert(Visibility::Hidden);
            assert!(!super::super::hit::label(
                world, &state, entity, &binding, point
            ));
            world.entity_mut(entity).insert(Visibility::Inherited);
            world
                .entity_mut(entity)
                .insert(CalculatedClip::FullyClipped);
            assert!(!super::super::hit::label(
                world, &state, entity, &binding, point
            ));
            world.entity_mut(entity).remove::<CalculatedClip>();
            let parent = world.get::<ChildOf>(entity).ok_or("parent")?.parent();
            world
                .entity_mut(parent)
                .insert(bevy::ui::InteractionDisabled);
            assert!(!super::super::hit::label(
                world, &state, entity, &binding, point
            ));
            world
                .entity_mut(parent)
                .remove::<bevy::ui::InteractionDisabled>();
            state.rendered = None;
            assert!(!super::super::hit::label(
                world, &state, entity, &binding, point
            ));
            Ok(())
        },
    )?;
    Ok(())
}
