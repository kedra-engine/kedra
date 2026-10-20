//! Physical source paths must retain their meaning in native asset addresses.
mod common;
use bevy::prelude::*;
use common::{Result, baseline, document, setup};
use kedra_authoring::persistence::Workspace;

#[test]
fn ambiguous_native_paths_reject_baseline_assignment_and_restored_save_atomically() -> Result<()> {
    let (app, policies) = setup()?;
    let original = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    for path in [
        "prefabs/actor#variant.prefab.ron",
        "prefabs#label/actor.prefab.ron",
    ] {
        let mut source = baseline(std::slice::from_ref(&original))?;
        source.documents[0].path = path.into();
        let rejected = Workspace::open(source.clone(), &types.read(), &policies, &mut loader);
        assert!(rejected.diagnostic.is_some(), "ambiguous baseline admitted");
        assert_eq!(
            rejected.source.documents[0].source,
            source.documents[0].source
        );

        let mut workspace = Workspace::open(baseline(&[])?, &types.read(), &policies, &mut loader);
        workspace.working.open(original.clone())?;
        let before = ron::to_string(&workspace)?;
        assert!(workspace.assign_path(original.id, path.into()).is_err());
        assert_eq!(ron::to_string(&workspace)?, before);

        let workspace = Workspace::open(
            baseline(std::slice::from_ref(&original))?,
            &types.read(),
            &policies,
            &mut loader,
        );
        let old = workspace.path(original.id).ok_or("path missing")?;
        let encoded = ron::to_string(&workspace)?;
        let mut restored: Workspace = ron::from_str(&encoded.replace(old, path))?;
        let before = ron::to_string(&restored)?;
        assert!(
            restored
                .prepare_save(None, &types.read(), &policies, &mut loader)
                .is_err()
        );
        assert_eq!(ron::to_string(&restored)?, before);
    }
    Ok(())
}

#[test]
fn unicode_spaces_and_literal_percent_paths_roundtrip_without_uri_encoding() -> Result<()> {
    let (app, policies) = setup()?;
    let original = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    for path in [
        "prefabs/Игрок 场景.prefab.ron",
        "prefabs/actor%23literal.prefab.ron",
    ] {
        let mut source = baseline(std::slice::from_ref(&original))?;
        source.documents[0].path = path.into();
        let mut workspace = Workspace::open(source, &types.read(), &policies, &mut loader);
        assert!(workspace.diagnostic.is_none());
        let request = workspace.prepare_save(None, &types.read(), &policies, &mut loader)?;
        let index = request
            .files
            .iter()
            .find(|file| file.document.is_none())
            .and_then(|file| file.source.as_deref())
            .ok_or("catalog missing")?;
        let paths = kedra_scene::assets::CatalogLoader::parse(index)?;
        let native = bevy::asset::AssetPath::try_parse(&paths[0])?;
        assert_eq!(native.path(), std::path::Path::new(path));
        assert_eq!(native.label(), None);
        assert_eq!(
            native.source(),
            &bevy::asset::io::AssetSourceId::from("game")
        );
        assert_eq!(paths[0], format!("game://{path}"));
    }
    Ok(())
}
