//! Real Bevy asset delivery and Flair cascade, with CPU-only test images.
use super::*;
use bevy::asset::{
    AssetLoader, AssetMetaCheck, LoadContext,
    io::{
        Reader,
        memory::{Dir, MemoryAssetReader},
    },
};
use bevy_flair::style::{StyleSheet, components::Styled};
use std::{
    path::Path,
    time::{Duration, Instant},
};

// Like the editor's CPU fixture, this tests hydration/ownership, not image
// decoding, SVG processing or GPU fidelity. No platform renderer is installed.
#[derive(Default, TypePath)]
struct CpuImageLoader;
impl AssetLoader for CpuImageLoader {
    type Asset = Image;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        _: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> std::result::Result<Image, Self::Error> {
        Ok(Image::default())
    }
    fn extensions(&self) -> &[&str] {
        &["cpu"]
    }
}

fn drive_until(app: &mut App, reason: &str, ready: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready(app) {
        assert!(Instant::now() < deadline, "{reason}");
        app.update();
        std::thread::yield_now();
    }
}

#[test]
fn delivered_stylesheet_image_and_inline_geometry_hydrate_through_native_assets() -> Result<()> {
    let memory = Dir::default();
    memory.insert_asset_text(
        Path::new("panel.css"),
        ".panel { width:123px; min-width:0px; }",
    );
    memory.insert_asset_text(Path::new("icon.cpu"), "CPU-only image fixture");
    let mut app = App::new();
    app.register_asset_source(
        "memory",
        bevy::asset::io::AssetSourceBuilder::new(move || {
            Box::new(MemoryAssetReader {
                root: memory.clone(),
            })
        }),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: AssetMetaCheck::Never,
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
    ))
    .init_asset::<Font>()
    .init_asset::<Image>()
    .init_asset_loader::<CpuImageLoader>()
    .init_resource::<AppTypeRegistry>()
    .add_plugins(AuthoredUiPlugin);
    // CSS workers share the registry completed by Flair's finish hooks.
    // Match native startup: register all properties before requesting assets.
    app.finish();
    app.cleanup();
    assert_eq!(app.plugins_state(), bevy::app::PluginsState::Cleaned);
    let sheet: Handle<StyleSheet> = app
        .world()
        .resource::<AssetServer>()
        .load("memory://panel.css");
    let image: Handle<Image> = app
        .world()
        .resource::<AssetServer>()
        .load("memory://icon.cpu");
    let mut source = PrefabDocument::new("Delivered UI assets");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    source.nodes[0].components.insert(
        "ui.classes".into(),
        ComponentData::ron(1, "(names:[\"panel\"])")?,
    );
    source.nodes[0].components.insert(
        "ui.caption".into(),
        ComponentData::ron(1, "(text:\"Lossless e\\u{301}\",justify:Left,wrap:false)")?,
    );
    let policies = app.world().resource::<ComponentPolicies>();
    let types = app.world().resource::<AppTypeRegistry>().read();
    source.nodes[0].components.insert(
        "ui.stylesheet".into(),
        policies.encode(
            "ui.stylesheet",
            &UiStylesheet {
                asset: sheet.clone(),
            },
            &types,
            &BTreeMap::new(),
        )?,
    );
    let picture = SourceNode {
        id: NodeId::new(),
        name: "Picture".into(),
        parent: Some(source.root.into()),
        components: BTreeMap::from([
            ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
            ("ui.id".into(), ComponentData::ron(1, "(\"image\")")?),
            (
                "ui.image".into(),
                policies.encode(
                    "ui.image",
                    &UiImage {
                        variants: vec![UiImageDensity {
                            density: 1.,
                            image: image.clone(),
                        }],
                        color: Color::WHITE,
                    },
                    &types,
                    &BTreeMap::new(),
                )?,
            ),
        ]),
    };
    drop(types);
    source.nodes.push(picture);
    let snapshot = compile(&app, source)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !snapshot.asset_status(app.world().resource::<AssetServer>())? {
        assert!(
            Instant::now() < deadline,
            "In-memory stylesheet/image dependencies did not load"
        );
        app.update();
        std::thread::yield_now();
    }
    let before = app.world().entities().count_spawned();
    assert!(
        UiView::mount(
            app.world_mut(),
            &snapshot,
            None,
            &BTreeMap::new(),
            "assets",
            2.
        )
        .is_err()
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    let view = UiView::mount(
        app.world_mut(),
        &snapshot,
        None,
        &BTreeMap::new(),
        "assets",
        1.,
    )?;
    assert!(
        matches!(app.world().get::<Styled>(view.root), Some(Styled::StyleSheet(handle)) if handle == &sheet)
    );
    assert_eq!(
        app.world()
            .get::<ImageNode>(view.element("image")?)
            .ok_or("native image")?
            .image,
        image
    );
    assert_eq!(
        app.world()
            .get::<Text>(view.root)
            .ok_or("native caption")?
            .0,
        "Lossless e\u{301}"
    );
    // This observes Flair's actual Node-property publication, not a fake parser or
    // a competing native Node writer. Layout/rendering are deliberately absent.
    drive_until(&mut app, "Flair did not publish stylesheet width", |app| {
        app.world()
            .get::<Node>(view.root)
            .is_some_and(|n| n.width == px(123))
    });
    assert_eq!(
        app.world().get::<Node>(view.root).ok_or("root")?.width,
        px(123)
    );
    kedra_ui::docking::geometry(
        app.world_mut(),
        view.root,
        kedra_ui::docking::Geometry::Rectangle(Rect::from_corners(
            Vec2::new(2., 3.),
            Vec2::new(32., 43.),
        )),
    )?;
    drive_until(&mut app, "Flair did not publish inline geometry", |app| {
        app.world()
            .get::<Node>(view.root)
            .is_some_and(|n| (n.left, n.top, n.width, n.height) == (px(2), px(3), px(30), px(40)))
    });
    let node = app.world().get::<Node>(view.root).ok_or("styled root")?;
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (px(2), px(3), px(30), px(40))
    );
    app.world_mut().despawn(view.root);
    let removed = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .remove(image.id())
        .ok_or("materialized image")?;
    let before = app.world().entities().count_spawned();
    let error = UiView::mount(
        app.world_mut(),
        &snapshot,
        None,
        &BTreeMap::new(),
        "assets",
        1.,
    )
    .err()
    .ok_or("missing image accepted")?;
    assert!(
        error
            .to_string()
            .contains("image dependency is not materialized")
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(image.id(), removed)?;
    assert!(
        UiView::mount(
            app.world_mut(),
            &snapshot,
            None,
            &BTreeMap::new(),
            "assets",
            1.
        )
        .is_ok()
    );
    Ok(())
}
