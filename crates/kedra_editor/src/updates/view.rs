//! Disposable authored presentation. Hover/focus never owns the update operation.
use super::model::Phase;
use bevy::{
    input_focus::{InputFocus, InputFocusVisible},
    picking::hover::Hovered,
    prelude::*,
};
use kedra_ui::{
    authored::view::UiView,
    window_chrome::{HeaderRegion, RegionRole},
};

#[derive(Component, Clone, Copy)]
pub(super) struct Control {
    popup: Entity,
    status: Entity,
    version: Entity,
    detail: Entity,
    notes: Entity,
    spinner: Entity,
    progress: Entity,
    fill: Entity,
}

pub(super) fn bind(world: &mut World, view: &UiView) -> Result<(), String> {
    let Ok(button) = view.element("editor.update") else {
        return Ok(());
    };
    let element = |id| view.element(id).map_err(|e| e.to_string());
    let control = Control {
        popup: element("update.popover")?,
        status: element("update.status")?,
        version: element("update.version")?,
        detail: element("update.detail")?,
        notes: element("update.notes")?,
        spinner: element("update.spinner")?,
        progress: element("update.progress")?,
        fill: element("update.progress-fill")?,
    };
    world.entity_mut(button).insert((
        control,
        Hovered(false),
        HeaderRegion {
            header: view.root,
            role: RegionRole::Client,
        },
    ));
    world
        .entity_mut(control.popup)
        .insert((bevy::ui::FixedNode, GlobalZIndex(300)));
    world
        .entity_mut(control.notes)
        .insert(TextLayout::linebreak(
            bevy::text::LineBreak::WordOrCharacter,
        ));
    for entity in [
        control.popup,
        control.status,
        control.version,
        control.detail,
        control.notes,
        control.progress,
        control.fill,
    ] {
        world
            .entity_mut(entity)
            .insert(bevy::picking::Pickable::IGNORE);
    }
    Ok(())
}

fn text(world: &mut World, entity: Entity, value: &str) {
    if let Some(mut text) = world.get_mut::<Text>(entity)
        && text.0 != value
    {
        text.0 = value.into();
    }
}

pub(super) fn sync(world: &mut World) {
    let Some(updates) = world.get_resource::<super::Updates>() else {
        return;
    };
    let automatic = updates
        .installation
        .as_ref()
        .is_some_and(|i| i.macos.is_some());
    let preparing = updates.model.applying && automatic;
    let phase = if preparing {
        Phase::Verifying
    } else {
        updates.model.phase
    };
    let busy = matches!(phase, Phase::Downloading | Phase::Verifying);
    let status = match phase {
        Phase::Hidden => "",
        Phase::Available => "Update available",
        Phase::Downloading => "Downloading update",
        Phase::Verifying if preparing => "Preparing update",
        Phase::Verifying => "Verifying update",
        Phase::Ready if automatic => "Ready to install",
        Phase::Ready => "Ready — manual installation",
        Phase::Error => "Download failed",
    };
    let version = updates
        .release
        .as_ref()
        .map(|r| {
            format!(
                "Kedra Engine {} → {}",
                env!("CARGO_PKG_VERSION"),
                r.version()
            )
        })
        .unwrap_or_default();
    let notes = updates
        .release
        .as_ref()
        .map(|r| r.notes().to_owned())
        .unwrap_or_default();
    let total = updates.release.as_ref().map_or(0, |r| r.bytes());
    let percent = if total > 0 {
        (updates.received as f64 * 100. / total as f64).clamp(0., 100.) as f32
    } else {
        0.
    };
    let detail = if !updates.model.error.is_empty() {
        updates.model.error.clone()
    } else if phase == Phase::Downloading {
        let total = updates.release.as_ref().map_or(0, |r| r.bytes());
        if let Some(percent) = updates.received.saturating_mul(100).checked_div(total) {
            format!("Downloading · {percent}%")
        } else {
            "Downloading…".into()
        }
    } else if preparing {
        "Preparing the application replacement…".into()
    } else if phase == Phase::Verifying {
        "Checking update integrity…".into()
    } else if phase == Phase::Ready {
        if automatic {
            "Kedra will restart after preparation. Unsaved changes prevent restart.".into()
        } else {
            updates
                .installation
                .as_ref()
                .map(|i| super::host::instructions(i.kind))
                .unwrap_or_default()
                .into()
        }
    } else {
        String::new()
    };
    let controls: Vec<_> = world
        .query::<(Entity, &Control)>()
        .iter(world)
        .map(|(e, c)| (e, *c))
        .collect();
    for (entity, control) in controls {
        indicator(world, entity, phase);
        let open = phase != Phase::Hidden && popup_requested(world, entity);
        kedra_ui::docking::class(world, control.popup, "hidden", !open);
        kedra_ui::docking::class(world, control.popup, "failed", phase == Phase::Error);
        kedra_ui::docking::class(world, control.detail, "hidden", detail.is_empty());
        kedra_ui::docking::class(
            world,
            control.progress,
            "hidden",
            phase != Phase::Downloading,
        );
        if let Some(mut node) = world.get_mut::<Node>(control.fill) {
            node.width = Val::Percent(percent);
        }
        if let Some(mut spin) =
            world.get_mut::<kedra_ui::authored::components::UiSpin>(control.spinner)
        {
            spin.paused = !busy;
        }
        text(world, control.status, status);
        text(world, control.version, &version);
        text(world, control.detail, &detail);
        text(world, control.notes, &notes);
        if let Some(mut accessibility) = world.get_mut::<bevy::a11y::AccessibilityNode>(entity) {
            accessibility.set_label(match phase {
                Phase::Ready if automatic => "Install update and restart",
                Phase::Ready => "Update — reveal package for manual installation",
                Phase::Error => "Retry update download",
                _ => status,
            });
        }
        if open {
            let rect = kedra_ui::docking::rect(world, entity);
            let target = world
                .get::<ComputedUiRenderTargetInfo>(entity)
                .map(|t| t.logical_size());
            if let (Some(rect), Some(viewport)) = (rect, target) {
                let width = 350_f32.min((viewport.x - 8.).max(1.));
                let height = world
                    .get::<ComputedNode>(control.popup)
                    .map_or(0., |n| n.size().y * n.inverse_scale_factor());
                if let Some(mut node) = world.get_mut::<Node>(control.popup) {
                    node.width = px(width);
                    node.max_width = px(width);
                    node.max_height = px((viewport.y - 8.).max(1.));
                    node.left =
                        px((rect.max.x - width).clamp(4., (viewport.x - width - 4.).max(4.)));
                    node.top = px((rect.max.y + 4.).min((viewport.y - height - 4.).max(4.)));
                }
            }
        }
    }
}

pub(super) fn popup_requested(world: &World, entity: Entity) -> bool {
    world.get::<Hovered>(entity).is_some_and(|h| h.0)
        || (world
            .get_resource::<InputFocus>()
            .is_some_and(|f| f.get() == Some(entity))
            && world
                .get_resource::<InputFocusVisible>()
                .is_some_and(|f| f.0))
}

pub(super) fn indicator(world: &mut World, entity: Entity, phase: Phase) {
    if let Some(mut node) = world.get_mut::<Node>(entity) {
        node.display = if phase == Phase::Hidden {
            Display::None
        } else {
            Display::Flex
        };
        node.width = px(if phase == Phase::Ready { 64 } else { 22 });
        node.height = px(22);
    }
    for (class, enabled) in [
        ("hidden", phase == Phase::Hidden),
        ("ready", phase == Phase::Ready),
        ("primary", phase == Phase::Ready),
        ("flat", phase != Phase::Ready),
        (
            "busy",
            matches!(phase, Phase::Downloading | Phase::Verifying),
        ),
        ("failed", phase == Phase::Error),
    ] {
        kedra_ui::docking::class(world, entity, class, enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_flair::style::components::ClassList;
    #[test]
    fn hidden_reclaims_width_and_ready_is_primary_64_by_22() {
        let mut world = World::new();
        let button = world
            .spawn((Node::default(), ClassList::new("update-control")))
            .id();
        indicator(&mut world, button, Phase::Hidden);
        assert_eq!(
            world.get::<Node>(button).map(|n| n.display),
            Some(Display::None)
        );
        indicator(&mut world, button, Phase::Ready);
        assert_eq!(
            world.get::<Node>(button).map(|n| (n.width, n.height)),
            Some((px(64), px(22)))
        );
        assert!(
            world
                .get::<ClassList>(button)
                .is_some_and(|c| c.contains("primary"))
        );
        indicator(&mut world, button, Phase::Available);
        assert_eq!(world.get::<Node>(button).map(|n| n.width), Some(px(22)));
    }
    #[test]
    fn keyboard_focus_and_hover_open_status_but_pointer_focus_alone_does_not() {
        let mut world = World::new();
        world.init_resource::<InputFocus>();
        world.init_resource::<InputFocusVisible>();
        let button = world.spawn(Hovered(false)).id();
        world
            .resource_mut::<InputFocus>()
            .set(button, bevy::input_focus::FocusCause::Pressed);
        assert!(!popup_requested(&world, button));
        world.resource_mut::<InputFocusVisible>().0 = true;
        assert!(popup_requested(&world, button));
        world.resource_mut::<InputFocus>().clear();
        world.entity_mut(button).insert(Hovered(true));
        assert!(popup_requested(&world, button));
    }
}
