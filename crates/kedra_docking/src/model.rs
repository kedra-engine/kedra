//! Identity, bounded topology and publication of validated candidates.
use crate::{Error, Result, command::Command, snapshot::Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);
        impl $name {
            /// Allocate an identity, independent of entities and native handles.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}
identity!(
    HostId,
    "Stable logical surface identity, not a native window handle."
);
identity!(
    NodeId,
    "Stable split or tab-group identity within a workspace."
);
identity!(
    ViewId,
    "Stable content instance identity, distinct from its document."
);

/// Split orientation. Horizontal places children left/right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    /// Children are left and right.
    Horizontal,
    /// Children are above and below.
    Vertical,
}

/// Edge placement relative to an existing group or split subtree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Before the target on its horizontal axis.
    Left,
    /// After the target on its horizontal axis.
    Right,
    /// Before the target on its vertical axis.
    Top,
    /// After the target on its vertical axis.
    Bottom,
}
impl Edge {
    pub(crate) fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            _ => Axis::Vertical,
        }
    }
    pub(crate) fn before(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

/// A visible tab group or a binary split. Only validated snapshots are published.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Node {
    /// Nonempty ordered instances with exactly one active member.
    Group {
        /// Visual tab order; no duplicates across the entire workspace.
        tabs: Vec<ViewId>,
        /// One member of `tabs`.
        active: ViewId,
    },
    /// Two distinct child subtrees. Both have exactly one parent.
    Split {
        /// Direction of subdivision.
        axis: Axis,
        /// Fraction allocated to the first child, strictly between zero and one.
        fraction: f32,
        /// Left/top subtree.
        first: NodeId,
        /// Right/bottom subtree.
        second: NodeId,
    },
}

/// Feature identity needed to restore a view, without document data or UI state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// Registered feature kind. Unknown kinds must not be silently discarded.
    pub kind: String,
    /// Feature-owned restore locator; opaque to docking and never executed.
    pub restore_key: Option<String>,
}

/// Hidden tool's former placement. References may outlive removed groups/hosts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bookmark {
    /// Previous logical surface.
    pub host: HostId,
    /// Previous tab group; absence requires an explicit new destination.
    pub group: NodeId,
    /// Previous tab index, clamped by the caller after inspecting current tabs.
    pub index: usize,
}

/// A logical surface. An empty main surface is legal; adapters retire empty floats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Host {
    /// Topology root, if any.
    pub root: Option<NodeId>,
    /// Temporary maximize projection; does not replace or flatten the tree.
    pub maximized: Option<NodeId>,
}

/// Published layout authority. Not a Bevy resource and not directly deserializable.
///
/// Adapters can wrap it in a resource. Cloning is deliberately not exposed: a
/// restored copy is a new publication lineage, even at the same revision.
pub struct Workspace {
    pub(crate) state: Snapshot,
    lineage: Uuid,
    revision: u64,
}

/// A validated, unpublished operation. Dropping it cancels without side effects.
pub struct Candidate {
    state: Snapshot,
    lineage: Uuid,
    revision: u64,
}
impl Candidate {
    /// Inspect the exact topology which commit would publish.
    pub fn snapshot(&self) -> &Snapshot {
        &self.state
    }
}

impl Workspace {
    /// Start a workspace with one empty logical host. Does not create an OS window.
    pub fn new(host: HostId) -> Self {
        Self::from_valid(Snapshot {
            version: crate::snapshot::VERSION,
            hosts: BTreeMap::from([(
                host,
                Host {
                    root: None,
                    maximized: None,
                },
            )]),
            nodes: BTreeMap::new(),
            views: BTreeMap::new(),
            hidden: BTreeMap::new(),
        })
    }
    pub(crate) fn from_valid(state: Snapshot) -> Self {
        Self {
            state,
            lineage: Uuid::new_v4(),
            revision: 0,
        }
    }
    /// Current immutable topology, suitable for projection and persistence.
    pub fn snapshot(&self) -> &Snapshot {
        &self.state
    }
    /// Publication counter. No-op commits do not advance it.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub(crate) fn stamp(&self) -> (Uuid, u64) {
        (self.lineage, self.revision)
    }
    /// Build and validate a complete candidate without touching this workspace.
    pub fn prepare(&self, command: Command) -> Result<Candidate> {
        let mut state = self.state.clone();
        command.apply(&mut state)?;
        state.validate()?;
        Ok(Candidate {
            state,
            lineage: self.lineage,
            revision: self.revision,
        })
    }
    /// Atomically publish a current candidate. Returns whether state changed.
    ///
    /// Stale/foreign candidates and counter exhaustion leave state unchanged.
    pub fn commit(&mut self, candidate: Candidate) -> Result<bool> {
        if candidate.lineage != self.lineage || candidate.revision != self.revision {
            return Err(Error::Stale);
        }
        if candidate.state == self.state {
            return Ok(false);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Error::Limit("revision"))?;
        self.state = candidate.state;
        self.revision = revision;
        Ok(true)
    }
    /// Prepare and immediately publish a command, with identical validation.
    pub fn execute(&mut self, command: Command) -> Result<bool> {
        let candidate = self.prepare(command)?;
        self.commit(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Destination;

    #[test]
    fn revision_exhaustion_does_not_publish_but_noop_remains_valid() {
        let host = HostId::new();
        let mut workspace = Workspace::new(host);
        workspace.revision = u64::MAX;
        let before = workspace.snapshot().clone();
        let result = workspace.execute(Command::Open {
            id: ViewId::new(),
            view: View {
                kind: "test.exhaustion".into(),
                restore_key: None,
            },
            destination: Destination::EmptyHost(host),
        });
        assert_eq!(result, Err(Error::Limit("revision")));
        assert_eq!(workspace.snapshot(), &before);
        assert_eq!(workspace.revision(), u64::MAX);
        assert_eq!(
            workspace.execute(Command::Maximize { host, group: None }),
            Ok(false)
        );
        assert_eq!(workspace.revision(), u64::MAX);
    }
}
