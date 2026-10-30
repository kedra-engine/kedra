//! Editor-owned production styles and composition through installed CSS; CPU-only.
use bevy::{asset::AssetPlugin, prelude::*};
use bevy_flair::style::{
    StyleSheet,
    components::{ClassList, Styled},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "styles/asset_tiles.rs"]
mod asset_tiles;
#[path = "styles/compact.rs"]
mod compact;
#[path = "styles/inspector.rs"]
mod inspector;
#[path = "styles/startup.rs"]
mod startup;

fn load(app: &mut App, path: &str) -> Result<Handle<StyleSheet>> {
    let handle = app.world().resource::<AssetServer>().load(path.to_owned());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let server = app.world().resource::<AssetServer>();
        if let Some(bevy::asset::LoadState::Failed(error)) = server.get_load_state(handle.id()) {
            return Err(error.to_string().into());
        }
        if server.is_loaded_with_dependencies(handle.id()) {
            break;
        }
        if Instant::now() > deadline {
            return Err("stylesheet load timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(handle)
}
fn frames(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

// Style behavior uses real CSS loading without rebuilding a delivery per assertion.
// The installed-product pipeline remains covered by workbench_styles below.
fn stylesheet(source: String) -> Result<(App, Entity)> {
    let memory = bevy::asset::io::memory::Dir::default();
    memory.insert_asset_text(Path::new("contract.css"), &source);
    let mut app = App::new();
    app.register_asset_source(
        "memory",
        bevy::asset::io::AssetSourceBuilder::new(move || {
            Box::new(bevy::asset::io::memory::MemoryAssetReader {
                root: memory.clone(),
            })
        }),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: bevy::asset::AssetMetaCheck::Never,
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
        kedra_ui::styles::StylesPlugin,
        bevy::input_focus::InputFocusPlugin,
    ));
    app.finish();
    app.cleanup();
    let sheet = load(&mut app, "memory://contract.css")?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(sheet), ClassList::new("theme")))
        .id();
    Ok((app, root))
}

#[test]
fn scene_tool_strip_keeps_square_targets_and_restores_after_hidden_state() -> Result<()> {
    let (mut app, root) = stylesheet(format!(
        "{}\n{}\n{}\n.theme {{ --control-height:22px; --space:4px; }}",
        include_str!("../../../../assets/editor/styles/controls.css"),
        include_str!("../../../../assets/editor/styles/startup.css"),
        include_str!("../../../../assets/editor/styles/scene-view.css")
    ))?;
    kedra_ui::docking::class(app.world_mut(), root, "scene-view", true);
    let toolbar = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("scene-navigation-strip"),
            ChildOf(root),
        ))
        .id();
    let projection = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("button flat"),
            ChildOf(toolbar),
        ))
        .id();
    let menu = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("button flat"),
            ChildOf(toolbar),
        ))
        .id();
    let navigation = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("scene-navigation hidden"),
            ChildOf(root),
        ))
        .id();
    let tools = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("scene-transform-tools"),
            ChildOf(navigation),
        ))
        .id();
    frames(&mut app);
    let node = app.world().get::<Node>(toolbar).ok_or("Toolbar missing")?;
    assert_eq!(
        node.flex_wrap,
        FlexWrap::NoWrap,
        "The selected design keeps commands on one row, using View in narrow docks"
    );
    let node = app
        .world()
        .get::<Node>(projection)
        .ok_or("Projection missing")?;
    assert_eq!(
        node.width, node.height,
        "Projection control retains a square target"
    );
    assert_eq!(node.flex_shrink, 0.);
    let document = kedra_scene::document::PrefabDocument::parse(include_str!(
        "../../../../assets/editor/prefabs/source-document.prefab.ron"
    ))?;
    let mut identities = std::collections::BTreeMap::new();
    for node in &document.nodes {
        if let Some(identity) = node.components.get("ui.id") {
            let (identity,): (String,) = ron::from_str(identity.value.get_ron())?;
            identities.insert(identity, node);
        }
    }
    let viewport = identities.get("viewport").ok_or("Viewport missing")?;
    let diagnostic = identities.get("information").ok_or("Diagnostic missing")?;
    assert_eq!(
        diagnostic.parent.as_ref().map(|p| p.node),
        Some(viewport.id),
        "Diagnostics belong to the canvas, not a window-relative top offset"
    );
    for hidden in [true, false, true, false] {
        for target in [navigation, menu, tools] {
            kedra_ui::docking::class(app.world_mut(), target, "hidden", hidden);
        }
        frames(&mut app);
        for target in [navigation, menu, tools] {
            assert_eq!(
                app.world()
                    .get::<Node>(target)
                    .ok_or("Scene control missing")?
                    .display,
                if hidden { Display::None } else { Display::Flex }
            );
        }
    }
    Ok(())
}

#[test]
fn native_dock_label_reserves_close_space_without_hover_reflow() -> Result<()> {
    let (mut app, root) = stylesheet(format!(
        "{}\n.theme {{--canvas:#242424;--panel:#303030;--hover:#626262;--border:#1c1c1c;--muted:#afafaf;--focus:#b1d2ff;}}",
        include_str!("../../../../assets/editor/styles/docking.css")
    ))?;
    let tab = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("dock-tab"),
            bevy::picking::hover::Hovered(false),
            ChildOf(root),
        ))
        .id();
    let label = app
        .world_mut()
        .spawn((Node::default(), ClassList::new("dock-label"), ChildOf(tab)))
        .id();
    let close = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("dock-tab-close"),
            ChildOf(tab),
        ))
        .id();
    frames(&mut app);
    let idle = app
        .world()
        .get::<Node>(label)
        .ok_or("Label absent")?
        .clone();
    let Val::Px(reserved) = idle.padding.right else {
        return Err("Close slot requires reserved space".into());
    };
    let Val::Px(close_width) = app.world().get::<Node>(close).ok_or("Close absent")?.width else {
        return Err("Close target requires width".into());
    };
    assert!(
        close_width > 0. && reserved >= close_width,
        "The close slot must fit before hover"
    );
    assert_eq!(
        app.world().get::<Visibility>(close),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .entity_mut(tab)
        .insert(bevy::picking::hover::Hovered(true));
    frames(&mut app);
    let hovered = app.world().get::<Node>(label).ok_or("Label absent")?;
    assert_eq!(
        idle.padding, hovered.padding,
        "Hover must not change intrinsic label width"
    );
    assert_ne!(
        app.world().get::<Visibility>(close),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .entity_mut(tab)
        .insert(bevy::picking::hover::Hovered(false));
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<Node>(label)
            .ok_or("Label absent")?
            .padding,
        idle.padding
    );
    Ok(())
}

#[test]
fn workbench_styles_are_verified_assets_not_an_editor_only_loader() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor");
    let manifest = kedra_import::build(
        &source,
        &temp.path().join("installed"),
        "editor-desktop",
        "editor",
        |_| {},
    )?;
    let mut styles: Vec<_> = manifest
        .assets
        .iter()
        .filter(|asset| asset.path.ends_with(".css"))
        .map(|asset| asset.path.as_str())
        .collect();
    styles.sort_unstable();
    assert_eq!(
        styles,
        [
            "fixtures/docking.css",
            "fixtures/inspector.css",
            "styles/asset-browser.css",
            "styles/controls.css",
            "styles/docking.css",
            "styles/hierarchy.css",
            "styles/inspector.css",
            "styles/properties.css",
            "styles/scene-view.css",
            "styles/startup.css",
            "styles/tokens.css",
            "styles/updates.css",
            "styles/workbench.css",
        ]
    );
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "editor",
        temp.path()
            .join("installed")
            .to_str()
            .ok_or("non-Unicode fixture")?
            .into(),
        manifest,
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        kedra_ui::styles::StylesPlugin,
    ));
    use bevy::asset::AssetApp;
    app.init_asset::<Font>()
        .init_asset_loader::<bevy::text::FontLoader>();
    app.finish();
    app.cleanup();
    let sheet = load(&mut app, "editor://styles/workbench.css")?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(sheet), ClassList::new("theme")))
        .id();
    let path = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            ClassList::new("text mono"),
            TextFont::default(),
        ))
        .id();
    let tab = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            ClassList::new("dock-tab active"),
        ))
        .id();
    let action = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            ClassList::new("button flat dock-action"),
        ))
        .id();
    frames(&mut app);
    for (entity, width) in [(tab, 1.), (action, 0.)] {
        assert!(
            app.world()
                .get::<BackgroundGradient>(entity)
                .is_none_or(|gradient| gradient.0.is_empty()),
            "A fill gradient must not overpaint the chrome rim"
        );
        assert_eq!(
            app.world()
                .get::<Node>(entity)
                .ok_or("Chrome node missing")?
                .border
                .top,
            Val::Px(width)
        );
    }
    assert!(
        app.world()
            .get::<BoxShadow>(action)
            .is_none_or(|shadow| shadow.0.is_empty()),
        "Flat actions must not retain a bevel/drop shadow"
    );
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(action)
            .ok_or("Flat background missing")?
            .0
            .alpha(),
        0.
    );
    kedra_ui::docking::class(app.world_mut(), action, "selected", true);
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(action)
            .ok_or("Selected background missing")?
            .0
            .to_srgba()
            .to_u8_array(),
        [87, 65, 111, 255]
    );
    kedra_ui::docking::class(app.world_mut(), action, "selected", false);
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(action)
            .ok_or("Flat background missing after deselection")?
            .0
            .alpha(),
        0.
    );
    kedra_ui::docking::class(app.world_mut(), tab, "dragging", true);
    frames(&mut app);
    assert!(
        app.world()
            .get::<BackgroundGradient>(tab)
            .is_none_or(|gradient| gradient.0.is_empty()),
        "Dragged source must not retain a bright inset"
    );
    kedra_ui::docking::class(app.world_mut(), tab, "dragging", false);
    kedra_ui::docking::class(app.world_mut(), tab, "active", false);
    frames(&mut app);
    assert!(
        app.world()
            .get::<BackgroundGradient>(tab)
            .is_none_or(|gradient| gradient.0.is_empty()),
        "Inactive tabs must not retain an active inset"
    );
    let bevy::text::FontSource::Handle(font) = &app
        .world()
        .get::<TextFont>(path)
        .ok_or("path font missing")?
        .font
    else {
        return Err(
            "path typography resolved a generic/system font instead of its delivered asset".into(),
        );
    };
    assert_eq!(
        font.path().ok_or("font has no delivered path")?,
        &bevy::asset::AssetPath::from("editor://fonts/JetBrainsMono-Regular.ttf")
    );
    for (parent_classes, text_classes, expected_weight, expected_source) in [
        ("recent-header", "text strong", 500, "Inter-Medium.ttf"),
        ("destination", "text strong", 400, "Inter-Regular.ttf"),
        (
            "loading-body",
            "text strong startup-phase",
            500,
            "Inter-Medium.ttf",
        ),
        ("empty-projects", "text title", 500, "Inter-Medium.ttf"),
        ("button quiet", "text strong", 500, "Inter-Medium.ttf"),
        ("button primary", "text strong", 500, "Inter-Medium.ttf"),
    ] {
        let parent = app
            .world_mut()
            .spawn((
                Node::default(),
                ChildOf(root),
                ClassList::new(parent_classes),
            ))
            .id();
        let label = app
            .world_mut()
            .spawn((
                Node::default(),
                ChildOf(parent),
                ClassList::new(text_classes),
                TextFont::default(),
            ))
            .id();
        frames(&mut app);
        let typography = app
            .world()
            .get::<TextFont>(label)
            .ok_or("label typography missing")?;
        assert_eq!(typography.weight.0, expected_weight, "{parent_classes}");
        let bevy::text::FontSource::Handle(font) = &typography.font else {
            return Err("label resolved a system font".into());
        };
        assert_eq!(
            font.path().ok_or("label font path missing")?,
            &bevy::asset::AssetPath::from(format!("editor://fonts/{expected_source}"))
        );
    }
    let glyph = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            ClassList::new("icon hidden"),
        ))
        .id();
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<Node>(glyph)
            .ok_or("glyph Node missing")?
            .display,
        Display::None
    );
    app.world_mut()
        .get_mut::<ClassList>(glyph)
        .ok_or("glyph classes missing")?
        .remove("hidden");
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<Node>(glyph)
            .ok_or("glyph Node missing")?
            .display,
        Display::Flex
    );
    app.init_resource::<bevy::input_focus::InputFocus>()
        .init_resource::<bevy::input_focus::InputFocusVisible>();
    let button = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            ClassList::new("button authored-focus"),
        ))
        .id();
    let ring = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(button),
            ClassList::new("control-focus"),
        ))
        .id();
    let flat_ring = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(action),
            ClassList::new("control-focus"),
        ))
        .id();
    app.world_mut()
        .get_mut::<ClassList>(action)
        .ok_or("action classes")?
        .add("authored-focus");
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocusVisible>()
        .0 = true;
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(button, bevy::input_focus::FocusCause::Navigated);
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<Node>(ring)
            .ok_or("focus ring missing")?
            .display,
        Display::Flex
    );
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .clear();
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<Node>(ring)
            .ok_or("ring missing after blur")?
            .display,
        Display::None
    );
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(action, bevy::input_focus::FocusCause::Navigated);
    frames(&mut app);
    let outline = app
        .world()
        .get::<Node>(flat_ring)
        .ok_or("Flat focus ring missing")?;
    assert_eq!(outline.display, Display::Flex);
    assert_eq!(outline.border, UiRect::all(px(2)));
    assert_eq!(outline.left, px(1));
    Ok(())
}
