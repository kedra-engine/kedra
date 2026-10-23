//! Environment assets use the same bounded native loader in installed games.
use bevy::{
    asset::{AssetPlugin, LoadState},
    prelude::*,
};
use kedra_scene::{assets::SceneAssetPlugin, environment::Environment};
use std::time::{Duration, Instant};

#[test]
fn installed_environment_survives_source_retirement_and_invalid_build_is_not_published()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    std::fs::create_dir(&source)?;
    let text = "(version:1,background:Srgba((red:0.1,green:0.2,blue:0.3,alpha:1.0)),ambient_color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),ambient_brightness:120.0)";
    std::fs::write(source.join("studio.environment.ron"), text)?;
    kedra_import::initialize(&source, |_| {})?;
    let delivery = temp.path().join("delivery");
    let manifest = kedra_import::build(&source, &delivery, "editor-desktop", "game", |_| {})?;
    assert_eq!(
        manifest.assets[0].loader,
        "kedra_scene::environment::EnvironmentLoader"
    );
    assert_eq!(
        std::fs::read_to_string(source.join("studio.environment.ron"))?,
        text
    );
    std::fs::write(
        source.join("studio.environment.ron"),
        text.replace("120.0", "-1.0"),
    )?;
    let rejected = temp.path().join("rejected");
    assert!(kedra_import::build(&source, &rejected, "editor-desktop", "game", |_| {}).is_err());
    assert!(!rejected.exists());
    std::fs::rename(&source, temp.path().join("retired-source"))?;
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "game",
        delivery.to_str().ok_or("Unicode delivery")?.into(),
        manifest,
    );
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), SceneAssetPlugin));
    app.finish();
    app.cleanup();
    let server = app.world().resource::<AssetServer>().clone();
    let handle: Handle<Environment> = server.load("game://studio.environment.ron");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.is_loaded_with_dependencies(handle.id()) {
        app.update();
        assert!(!matches!(
            server.get_load_state(handle.id()),
            Some(LoadState::Failed(_))
        ));
        assert!(Instant::now() < deadline, "Environment loading timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        app.world()
            .resource::<Assets<Environment>>()
            .get(&handle)
            .ok_or("Loaded environment missing")?
            .ambient_brightness,
        120.
    );
    Ok(())
}
