//! Admission limits before Bevy's recursive native model loading.

/// Validate a GLB's schema, hierarchy and native scene expansion before loading.
///
/// All nodes, including disconnected ones, must form a forest with at most one
/// parent and at most 128 nodes per root-to-leaf path. At most 4096 JSON nodes,
/// mesh primitives and aggregate scene entities are admitted (separate budgets).
/// Scene entities include every scene container, repeated root/subtree instance,
/// primitive child and possible punctual-light child, regardless of loader flags.
///
/// Returns a diagnostic for malformed GLB/schema, invalid topology or exceeded
/// budgets. The maintained `gltf` parser owns decoding and index validation. This
/// does not validate geometry, external resources or loader settings; Bevy still
/// owns loading. No I/O occurs and the borrowed bytes are not retained or changed.
pub fn validate_glb(bytes: &[u8]) -> Result<(), String> {
    const MAX_DEPTH: usize = 128;
    const MAX_EXPANSION: usize = 4096;
    let malformed = |error| format!("Invalid GLB: {error}");
    let container = gltf::binary::Glb::from_slice(bytes).map_err(malformed)?;
    let json = gltf::json::deserialize::from_slice(&container.json)
        .map_err(|error| format!("Invalid GLB JSON: {error}"))?;
    let document = gltf::Document::from_json(json).map_err(malformed)?;
    if document.as_json().asset.version != "2.0" {
        return Err("GLB requires glTF asset version 2.0".into());
    }
    let nodes: Vec<_> = document.nodes().collect();
    if nodes.len() > MAX_EXPANSION
        || document.meshes().fold(0usize, |count, mesh| {
            count.saturating_add(mesh.primitives().len())
        }) > MAX_EXPANSION
    {
        return Err("GLB native node/primitive budget exceeds 4096".into());
    }

    // Parent admission is global, not limited to the default scene. The schema
    // parser has already validated indices; no recursive graph walk is needed.
    let mut parents = vec![None; nodes.len()];
    for node in &nodes {
        for child in node.children() {
            if parents[child.index()].replace(node.index()).is_some() {
                return Err("GLB node has multiple parents or duplicate child edges".into());
            }
        }
    }
    let mut order: Vec<_> = parents
        .iter()
        .enumerate()
        .filter_map(|(index, parent)| parent.is_none().then_some(index))
        .collect();
    let mut depths = vec![1; nodes.len()];
    let mut cursor = 0;
    while cursor < order.len() {
        let index = order[cursor];
        if depths[index] > MAX_DEPTH {
            return Err("GLB hierarchy depth exceeds 128".into());
        }
        for child in nodes[index].children() {
            depths[child.index()] = depths[index] + 1;
            order.push(child.index());
        }
        cursor += 1;
    }
    if order.len() != nodes.len() {
        return Err("GLB hierarchy contains a cycle".into());
    }

    // Bevy 0.20 loads ALL scenes, each with a container entity, recursively
    // duplicating roots and spawning primitive/light children. Saturate above
    // the policy ceiling so adversarial repetition cannot overflow arithmetic.
    let add = |left: usize, right: usize| left.saturating_add(right).min(MAX_EXPANSION + 1);
    let mut sizes = vec![0; nodes.len()];
    for &index in order.iter().rev() {
        let node = &nodes[index];
        let primitives = node.mesh().map_or(0, |mesh| mesh.primitives().len());
        let mut size = add(1 + usize::from(node.light().is_some()), primitives);
        for child in node.children() {
            size = add(size, sizes[child.index()]);
        }
        sizes[index] = size;
    }
    let mut expanded = 0;
    for scene in document.scenes() {
        expanded = add(expanded, 1);
        for root in scene.nodes() {
            expanded = add(expanded, sizes[root.index()]);
        }
        if expanded > MAX_EXPANSION {
            return Err("GLB aggregate native scene expansion exceeds 4096".into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "model_tests.rs"]
pub(crate) mod tests;
