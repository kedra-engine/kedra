//! Bounded tree projection of the existing composed graph, not another composer.
use kedra_scene::{composition::ResolvedGraph, document::NodeAddress};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Row {
    pub address: NodeAddress,
    pub name: String,
    pub parent: Option<usize>,
    pub depth: usize,
    pub children: bool,
    pub boundary: bool,
    pub provenance: String,
}
pub(super) struct Tree {
    pub rows: Vec<Row>,
    pub indices: BTreeMap<NodeAddress, usize>,
}
#[derive(Default)]
pub(super) struct Navigation {
    pub collapsed: BTreeSet<NodeAddress>,
    pub filter: String,
    pub scroll: f32,
    pub unfiltered_scroll: f32,
}
impl Tree {
    pub fn new(graph: &ResolvedGraph) -> Self {
        let mut rows = Vec::with_capacity(graph.nodes.len());
        let mut stack = vec![(graph.root.clone(), None, 0)];
        while let Some((address, parent, depth)) = stack.pop() {
            let node = &graph.nodes[&address];
            let index = rows.len();
            let boundary = node
                .parent
                .as_ref()
                .is_some_and(|p| p.instances != address.instances);
            rows.push(Row {
                address: address.clone(),
                name: node.name.clone(),
                parent,
                depth,
                children: !node.children.is_empty(),
                boundary,
                provenance: format!(
                    "{} · source {} · node {} · instance scope {:?}",
                    node.name, node.origin.document, address.node, address.instances
                ),
            });
            stack.extend(
                node.children
                    .iter()
                    .rev()
                    .map(|child| (child.clone(), Some(index), depth + 1)),
            );
        }
        let indices = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.address.clone(), i))
            .collect();
        Self { rows, indices }
    }
    /// Preorder remains the composition's exact child order. Search includes ancestors.
    pub fn visible(&self, navigation: &Navigation) -> Vec<(usize, bool)> {
        if !navigation.filter.is_empty() {
            let query = unicase::UniCase::unicode(&navigation.filter).to_folded_case();
            let mut included = vec![false; self.rows.len()];
            let matches: Vec<_> = self
                .rows
                .iter()
                .map(|r| {
                    unicase::UniCase::unicode(&r.name)
                        .to_folded_case()
                        .contains(&query)
                })
                .collect();
            for (index, matched) in matches.iter().enumerate() {
                if *matched {
                    included[index] = true;
                }
            }
            for index in (0..self.rows.len()).rev() {
                if included[index]
                    && let Some(parent) = self.rows[index].parent
                {
                    included[parent] = true;
                }
            }
            return included
                .into_iter()
                .enumerate()
                .filter_map(|(i, yes)| yes.then_some((i, !matches[i])))
                .collect();
        }
        let mut visible = vec![true; self.rows.len()];
        for (i, row) in self.rows.iter().enumerate() {
            if let Some(parent) = row.parent {
                visible[i] =
                    visible[parent] && !navigation.collapsed.contains(&self.rows[parent].address);
            }
        }
        visible
            .into_iter()
            .enumerate()
            .filter_map(|(i, yes)| yes.then_some((i, false)))
            .collect()
    }
    pub fn reveal(&self, navigation: &mut Navigation, address: &NodeAddress) {
        navigation.filter.clear();
        let mut index = self.indices.get(address).copied();
        while let Some(i) = index {
            navigation.collapsed.remove(&self.rows[i].address);
            index = self.rows[i].parent;
        }
    }
}
