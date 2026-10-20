//! Deterministic resolver-work checks; no timing-based performance assertions.
use super::*;

fn reduced_chain(depth: usize) -> crate::Result<(Catalog, DocumentId)> {
    let mut catalog = Catalog::default();
    let mut last = PrefabDocument::new("Leaf");
    catalog.insert(last.clone())?;
    for _ in 0..depth {
        let mut next = PrefabDocument::new("Reduced branch");
        for _ in 0..2 {
            let site = InstanceId::new();
            next.instances.push(NestedInstance {
                id: site,
                asset: last.id,
                parent: next.root.into(),
                bindings: BTreeMap::new(),
                overrides: vec![],
            });
            next.overrides.push(Operation::RemoveNestedInstance(site));
        }
        catalog.insert(next.clone())?;
        last = next;
    }
    Ok((catalog, last.id))
}

#[test]
fn repeated_reduced_subgraphs_resolve_once_and_keep_exact_transitive_sources() -> crate::Result<()>
{
    let (mut catalog, id) = reduced_chain(32)?;
    let mut state = Resolution::default();
    let graph = catalog.resolve_inner(id, &mut state)?;
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(state.cache.len(), 33);
    assert_eq!(graph.sources.len(), 33);
    assert_eq!(graph.dependencies.len(), 33);
    assert!(
        state.work < 4096,
        "linear cached traversal exceeded expected work"
    );
    let leaf = *catalog
        .documents
        .keys()
        .find(|key| catalog.documents[key].instances.is_empty())
        .ok_or_else(|| failure("leaf missing"))?;
    catalog
        .documents
        .get_mut(&leaf)
        .ok_or_else(|| failure("leaf missing"))?
        .title = "Changed shadowed source".into();
    assert_ne!(
        catalog.resolve(id)?,
        graph,
        "a fresh request reused stale source values"
    );
    Ok(())
}

#[test]
fn cached_subtree_height_cannot_bypass_depth_or_cycle_guards() -> crate::Result<()> {
    let (mut catalog, subtree) = reduced_chain(2)?;
    let mut last = catalog.documents[&subtree].clone();
    for _ in 0..62 {
        let next = last.variant("Deep");
        catalog.insert(next.clone())?;
        last = next;
    }
    let mut state = Resolution::default();
    catalog.resolve_inner(subtree, &mut state)?;
    assert!(catalog.resolve_inner(last.id, &mut state).is_err());
    assert!(state.stack.is_empty());
    assert_eq!(state.depth, 0);
    catalog
        .documents
        .get_mut(&subtree)
        .ok_or_else(|| failure("subtree missing"))?
        .base = Some(last.id);
    assert!(
        catalog.resolve(last.id).is_err(),
        "fresh request hid a cycle"
    );
    Ok(())
}

#[test]
fn instance_overrides_mutate_clones_not_cached_sources_or_siblings() -> crate::Result<()> {
    let child = PrefabDocument::new("Original");
    let mut parent = PrefabDocument::new("Parent");
    for name in [Some("First"), Some("Second"), None] {
        parent.instances.push(NestedInstance {
            id: InstanceId::new(),
            asset: child.id,
            parent: parent.root.into(),
            bindings: BTreeMap::new(),
            overrides: name
                .into_iter()
                .map(|name| Operation::RenameNode {
                    node: child.root.into(),
                    name: name.into(),
                })
                .collect(),
        });
    }
    let mut catalog = Catalog::default();
    catalog.insert(child.clone())?;
    catalog.insert(parent.clone())?;
    let mut state = Resolution::default();
    let graph = catalog.resolve_inner(parent.id, &mut state)?;
    for (site, expected) in parent.instances.iter().zip(["First", "Second", "Original"]) {
        assert_eq!(
            graph.nodes[&NodeAddress {
                instances: vec![site.id],
                node: child.root
            }]
                .name,
            expected
        );
    }
    assert_eq!(
        state.cache[&child.id].0.nodes[&child.root.into()].name,
        "Original"
    );
    assert_eq!(graph.sources[&child.id].as_ref(), &child);
    Ok(())
}

#[test]
fn inline_override_sites_contribute_to_depth_even_when_assets_are_cached() -> crate::Result<()> {
    for overflow in [false, true] {
        let child = PrefabDocument::new("Child");
        let mut overrides = vec![];
        for _ in 0..if overflow { 63 } else { 62 } {
            overrides = vec![Operation::AddNestedInstance(NestedInstance {
                id: InstanceId::new(),
                asset: child.id,
                parent: child.root.into(),
                bindings: BTreeMap::new(),
                overrides,
            })];
        }
        let mut parent = PrefabDocument::new("Parent");
        parent.instances.push(NestedInstance {
            id: InstanceId::new(),
            asset: child.id,
            parent: parent.root.into(),
            bindings: BTreeMap::new(),
            overrides,
        });
        let mut catalog = Catalog::default();
        catalog.insert(child)?;
        catalog.insert(parent.clone())?;
        let result = catalog.resolve(parent.id);
        if overflow {
            assert!(result.is_err());
        } else {
            assert_eq!(result?.nodes.len(), 64);
        }
    }
    Ok(())
}
