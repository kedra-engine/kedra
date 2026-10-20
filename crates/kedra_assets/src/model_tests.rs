use super::validate_glb;
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(crate) fn glb(document: Value) -> TestResult<Vec<u8>> {
    Ok(gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: std::borrow::Cow::Owned(serde_json::to_vec(&document)?),
        bin: None,
    }
    .to_vec()?)
}

fn document(nodes: Value, scenes: Value) -> Value {
    json!({"asset":{"version":"2.0"}, "nodes":nodes, "scenes":scenes})
}

#[test]
fn valid_static_model_and_exact_depth_limit_are_admitted() -> TestResult {
    validate_glb(&glb(document(json!([{}]), json!([{"nodes":[0]}])))?)?;
    validate_glb(&chain(128)?)?;
    Ok(())
}

fn chain(length: usize) -> TestResult<Vec<u8>> {
    let nodes: Vec<_> = (0..length)
        .map(|i| {
            if i + 1 == length {
                json!({})
            } else {
                json!({"children":[i+1]})
            }
        })
        .collect();
    glb(document(json!(nodes), json!([{"nodes":[0]}])))
}

#[test]
fn shared_descendants_and_duplicate_children_are_rejected() -> TestResult {
    for nodes in [
        json!([{"children":[2]}, {"children":[2]}, {}]),
        json!([{"children":[1,1]}, {}]),
    ] {
        assert!(validate_glb(&glb(document(nodes, json!([])))?).is_err());
    }
    Ok(())
}

#[test]
fn cycles_including_disconnected_cycles_are_rejected() -> TestResult {
    for nodes in [
        json!([{"children":[0]}]),
        json!([{}, {"children":[2]}, {"children":[1]}]),
    ] {
        assert!(validate_glb(&glb(document(nodes, json!([{"nodes":[0]}])))?).is_err());
    }
    Ok(())
}

#[test]
fn excessive_depth_is_rejected() -> TestResult {
    assert!(validate_glb(&chain(129)?).is_err());
    Ok(())
}

#[test]
fn repeated_scenes_roots_and_scene_containers_consume_expansion_budget() -> TestResult {
    for scenes in [
        json!(vec![json!({"nodes":[0]}); 2049]),
        json!([{"nodes":vec![0;4096]}]),
        json!(vec![json!({}); 4097]),
    ] {
        assert!(validate_glb(&glb(document(json!([{}]), scenes))?).is_err());
    }
    assert!(
        validate_glb(&glb(document(
            json!([{}]),
            json!(vec![json!({"nodes":[0]}); 2048])
        ))?)
        .is_ok()
    );
    Ok(())
}

#[test]
fn mesh_primitives_are_counted_per_scene_instance() -> TestResult {
    let mut model = document(json!([{"mesh":0}]), json!(vec![json!({"nodes":[0]}); 2048]));
    model["meshes"] = json!([{"primitives":[{"attributes":{"POSITION":0}}]}]);
    model["accessors"] = json!([{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,1]}]);
    model["bufferViews"] = json!([{"buffer":0,"byteLength":36}]);
    model["buffers"] = json!([{"byteLength":36}]);
    let mut valid = model.clone();
    valid["scenes"] = json!([{"nodes":[0]}]);
    validate_glb(&glb(valid)?)?;
    assert!(validate_glb(&glb(model)?).is_err());
    Ok(())
}

#[test]
fn punctual_light_children_and_disconnected_native_assets_are_bounded() -> TestResult {
    let mut model = document(
        json!([{"extensions":{"KHR_lights_punctual":{"light":0}}}]),
        json!([{"nodes":[0]}]),
    );
    model["extensions"] = json!({"KHR_lights_punctual":{"lights":[{"type":"point"}]}});
    model["extensionsUsed"] = json!(["KHR_lights_punctual"]);
    validate_glb(&glb(model.clone())?)?;
    model["scenes"] = json!(vec![json!({"nodes":[0]}); 1366]);
    assert!(validate_glb(&glb(model)?).is_err());
    assert!(validate_glb(&glb(document(json!(vec![json!({}); 4097]), json!([])))?).is_err());
    Ok(())
}

#[test]
fn malformed_container_schema_and_indices_are_rejected() -> TestResult {
    for bytes in [
        b"not a GLB".to_vec(),
        glb(document(json!([{"children":[5]}]), json!([])))?,
        glb(json!({"asset":{"version":"1.0"}}))?,
    ] {
        assert!(validate_glb(&bytes).is_err());
    }
    Ok(())
}
