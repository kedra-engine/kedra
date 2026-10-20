//! Disposable inspection rigs use existing authored projection, never document authority.
use bevy::prelude::*;
use kedra_scene::{
    composition::{Catalog, ResolvedGraph},
    document::{ComponentData, PrefabDocument},
    reflection::ComponentPolicies,
};
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn rig(world: &World, path: &str) -> Result<Arc<ResolvedGraph>, String> {
    let uri = format!("game://{path}");
    let registry = world.resource::<AppTypeRegistry>();
    let types = registry.read();
    let policies = world.resource::<ComponentPolicies>();
    let encode = |key, value: &dyn PartialReflect| {
        policies
            .encode(key, value, &types, &BTreeMap::new())
            .map_err(|e| e.to_string())
    };
    // These generated IDs identify an ephemeral presentation, not an admitted
    // source document. The rig is never inserted into Documents or history.
    let mut rig = PrefabDocument::new("Asset inspection rig");
    if path.ends_with(".png") {
        rig.nodes[0].components.insert(
            "ui.layout".into(),
            encode(
                "ui.layout",
                &kedra_ui::authored::components::UiLayout {
                    node: Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                },
            )?,
        );
        rig.nodes[0].components.insert("ui.image".into(),ComponentData::ron(1,&format!("(variants:[(density:1.0,image:Path({}))],color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)))",ron::to_string(&uri).map_err(|e|e.to_string())?)).map_err(|e|e.to_string())?);
    } else if path.ends_with(".glb") {
        let uri = ron::to_string(&format!("{uri}#Scene0")).map_err(|e| e.to_string())?;
        rig.nodes[0].components.insert(
            "scene.model".into(),
            ComponentData::ron(1, &format!("(scene:Path({uri}))")).map_err(|e| e.to_string())?,
        );
    } else {
        return Err("Only PNG images and static GLB Scene0 asset previews are supported".into());
    }
    let id = rig.id;
    let mut catalog = Catalog::default();
    catalog.insert(rig).map_err(|e| e.to_string())?;
    catalog.resolve(id).map(Arc::new).map_err(|e| e.to_string())
}
