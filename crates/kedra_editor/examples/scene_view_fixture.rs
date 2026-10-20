//! Generate NEW source-only QA data; build/verify deliveries explicitly with the CLI.
use bevy::prelude::*;
use kedra_scene::{
    document::{
        DocumentKind, InstanceId, NestedInstance, Node as SourceNode, NodeId, PrefabDocument,
    },
    reflection::ComponentPolicies,
    spatial::{Primitive, Sun, Surface},
};
use std::{collections::BTreeMap, path::PathBuf};

#[path = "scene_view_fixture/model.rs"]
mod model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass a NEW fixture parent directory")?,
    );
    if std::env::args_os().nth(2).is_some() {
        return Err(
            "The acceptance fixture generates its own model; pass only a new parent".into(),
        );
    }
    if parent.exists() {
        return Err("Fixture parent must not exist; existing work is never replaced".into());
    }
    std::fs::create_dir_all(&parent)?;
    let project = kedra_project::create(&parent, "Scene View QA")?;
    let mut types = bevy::reflect::TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut types, &mut policies)?;
    let encode =
        |key, value: &dyn PartialReflect| policies.encode(key, value, &types, &BTreeMap::new());
    let surface = Surface {
        color: Color::srgb(0.25, 0.55, 0.9),
        texture: None,
        metallic: 0.1,
        roughness: 0.5,
    };
    let mut prefab = PrefabDocument::new("Textured cuboid");
    prefab.nodes[0].components.insert(
        "scene.primitive".into(),
        encode(
            "scene.primitive",
            &Primitive::Cuboid {
                size: Vec3::splat(2.),
            },
        )?,
    );
    let mut material = encode("scene.surface", &surface)?;
    // Only the native dependency URI is fixture data. Serialization/validation is
    // still handled by the existing reflected compiler in both tests and product.
    material = kedra_scene::document::ComponentData::ron(
        1,
        &material
            .value
            .get_ron()
            .replace("texture:None", "texture:Some(Path(\"game://checker.png\"))"),
    )?;
    prefab.nodes[0]
        .components
        .insert("scene.surface".into(), material);
    prefab.nodes[0].components.insert(
        "scene.transform".into(),
        encode("scene.transform", &Transform::from_xyz(0., 1., 0.))?,
    );
    let mut scene = PrefabDocument::new("Composed spatial scene");
    std::fs::write(
        project.assets.join("day.environment.ron"),
        "(version:1,background:Srgba((red:0.14117648,green:0.16862746,blue:0.20392157,alpha:1.0)),ambient_color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),ambient_brightness:20.0)",
    )?;
    std::fs::write(
        project.assets.join("night.environment.ron"),
        "(version:1,background:Srgba((red:0.02,green:0.04,blue:0.09,alpha:1.0)),ambient_color:Srgba((red:0.3,green:0.4,blue:1.0,alpha:1.0)),ambient_brightness:5.0)",
    )?;
    scene.nodes[0].components.insert(
        "scene.environment".into(),
        kedra_scene::document::ComponentData::ron(
            1,
            "(asset:Path(\"game://day.environment.ron\"))",
        )?,
    );
    scene.kind = DocumentKind::Scene;
    scene.instances.push(NestedInstance {
        id: InstanceId::new(),
        asset: prefab.id,
        parent: scene.root.into(),
        bindings: BTreeMap::new(),
        overrides: Vec::new(),
    });
    let sphere = NodeId::new();
    scene.nodes.push(SourceNode {
        id: sphere,
        name: "Warm sphere".into(),
        parent: Some(scene.root.into()),
        components: BTreeMap::from([
            (
                "scene.transform".into(),
                encode("scene.transform", &Transform::from_xyz(3., 1., 0.))?,
            ),
            (
                "scene.primitive".into(),
                encode("scene.primitive", &Primitive::Sphere { radius: 1. })?,
            ),
            (
                "scene.surface".into(),
                encode(
                    "scene.surface",
                    &Surface {
                        color: Color::srgb(0.9, 0.35, 0.12),
                        texture: None,
                        metallic: 0.,
                        roughness: 0.4,
                    },
                )?,
            ),
        ]),
    });
    scene.nodes.push(SourceNode {
        id: NodeId::new(),
        name: "Authored sun".into(),
        parent: Some(scene.root.into()),
        components: BTreeMap::from([
            (
                "scene.transform".into(),
                encode(
                    "scene.transform",
                    &Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.5, 0.)),
                )?,
            ),
            (
                "scene.sun".into(),
                encode(
                    "scene.sun",
                    &Sun {
                        color: Color::WHITE,
                        illuminance: 15000.,
                    },
                )?,
            ),
        ]),
    });
    let mut other = PrefabDocument::new("Other scene");
    other.kind = DocumentKind::Scene;
    other.nodes[0].components = BTreeMap::from([
        (
            "scene.primitive".into(),
            encode("scene.primitive", &Primitive::Sphere { radius: 2. })?,
        ),
        (
            "scene.surface".into(),
            encode(
                "scene.surface",
                &Surface {
                    color: Color::srgb(0.2, 0.8, 0.3),
                    texture: None,
                    metallic: 0.,
                    roughness: 0.8,
                },
            )?,
        ),
    ]);
    let light = scene.nodes.last().ok_or("Fixture sun missing")?;
    other.nodes[0].components.insert(
        "scene.environment".into(),
        kedra_scene::document::ComponentData::ron(
            1,
            "(asset:Path(\"game://night.environment.ron\"))",
        )?,
    );
    other.nodes.push(SourceNode {
        id: NodeId::new(),
        name: light.name.clone(),
        parent: Some(other.root.into()),
        components: light.components.clone(),
    });
    let mut hud = PrefabDocument::new("Passive UI fixture");
    hud.nodes[0].components = BTreeMap::from([
        (
            "ui.layout".into(),
            kedra_scene::document::ComponentData::ron(1, "(node:())")?,
        ),
        (
            "ui.classes".into(),
            kedra_scene::document::ComponentData::ron(1, "(names:[\"hud\"])")?,
        ),
        (
            "ui.stylesheet".into(),
            kedra_scene::document::ComponentData::ron(1, "(asset:Path(\"game://preview.css\"))")?,
        ),
    ]);
    for (name, text, class, kind) in [
        ("title", "SCENE / SYSTEM CHECK", "hud-title", "text"),
        (
            "note",
            "Passive UI · no commands or text focus",
            "hud-note",
            "text",
        ),
        (
            "button",
            "Launch — inert in preview",
            "hud-button",
            "button",
        ),
        (
            "field",
            "Autofocus must stay in the editor",
            "hud-field",
            "field",
        ),
    ] {
        let mut components = BTreeMap::from([
            (
                "ui.layout".into(),
                kedra_scene::document::ComponentData::ron(1, "(node:())")?,
            ),
            (
                "ui.id".into(),
                kedra_scene::document::ComponentData::ron(
                    1,
                    &format!("({})", ron::to_string(&name)?),
                )?,
            ),
            (
                "ui.classes".into(),
                kedra_scene::document::ComponentData::ron(1, &format!("(names:[\"{class}\"])"))?,
            ),
            (
                "ui.caption".into(),
                kedra_scene::document::ComponentData::ron(
                    1,
                    &format!("(text:{},justify:Left,wrap:false)", ron::to_string(text)?),
                )?,
            ),
        ]);
        if kind == "button" {
            components.insert(
                "ui.button".into(),
                kedra_scene::document::ComponentData::ron(1, "()")?,
            );
        }
        if kind == "field" {
            components.insert(
                "ui.editable".into(),
                kedra_scene::document::ComponentData::ron(1, "(autofocus:true,select_all:true)")?,
            );
        }
        hud.nodes.push(SourceNode {
            id: NodeId::new(),
            name: name.into(),
            parent: Some(hud.root.into()),
            components,
        });
    }
    hud.nodes.push(SourceNode{id:NodeId::new(),name:"Image inside UI".into(),parent:Some(hud.root.into()),components:BTreeMap::from([
        ("ui.layout".into(),kedra_scene::document::ComponentData::ron(1,"(node:())")?),
        ("ui.classes".into(),kedra_scene::document::ComponentData::ron(1,"(names:[\"hud-picture\"])")?),
        ("ui.image".into(),kedra_scene::document::ComponentData::ron(1,"(variants:[(density:1.0,image:Path(\"game://checker.png\"))],color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)))")?),
    ])});
    scene.instances.push(NestedInstance {
        id: InstanceId::new(),
        asset: hud.id,
        parent: scene.root.into(),
        bindings: BTreeMap::new(),
        overrides: Vec::new(),
    });
    {
        let data = model::generate()?;
        std::fs::write(project.assets.join("ship.glb"), &data)?;
        std::fs::write(
            parent.join("model-provenance.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"source":"generated native Bevy cuboids; no external model", "sha256":kedra_assets::delivery::digest(&data), "materials":"opaque unlit cyan and yellow", "distribution":"generated QA fixture only"}),
            )?,
        )?;
        scene.nodes.push(SourceNode {
            id: NodeId::new(),
            name: "Generated two-material model".into(),
            parent: Some(scene.root.into()),
            components: BTreeMap::from([
                (
                    "scene.model".into(),
                    kedra_scene::document::ComponentData::ron(
                        1,
                        "(scene:Path(\"game://ship.glb#Scene0\"))",
                    )?,
                ),
                (
                    "scene.transform".into(),
                    encode("scene.transform", &Transform::from_xyz(-3., 1., 0.))?,
                ),
            ]),
        });
    }
    for (name, document) in [
        ("Composed.scene.ron", &scene),
        ("Cuboid.prefab.ron", &prefab),
        ("Other.scene.ron", &other),
        ("Hud.prefab.ron", &hud),
    ] {
        std::fs::write(project.assets.join(name), document.serialize()?)?;
    }
    let image = image::RgbaImage::from_fn(32, 32, |x, y| {
        if (x / 8 + y / 8) % 2 == 0 {
            image::Rgba([255, 255, 255, 255])
        } else {
            image::Rgba([55, 100, 160, 255])
        }
    });
    image.save(project.assets.join("checker.png"))?;
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor/fonts");
    std::fs::copy(
        fonts.join("Inter-Regular.ttf"),
        project.assets.join("Fixture-Inter.ttf"),
    )?;
    std::fs::copy(
        fonts.join("Inter-OFL.txt"),
        project.assets.join("Fixture-Inter-OFL.txt"),
    )?;
    std::fs::write(
        project.assets.join("preview.css"),
        "@font-face { font-family:\"FixtureUI\"; src:url(\"game://Fixture-Inter.ttf\"); }\n.hud { display:flex; width:100%; height:100%; padding:20px; flex-direction:column; align-items:flex-start; row-gap:12px; }\n.hud-title { font-family:\"FixtureUI\"; font-size:26px; color:#eeeeee; }\n.hud-note { font-family:\"FixtureUI\"; font-size:14px; color:#c0c6ce; }\n.hud-button,.hud-field { font-family:\"FixtureUI\"; font-size:14px; color:#eeeeee; padding:8px; background-color:#303030; }\n.hud-picture { width:96px; height:96px; }\n",
    )?;
    std::fs::write(
        project.assets.join("scenes.prefabs.ron"),
        "[\"game://Composed.scene.ron\",\"game://Cuboid.prefab.ron\",\"game://Other.scene.ron\",\"game://Hud.prefab.ron\"]",
    )?;
    println!("Created source fixture: {}", project.root.display());
    println!(
        "Initialize metadata, then build a game-namespace delivery explicitly before launching."
    );
    Ok(())
}
