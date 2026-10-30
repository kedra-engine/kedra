//! Kedra's shared live-style integration must retain dynamic selector behavior.
use bevy::{asset::AssetPlugin, input_focus::InputFocus, prelude::*};
use bevy_flair::{
    core::{PropertyCanonicalName, PropertyRegistry, ReflectValue},
    prelude::InlineStyle,
    style::{
        BlockBuilder, StyleBuilderProperty, StyleSheet,
        asset_loader::StyleAssetLoader,
        components::{ClassList, Styled},
        css_selector::CssSelector,
    },
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn sheet(app: &App, rules: &[(&str, f32)]) -> Result<Handle<StyleSheet>> {
    let mut builder = StyleSheet::builder();
    for (selector, width) in rules {
        builder
            .new_ruleset()
            .with_css_selector(CssSelector::parse_single(selector).map_err(|e| format!("{e:?}"))?)
            .with_property(StyleBuilderProperty::new(
                PropertyCanonicalName::from_component_field::<Node>("width"),
                ReflectValue::from(Val::Px(*width)),
            ));
    }
    let world = app.world();
    let server = world.resource::<AssetServer>();
    let sheet = builder.build(
        &world.resource::<AppTypeRegistry>().read(),
        world.resource::<PropertyRegistry>(),
        StyleAssetLoader::from_asset_server(server),
    )?;
    Ok(server.add(sheet))
}

#[test]
fn inline_changes_keep_geometry_variables_inheritance_and_transitions_live() -> Result<()> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .add_plugins(kedra_ui::styles::StylesPlugin);
    let styles = sheet(&app, &[("*", 10.)])?;
    let parent = app
        .world_mut()
        .spawn((
            Node::default(),
            TextColor::default(),
            Styled::new(styles.clone()),
            InlineStyle::new("width:30px; --extent:20px; color:#112233"),
        ))
        .id();
    let explicit = app
        .world_mut()
        .spawn((
            Node::default(),
            TextColor::default(),
            ChildOf(parent),
            InlineStyle::new("width:var(--extent); color:inherit"),
        ))
        .id();
    let implicit = app
        .world_mut()
        .spawn((
            Node::default(),
            TextColor::default(),
            ChildOf(parent),
            InlineStyle::new("width:var(--extent)"),
        ))
        .id();
    app.finish();
    app.cleanup();
    ready(&mut app, &[styles]);
    let width = |app: &App, entity| app.world().get::<Node>(entity).map(|n| n.width);
    let color = |app: &App, entity| app.world().get::<TextColor>(entity).map(|c| c.0);
    assert_eq!(width(&app, parent), Some(Val::Px(30.)));
    for child in [explicit, implicit] {
        assert_eq!(width(&app, child), Some(Val::Px(20.)));
        assert_eq!(color(&app, child), Some(Color::srgb_u8(0x11, 0x22, 0x33)));
    }
    app.world_mut()
        .get_mut::<InlineStyle>(parent)
        .ok_or("Missing inline style")?
        .set("width", "40px");
    app.update();
    assert_eq!(width(&app, parent), Some(Val::Px(40.)));
    for child in [explicit, implicit] {
        assert_eq!(width(&app, child), Some(Val::Px(20.)));
    }
    {
        let mut inline = app
            .world_mut()
            .get_mut::<InlineStyle>(parent)
            .ok_or("Missing inline style")?;
        inline.set("--extent", "25px");
        inline.set("color", "#445566");
    }
    app.update();
    for child in [explicit, implicit] {
        assert_eq!(width(&app, child), Some(Val::Px(25.)));
        assert_eq!(color(&app, child), Some(Color::srgb_u8(0x44, 0x55, 0x66)));
    }
    app.world_mut()
        .get_mut::<InlineStyle>(parent)
        .ok_or("Missing inline style")?
        .set("color", "#778899");
    app.update();
    for child in [explicit, implicit] {
        assert_eq!(width(&app, child), Some(Val::Px(25.)));
        assert_eq!(color(&app, child), Some(Color::srgb_u8(0x77, 0x88, 0x99)));
    }
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(250),
    ));
    {
        let mut inline = app
            .world_mut()
            .get_mut::<InlineStyle>(parent)
            .ok_or("Missing inline style")?;
        inline.set("transition", "width 1s linear");
        inline.set("width", "80px");
    }
    app.update();
    app.update();
    assert!(matches!(width(&app, parent), Some(Val::Px(value)) if value > 40. && value < 80.));
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(width(&app, parent), Some(Val::Px(80.)));
    Ok(())
}

fn ready(app: &mut App, sheets: &[Handle<StyleSheet>]) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.update();
        if sheets.iter().all(|sheet| {
            app.world()
                .resource::<AssetServer>()
                .is_loaded_with_dependencies(sheet.id())
        }) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "In-memory stylesheet dependencies not ready"
        );
        std::thread::yield_now();
    }
}

#[test]
fn class_focus_reparenting_and_nested_stylesheets_keep_live_selector_authority() -> Result<()> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<InputFocus>()
        .add_plugins(kedra_ui::styles::StylesPlugin);
    let outer = sheet(
        &app,
        &[
            ("*", 10.),
            (".target", 20.),
            (".target:has(.leaf:focus)", 30.),
            (".target + .neighbor", 40.),
        ],
    )?;
    let nested = sheet(&app, &[("*", 60.), (".target:has(.leaf:focus)", 70.)])?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(outer.clone())))
        .id();
    let target = app
        .world_mut()
        .spawn((Node::default(), ClassList::new("other"), ChildOf(root)))
        .id();
    let leaf = app
        .world_mut()
        .spawn((Node::default(), ClassList::new("leaf"), ChildOf(target)))
        .id();
    let neighbor = app
        .world_mut()
        .spawn((Node::default(), ClassList::new("neighbor"), ChildOf(root)))
        .id();
    let nested_root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(nested.clone()), ChildOf(root)))
        .id();
    app.finish();
    app.cleanup();
    ready(&mut app, &[outer, nested]);
    let width = |app: &App, entity| app.world().get::<Node>(entity).map(|n| n.width);
    assert_eq!(width(&app, target), Some(Val::Px(10.)));
    assert_eq!(width(&app, neighbor), Some(Val::Px(10.)));
    app.world_mut()
        .get_mut::<ClassList>(target)
        .ok_or("Missing classes")?
        .toggle("target");
    app.update();
    assert_eq!(width(&app, target), Some(Val::Px(20.)));
    assert_eq!(width(&app, neighbor), Some(Val::Px(40.)));
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(leaf, bevy::input_focus::FocusCause::Navigated);
    app.update();
    assert_eq!(width(&app, target), Some(Val::Px(30.)));
    app.world_mut()
        .entity_mut(target)
        .insert(ChildOf(nested_root));
    app.update();
    assert_eq!(width(&app, target), Some(Val::Px(70.)));
    assert_eq!(width(&app, neighbor), Some(Val::Px(10.)));
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert_eq!(width(&app, target), Some(Val::Px(60.)));
    app.world_mut()
        .get_mut::<ClassList>(target)
        .ok_or("Missing classes")?
        .toggle("target");
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(leaf, bevy::input_focus::FocusCause::Navigated);
    app.update();
    assert_eq!(width(&app, target), Some(Val::Px(60.)));
    assert_eq!(
        app.world().get::<ChildOf>(leaf).map(ChildOf::parent),
        Some(target)
    );
    Ok(())
}
