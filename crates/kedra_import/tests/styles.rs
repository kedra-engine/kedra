//! Native CSS delivery and runtime behavior, independent of editor widgets.
use bevy::{asset::AssetPlugin, prelude::*};
use bevy_flair::style::{
    StyleSheet,
    components::{ClassList, Styled},
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "styles/compact.rs"]
mod compact;
#[path = "styles/inspector.rs"]
mod inspector;
#[path = "styles/startup.rs"]
mod startup;

fn consumer(root: &Path, manifest: kedra_assets::delivery::Manifest) -> App {
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "game",
        root.to_str().unwrap_or_default().into(),
        manifest,
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
        kedra_ui::styles::StylesPlugin,
    ));
    app.finish();
    app.cleanup();
    app
}
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

#[test]
fn scene_toolbar_stays_fixed_and_navigation_restores_after_hidden_state() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(
        source.join("scene.css"),
        format!(
            "{}\n{}\n{}\n.theme {{ --control-height:22px; --space:4px; }}",
            include_str!("../../../assets/editor/styles/controls.css"),
            include_str!("../../../assets/editor/styles/startup.css"),
            include_str!("../../../assets/editor/styles/scene-view.css")
        ),
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    let mut app = consumer(&output, manifest);
    let sheet = load(&mut app, "game://scene.css")?;
    let root = app
        .world_mut()
        .spawn((
            Node::default(),
            Styled::new(sheet),
            ClassList::new("theme scene-view"),
        ))
        .id();
    let toolbar = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("scene-toolbar"),
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
            ClassList::new("button quiet scene-menu scene-view-menu"),
            ChildOf(toolbar),
        ))
        .id();
    let label = app
        .world_mut()
        .spawn((
            Node::default(),
            TextFont::default(),
            ClassList::new("text"),
            ChildOf(menu),
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
    let rail = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("scene-tool-rail hidden"),
            ChildOf(root),
        ))
        .id();
    let diagnostic = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("scene-diagnostic"),
            ChildOf(root),
        ))
        .id();
    frames(&mut app);
    let node = app.world().get::<Node>(toolbar).ok_or("Toolbar missing")?;
    assert_eq!(
        node.flex_wrap,
        FlexWrap::NoWrap,
        "The selected design keeps commands on one row, using View in narrow docks"
    );
    assert_eq!(node.height, px(38));
    assert_eq!(node.min_height, px(38));
    assert_eq!(node.column_gap, px(2));
    assert_eq!(node.padding.top, px(6));
    assert_eq!(node.padding.bottom, px(6));
    let node = app
        .world()
        .get::<Node>(projection)
        .ok_or("Projection missing")?;
    assert_eq!(
        (node.width, node.height, node.flex_shrink),
        (px(22), px(22), 0.)
    );
    assert_eq!(
        app.world()
            .get::<Node>(diagnostic)
            .ok_or("Diagnostic missing")?
            .top,
        px(8)
    );
    let document = kedra_scene::document::PrefabDocument::parse(include_str!(
        "../../../assets/editor/prefabs/source-document.prefab.ron"
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
    assert_eq!(
        app.world()
            .get::<TextFont>(label)
            .ok_or("Menu font missing")?
            .weight,
        bevy::text::FontWeight::MEDIUM
    );
    for hidden in [true, false, true, false] {
        for target in [navigation, rail, menu] {
            kedra_ui::docking::class(app.world_mut(), target, "hidden", hidden);
        }
        frames(&mut app);
        for target in [navigation, rail, menu] {
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
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(
        source.join("dock.css"),
        format!(
            "{}\n.theme {{--canvas:#242424;--panel:#303030;--hover:#626262;--border:#1c1c1c;--muted:#afafaf;--focus:#b1d2ff;}}",
            include_str!("../../../assets/editor/styles/docking.css")
        ),
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    let mut app = consumer(&output, manifest);
    let sheet = load(&mut app, "game://dock.css")?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(sheet), ClassList::new("theme")))
        .id();
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
    assert_eq!(
        idle.padding.right,
        px(32),
        "The close slot must exist before hover"
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
fn font_identity_uses_native_path_components_but_keeps_source_and_label_exact() {
    use bevy::asset::AssetPath;
    let native = AssetPath::from_path_buf(Path::new("fonts").join("JetBrainsMono-Regular.ttf"))
        .with_source("editor");
    assert_eq!(
        native,
        AssetPath::from("editor://fonts/JetBrainsMono-Regular.ttf")
    );
    for other in [
        "game://fonts/JetBrainsMono-Regular.ttf",
        "editor://fonts/Inter-Regular.ttf",
        "editor://fonts/JetBrainsMono-Regular.ttf#other",
    ] {
        assert_ne!(native, AssetPath::from(other), "{other}");
    }
}

#[test]
fn imported_variables_control_native_geometry_states_and_subtree_theme_switches() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(
        source.join("tokens.css"),
        ".theme { --accent: #416b9d; --radius: 7px; }",
    )?;
    fs::write(
        source.join("hud.css"),
        r#"@import "tokens.css";
        .action { width:120px; height:26px; background-color:var(--accent); border:2px solid #111; border-radius:var(--radius); padding:3px 9px; }
        .action:hover {background-color:#00ff00;}
        .action:disabled {background-color:#666666;}
        .action:focus {outline-width:1px; outline-color:#ffffff;}
        .action.local {border-radius:11px;}
    "#,
    )?;
    fs::write(
        source.join("alternate.css"),
        r#".theme { --accent:#ff0000; --radius:3px; }
        .action { width:120px; height:26px; background-color:var(--accent); border:2px solid #111; border-radius:var(--radius); padding:3px 9px; }
        .action.local {border-radius:11px;}
    "#,
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    fs::rename(&source, temp.path().join("offline"))?;
    let mut app = consumer(&output, manifest);
    let first = load(&mut app, "game://hud.css")?;
    let second = load(&mut app, "game://alternate.css")?;
    let root = app
        .world_mut()
        .spawn((
            Node::default(),
            Styled::new(first.clone()),
            ClassList::new("theme"),
        ))
        .id();
    let button = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            ClassList::new("action"),
            bevy::picking::hover::Hovered(false),
        ))
        .id();
    let isolated = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            Styled::new(second.clone()),
            ClassList::new("theme"),
        ))
        .id();
    let local = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(isolated),
            ClassList::new("action local"),
        ))
        .id();
    frames(&mut app);
    let node = app
        .world()
        .get::<Node>(button)
        .ok_or("native Node missing")?;
    assert_eq!(node.width, px(120));
    assert_eq!(node.border.left, px(2));
    assert_eq!(node.border_radius, BorderRadius::all(px(7)));
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(button)
            .ok_or("background missing")?
            .0,
        Color::srgb_u8(65, 107, 157)
    );
    assert_eq!(
        app.world()
            .get::<Node>(local)
            .ok_or("local Node missing")?
            .border_radius,
        BorderRadius::all(px(11))
    );
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(local)
            .ok_or("local background missing")?
            .0,
        Color::srgb_u8(255, 0, 0)
    );
    app.world_mut()
        .entity_mut(button)
        .insert(bevy::picking::hover::Hovered(true));
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(button)
            .ok_or("background missing")?
            .0,
        Color::srgb_u8(0, 255, 0)
    );
    app.world_mut()
        .entity_mut(button)
        .insert(bevy::ui::InteractionDisabled);
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(button)
            .ok_or("background missing")?
            .0,
        Color::srgb_u8(102, 102, 102)
    );
    app.world_mut().entity_mut(root).insert(Styled::new(second));
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(button)
            .ok_or("background missing")?
            .0,
        Color::srgb_u8(255, 0, 0)
    );
    assert_eq!(
        app.world()
            .get::<Node>(button)
            .ok_or("Node missing")?
            .border_radius,
        BorderRadius::all(px(3))
    );
    assert_eq!(
        app.world()
            .get::<Node>(local)
            .ok_or("local Node missing")?
            .border_radius,
        BorderRadius::all(px(11))
    );
    Ok(())
}

#[test]
fn stylesheet_switch_preserves_native_input_value_viewport_focus_and_editor_state() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(
        source.join("first.css"),
        ".input {font-size:12px; line-height:16px; color:#eeeeee; width:180px;}",
    )?;
    fs::write(
        source.join("second.css"),
        ".input {font-size:18px; line-height:24px; color:#00ff00; width:240px;}",
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let installed = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &installed, "game", "game", |_| {})?;
    let mut app = consumer(&installed, manifest);
    app.init_resource::<bevy::input_focus::InputFocus>();
    let first = load(&mut app, "game://first.css")?;
    let second = load(&mut app, "game://second.css")?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(first)))
        .id();
    let mut input = bevy::text::EditableText::new("Кедра / 日本語");
    input.visible_width = Some(96.);
    input.cursor_width = 1.5;
    input.editor_mut().set_scale(2.);
    let entity = app
        .world_mut()
        .spawn((
            Node::default(),
            ChildOf(root),
            ClassList::new("input"),
            TextFont::default(),
            TextColor::default(),
            input,
        ))
        .id();
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(entity, bevy::input_focus::FocusCause::Navigated);
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<TextFont>(entity)
            .ok_or("input font missing")?
            .font_size,
        FontSize::Px(12.)
    );
    app.world_mut().entity_mut(root).insert(Styled::new(second));
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<TextFont>(entity)
            .ok_or("input font missing")?
            .font_size,
        FontSize::Px(18.)
    );
    assert_eq!(
        app.world()
            .get::<Node>(entity)
            .ok_or("input Node missing")?
            .width,
        px(240)
    );
    assert_eq!(
        app.world()
            .get::<TextColor>(entity)
            .ok_or("input color missing")?
            .0,
        Color::srgb_u8(0, 255, 0)
    );
    let input = app
        .world()
        .get::<bevy::text::EditableText>(entity)
        .ok_or("native editor was replaced")?;
    assert_eq!(input.value().to_string(), "Кедра / 日本語");
    assert_eq!(input.visible_width, Some(96.));
    assert_eq!(input.cursor_width, 1.5);
    assert_eq!(input.editor().get_scale(), 2.);
    assert_eq!(
        app.world()
            .resource::<bevy::input_focus::InputFocus>()
            .get(),
        Some(entity)
    );
    Ok(())
}

#[test]
fn invalid_css_or_missing_import_never_publishes_a_delivery() -> Result<()> {
    for css in [
        ".button { not-a-property: 20px; }",
        "@import \"absent.css\"; .button {width:20px;}",
    ] {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        fs::create_dir(&source)?;
        fs::write(source.join("bad.css"), css)?;
        kedra_import::initialize(&source, |_| {})?;
        let before = fs::read(source.join("bad.css.meta"))?;
        let output = temp.path().join("rejected");
        let result = kedra_import::build(&source, &output, "game", "game", |_| {});
        assert!(result.is_err(), "invalid CSS was accepted: {css}");
        assert!(!output.exists());
        assert_eq!(fs::read(source.join("bad.css.meta"))?, before);
    }
    Ok(())
}

#[test]
fn recovery_loader_settings_and_oversized_css_are_rejected_before_publication() -> Result<()> {
    for (name, oversized) in [
        ("style.css", false),
        ("Style.CSS", false),
        ("bounded.css", true),
    ] {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        fs::create_dir(&source)?;
        fs::write(
            source.join(name),
            if oversized {
                " ".repeat(65_537)
            } else {
                ".button {width:20px;}".into()
            },
        )?;
        kedra_import::initialize(&source, |_| {})?;
        if !oversized {
            let meta = source.join(format!("{name}.meta"));
            let text = fs::read_to_string(&meta)?.replace("ReturnError", "PrintWarn");
            fs::write(meta, text)?;
        }
        let output = temp.path().join("rejected");
        assert!(kedra_import::build(&source, &output, "game", "game", |_| {}).is_err());
        assert!(!output.exists());
    }
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
        ("button quiet", "text strong", 400, "Inter-Regular.ttf"),
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
        .spawn((Node::default(), ChildOf(root), ClassList::new("button")))
        .id();
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocusVisible>()
        .0 = true;
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(button, bevy::input_focus::FocusCause::Navigated);
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<Outline>(button)
            .ok_or("focus outline missing")?
            .width,
        px(1)
    );
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .clear();
    frames(&mut app);
    assert_eq!(
        app.world()
            .get::<Outline>(button)
            .ok_or("outline missing after blur")?
            .width,
        px(0)
    );
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(action, bevy::input_focus::FocusCause::Navigated);
    frames(&mut app);
    let outline = app
        .world()
        .get::<Outline>(action)
        .ok_or("Flat focus ring missing")?;
    assert_eq!(outline.width, px(2));
    assert_eq!(outline.offset, px(1));
    Ok(())
}
