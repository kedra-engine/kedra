//! Versioned, bounded, validated layout preferences. No filesystem or Save owner.
use crate::{Error, Result, model::*};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Current persisted schema. Other versions fail explicitly, never auto-reset.
pub const VERSION: u32 = 1;
/// Maximum encoded preference input/output size, before parsing or publication.
pub const MAX_BYTES: usize = 1024 * 1024;
/// Maximum live or hidden content instances.
pub const MAX_VIEWS: usize = 4096;
/// Maximum topology nodes (groups and splits combined).
pub const MAX_NODES: usize = 2048;
/// Maximum simultaneous logical surfaces.
pub const MAX_HOSTS: usize = 64;
/// Maximum number of nested nodes, bounding validation and projection recursion.
pub const MAX_DEPTH: usize = 64;
const MAX_TEXT: usize = 4096;

/// Transfer representation, never authority until [`Snapshot::into_workspace`].
///
/// Public fields support migrations and diagnostics. Validation rejects orphans,
/// cycles, shared subtrees, duplicate tab ownership and unknown versions. Unknown
/// *feature kinds* are valid records, to be shown as unavailable by the adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// Exact supported schema version.
    pub version: u32,
    /// Logical surfaces; the adapter decides which one owns application exit.
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub hosts: BTreeMap<HostId, Host>,
    /// Flat arena: persisted input cannot hide topology depth inside JSON nesting.
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub nodes: BTreeMap<NodeId, Node>,
    /// Feature restore descriptors for visible and hidden instances.
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub views: BTreeMap<ViewId, View>,
    /// Hidden tool instances and their last known locations.
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub hidden: BTreeMap<ViewId, Bookmark>,
}
impl Snapshot {
    /// Verify structural/resource invariants without changing the snapshot.
    pub fn validate(&self) -> Result<()> {
        if self.version != VERSION {
            return invalid("unsupported schema version");
        }
        if self.hosts.is_empty() {
            return invalid("workspace must retain a host");
        }
        if self.hosts.len() > MAX_HOSTS {
            return Err(Error::Limit("hosts"));
        }
        if self.nodes.len() > MAX_NODES {
            return Err(Error::Limit("nodes"));
        }
        if self.views.len() > MAX_VIEWS {
            return Err(Error::Limit("views"));
        }
        if self.hidden.len() > MAX_VIEWS {
            return Err(Error::Limit("hidden views"));
        }
        for view in self.views.values() {
            if view.kind.is_empty() {
                return invalid("empty feature kind");
            }
            if view.kind.len() > MAX_TEXT
                || view
                    .restore_key
                    .as_ref()
                    .is_some_and(|key| key.len() > MAX_TEXT)
            {
                return Err(Error::Limit("feature locator length"));
            }
        }
        let mut nodes = BTreeSet::new();
        let mut views = BTreeSet::new();
        for host in self.hosts.values() {
            let mut local = BTreeSet::new();
            if let Some(root) = host.root {
                self.walk(root, 1, &mut nodes, &mut local, &mut views)?;
            }
            if let Some(maximized) = host.maximized
                && (!local.contains(&maximized)
                    || !matches!(self.nodes.get(&maximized), Some(Node::Group { .. })))
            {
                return invalid("maximized group is not owned by its host");
            }
        }
        if nodes.len() != self.nodes.len() {
            return invalid("unreachable topology nodes");
        }
        for view in self.hidden.keys() {
            if !self.views.contains_key(view) || !views.insert(*view) {
                return invalid("invalid or multiply owned hidden view");
            }
        }
        if views.len() != self.views.len() {
            return invalid("unplaced view record");
        }
        Ok(())
    }
    fn walk(
        &self,
        id: NodeId,
        depth: usize,
        seen: &mut BTreeSet<NodeId>,
        local: &mut BTreeSet<NodeId>,
        views: &mut BTreeSet<ViewId>,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(Error::Limit("topology depth"));
        }
        if !seen.insert(id) {
            return invalid("cycle or shared topology node");
        }
        local.insert(id);
        match self
            .nodes
            .get(&id)
            .ok_or_else(|| Error::Invalid("missing child node".into()))?
        {
            Node::Group { tabs, active } => {
                if tabs.is_empty() || !tabs.contains(active) {
                    return invalid("invalid active tab");
                }
                if tabs.len() > MAX_VIEWS {
                    return Err(Error::Limit("tabs"));
                }
                for tab in tabs {
                    if !self.views.contains_key(tab) || !views.insert(*tab) {
                        return invalid("missing or multiply owned view");
                    }
                }
            }
            Node::Split {
                fraction,
                first,
                second,
                ..
            } => {
                valid_fraction(*fraction)?;
                self.walk(*first, depth + 1, seen, local, views)?;
                self.walk(*second, depth + 1, seen, local, views)?;
            }
        }
        Ok(())
    }
    /// Validate then establish a new independent publication lineage.
    pub fn into_workspace(self) -> Result<Workspace> {
        self.validate()?;
        Ok(Workspace::from_valid(self))
    }
    /// Decode bounded JSON using serde, then validate before publication.
    /// JSON parser errors, duplicate map keys and invalid topology are rejected.
    pub fn decode(bytes: &[u8]) -> Result<Workspace> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::Limit("snapshot bytes"));
        }
        // serde_json::Value rejects neither duplicate keys nor unknown fields, so
        // decode directly into the typed schema with unique-map deserializers.
        let state: Self =
            serde_json::from_slice(bytes).map_err(|e| Error::Invalid(e.to_string()))?;
        state.into_workspace()
    }
    /// Encode a valid snapshot; an oversized result is not handed to a writer.
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| Error::Invalid(e.to_string()))?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::Limit("snapshot bytes"));
        }
        Ok(bytes)
    }
    /// Find the sole visible group and index for a content instance.
    pub fn location(&self, view: ViewId) -> Option<(NodeId, usize)> {
        self.nodes.iter().find_map(|(id, node)| match node {
            Node::Group { tabs, .. } => tabs
                .iter()
                .position(|tab| *tab == view)
                .map(|index| (*id, index)),
            _ => None,
        })
    }
    /// Find a node's owning host. Valid snapshots have exactly one owner.
    pub fn host_of(&self, node: NodeId) -> Option<HostId> {
        self.hosts.iter().find_map(|(id, host)| {
            host.root
                .filter(|root| self.contains(*root, node))
                .map(|_| *id)
        })
    }
    pub(crate) fn contains(&self, root: NodeId, target: NodeId) -> bool {
        // Snapshot is an editable transfer value, not guaranteed valid yet.
        // Even diagnostics on a cyclic imported snapshot must terminate.
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            if visited.len() > MAX_NODES {
                return false;
            }
            if node == target {
                return true;
            }
            if let Some(Node::Split { first, second, .. }) = self.nodes.get(&node) {
                pending.extend([*first, *second]);
            }
        }
        false
    }
}
pub(crate) fn valid_fraction(fraction: f32) -> Result<()> {
    if !fraction.is_finite() || fraction <= 0. || fraction >= 1. {
        Err(Error::Fraction)
    } else {
        Ok(())
    }
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::Invalid(message.into()))
}
