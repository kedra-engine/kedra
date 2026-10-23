//! Installed CSS contracts: native consumers, source retirement and publication refusal.
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
fn delivered_stylesheet_switch_preserves_retained_input_and_focus() -> Result<()> {
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
