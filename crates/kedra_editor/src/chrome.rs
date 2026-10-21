//! Editor-owned composition over shared UI slots and desktop native operations.
#[cfg(test)]
#[path = "chrome/tests.rs"]
mod tests;
use bevy::prelude::*;
use bevy_flair::style::components::ClassList;
use kedra_desktop::chrome::{ChromeAction, ChromeControl, NativeChromeState};
use kedra_ui::{
    authored::view::UiView,
    window_chrome::{Header, HeaderRegion, RegionRole},
};
use std::collections::BTreeMap;

#[derive(Component)]
pub(crate) struct HeaderOwner(pub Header);
#[derive(Component)]
struct Caption {
    window: Entity,
    frame: Option<Entity>,
    title: Entity,
    maximize: Entity,
    restore: Entity,
}
pub(crate) fn install(app: &mut App) {
    app.add_systems(Update, sync);
}
pub(crate) fn bind(world: &mut World, view: &UiView) -> Result<(), String> {
    let desktop = world.resource::<kedra_desktop::DesktopSurface>();
    let window = desktop.window.ok_or("Caption has no owned window")?;
    bind_window(world, view, window, desktop.frame)
}
pub(crate) fn bind_window(
    world: &mut World,
    view: &UiView,
    window: Entity,
    frame: Option<Entity>,
) -> Result<(), String> {
    let element = |name| view.element(name).map_err(|e| e.to_string());
    let title = element("window.title")?;
    let maximize = element("window.maximize_glyph")?;
    let restore = element("window.restore_glyph")?;
    let controls = [
        (element("window.minimize")?, ChromeAction::Minimize),
        (element("window.maximize")?, ChromeAction::ToggleMaximize),
        (element("window.close")?, ChromeAction::Close),
    ];
    let mut slots = BTreeMap::new();
    for key in ["window.tabs", "window.menu", "window.tools"] {
        if let Ok(entity) = view.element(key) {
            slots.insert(key.into(), entity);
        }
    }
    let header = Header::bind(world, window, view.root, slots)?;
    kedra_desktop::chrome::bind(world, window)?;
    world.entity_mut(view.root).insert((
        HeaderRegion {
            header: view.root,
            role: RegionRole::Drag,
        },
        HeaderOwner(header),
        Caption {
            window,
            frame,
            title,
            maximize,
            restore,
        },
    ));
    for (entity, action) in controls {
        world
            .entity_mut(entity)
            .insert(ChromeControl { window, action });
        match action {
            ChromeAction::Minimize => set_class(world, entity, "window-minimize", true),
            ChromeAction::ToggleMaximize => set_class(world, entity, "window-maximize", true),
            ChromeAction::Close => {}
        }
    }
    for key in [
        "window.tabs",
        "window.menu",
        "window.tools",
        "editor.transport",
    ] {
        if let Ok(entity) = view.element(key) {
            world.entity_mut(entity).insert(HeaderRegion {
                header: view.root,
                role: RegionRole::Client,
            });
        }
    }
    if let Ok(label) = view.element("editor.project") {
        let name = world
            .get_resource::<crate::model::ProjectManager>()
            .and_then(|manager| manager.project())
            .map(|project| project.manifest.name.clone())
            .unwrap_or_else(|| "Project".into());
        if let Some(mut text) = world.get_mut::<Text>(label) {
            text.0 = name;
        }
    }
    for (key, label) in [
        ("editor.play", "Play is not implemented"),
        ("editor.pause", "Play is not implemented"),
        ("editor.stop", "Play is not implemented"),
        ("editor.save", "Source Save is not implemented"),
        ("editor.build", "Project Build is not implemented"),
        ("editor.build-menu", "Project Build is not implemented"),
        ("editor.search", "Global search is not implemented"),
    ] {
        if let Ok(entity) = view.element(key) {
            world.entity_mut(entity).insert((
                bevy::ui::InteractionDisabled,
                kedra_ui::tooltip::Tooltip(label.into()),
            ));
            if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(entity) {
                node.set_label(label);
            }
        }
    }
    crate::workspace::bind_header(world, view);
    crate::updates::bind(world, view)?;
    if cfg!(target_os = "macos")
        && let Ok(native) = view.element("window.native")
    {
        world.entity_mut(native).insert(HeaderRegion {
            header: view.root,
            role: RegionRole::NativeControls,
        });
    }
    Ok(())
}
fn sync(world: &mut World) {
    let captions: Vec<_> = world
        .query::<(Entity, &Caption)>()
        .iter(world)
        .map(|(root, c)| (root, c.window, c.frame, c.title, c.maximize, c.restore))
        .collect();
    for (root, window, frame, title, maximize, restore) in captions {
        let Some(native) = world.get::<Window>(window) else {
            continue;
        };
        let text = native.title.clone();
        let can_minimize = native.enabled_buttons.minimize;
        let can_maximize = native.enabled_buttons.maximize && native.resizable;
        let state = world
            .get::<NativeChromeState>(window)
            .copied()
            .unwrap_or_default();
        if let Some(bounds) = world.get::<ChildOf>(title).map(ChildOf::parent)
            && let Err(error) =
                kedra_ui::window_chrome::set_title(world, root, title, bounds, &text)
        {
            warn!("Caption fitting unavailable: {error}");
        }
        for (entity, class, enabled) in [
            (root, "macos", cfg!(target_os = "macos")),
            (root, "inactive", state.ready && !state.focused),
            (root, "fullscreen", state.fullscreen),
            (root, "no-minimize", !can_minimize),
            (root, "no-maximize", !can_maximize),
            (maximize, "hidden", state.maximized),
            (restore, "hidden", !state.maximized),
        ] {
            set_class(world, entity, class, enabled);
        }
        if let Some(frame) = frame {
            set_class(
                world,
                frame,
                "maximized",
                state.maximized || state.fullscreen,
            );
        }
    }
}
fn set_class(world: &mut World, entity: Entity, name: &'static str, enabled: bool) {
    if let Some(mut classes) = world.get_mut::<ClassList>(entity)
        && classes.contains(name) != enabled
    {
        classes.toggle(name);
    }
}

pub(crate) fn floating_destination(
    world: &mut World,
    window: Entity,
    integrated: bool,
) -> Result<Option<kedra_ui::docking::HeaderDestination>, String> {
    let (root, row, menu) = world
        .query::<(Entity, &HeaderOwner)>()
        .iter(world)
        .find_map(|(root, owner)| {
            (owner.0.window == window).then(|| {
                (
                    root,
                    owner.0.slot("window.tabs"),
                    owner.0.slot("window.menu"),
                )
            })
        })
        .ok_or("Floating window has no shared header binding")?;
    let destination = if integrated {
        Some(kedra_ui::docking::HeaderDestination {
            row: row.ok_or("Floating header lacks its tab slot")?,
            menu: menu.ok_or("Floating header lacks its menu slot")?,
        })
    } else {
        None
    };
    set_class(world, root, "floating-titlebar", integrated);
    if integrated {
        let minimum = if cfg!(target_os = "macos") {
            312.
        } else {
            366.
        };
        if let Some(mut native) = world.get_mut::<Window>(window) {
            native.resize_constraints.min_width = native.resize_constraints.min_width.max(minimum);
        }
        if cfg!(target_os = "macos")
            && let Some(menu) = menu
        {
            let mut order = world
                .get::<Children>(root)
                .map(|c| c.to_vec())
                .unwrap_or_default();
            order.retain(|entity| *entity != menu);
            order.push(menu);
            world.entity_mut(root).replace_children(&order);
        }
    }
    Ok(destination)
}
