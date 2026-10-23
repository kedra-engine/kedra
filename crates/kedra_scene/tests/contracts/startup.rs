//! Real native source loading, one-shot entry instantiation and failure ownership.
use bevy::{asset::AssetPlugin, prelude::*};
use kedra_scene::{
    assets::{PrefabPlugin, PrefabSources},
    document::{DocumentKind, PrefabDocument},
    instance::PrefabInstance,
    startup::{StartupScene, StartupScenePlugin, StartupState},
};
use std::{
    fs,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn host(root: &std::path::Path, path: &str) -> Result<App> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: root.to_str().ok_or("Non-Unicode test asset root")?.into(),
            watch_for_changes_override: Some(false),
            use_asset_processor_override: Some(false),
            ..default()
        },
        PrefabPlugin {
            catalog_path: "catalog.prefabs.ron".into(),
        },
        StartupScenePlugin::new(path),
    ));
    app.finish();
    app.cleanup();
    Ok(app)
}

fn terminal(app: &mut App) -> Result<StartupState> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let state = app.world().resource::<StartupScene>().state().clone();
        if state != StartupState::Loading {
            return Ok(state);
        }
        if Instant::now() >= deadline {
            return Err("Entry source never reached a terminal state".into());
        }
        std::thread::yield_now();
    }
}

fn sources(root: &std::path::Path) -> Result<(PrefabDocument, PrefabDocument)> {
    let mut first = PrefabDocument::new("First entry");
    first.kind = DocumentKind::Scene;
    let mut second = PrefabDocument::new("Second entry");
    second.kind = DocumentKind::Scene;
    fs::write(root.join("first.scene.ron"), first.serialize()?)?;
    fs::write(root.join("second.scene.ron"), second.serialize()?)?;
    fs::write(
        root.join("catalog.prefabs.ron"),
        "[\"first.scene.ron\",\"second.scene.ron\"]",
    )?;
    Ok((first, second))
}

#[test]
fn explicit_path_selects_the_scene_and_root_retirement_never_restarts_it() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let (first, second) = sources(directory.path())?;
    for (path, expected) in [
        ("first.scene.ron", first.id),
        ("second.scene.ron", second.id),
    ] {
        let mut app = host(directory.path(), path)?;
        let StartupState::Mounted { document, root } = terminal(&mut app)? else {
            return Err("Valid scene failed to instantiate".into());
        };
        assert_eq!(document, expected);
        assert_eq!(app.world().resource::<StartupScene>().path(), path);
        assert_eq!(
            app.world()
                .get::<PrefabInstance>(root)
                .ok_or("Missing native instance")?
                .document,
            expected
        );
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world_mut()
                .query::<&PrefabInstance>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut().despawn(root);
        app.update();
        assert_eq!(
            app.world().resource::<StartupScene>().state(),
            &StartupState::Retired
        );
        assert_eq!(app.world().resource::<PrefabSources>().active_scene, None);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&PrefabInstance>()
                .iter(app.world())
                .count(),
            0
        );
    }
    Ok(())
}

#[test]
fn missing_invalid_non_scene_and_out_of_catalog_sources_fail_without_instances() -> Result<()> {
    let directory = tempfile::tempdir()?;
    sources(directory.path())?;
    let prefab = PrefabDocument::new("Not an entry scene");
    fs::write(
        directory.path().join("prefab.prefab.ron"),
        prefab.serialize()?,
    )?;
    fs::write(directory.path().join("broken.scene.ron"), "invalid source")?;
    for (path, member) in [
        ("missing.scene.ron", false),
        ("broken.scene.ron", true),
        ("prefab.prefab.ron", true),
        ("second.scene.ron", false),
        ("://invalid", false),
    ] {
        let catalog = if member {
            format!("[\"{path}\"]")
        } else {
            "[\"first.scene.ron\"]".into()
        };
        fs::write(directory.path().join("catalog.prefabs.ron"), catalog)?;
        let mut app = host(directory.path(), path)?;
        assert!(
            matches!(terminal(&mut app)?, StartupState::Failed(_)),
            "Accepted {path}"
        );
        assert_eq!(
            app.world_mut()
                .query::<&PrefabInstance>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(app.world().resource::<PrefabSources>().active_scene, None);
    }
    Ok(())
}
