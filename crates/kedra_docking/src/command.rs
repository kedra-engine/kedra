//! Explicit topology operations. Close approval and native readiness precede commit.
use crate::{
    Error, Result,
    model::*,
    snapshot::{Snapshot, valid_fraction},
};

/// Ordered content to relocate. Group moves preserve the source active view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Move one visible view, preserving its identity.
    View(ViewId),
    /// Move every tab of one group, preserving order and active view.
    Group(NodeId),
}

/// Destination in the pre-operation topology. No coordinate heuristics are stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Destination {
    /// Insert at a boundary of the current tab list (`0..=len`). Same-group
    /// moves adjust this pre-removal boundary rather than shifting twice.
    Tabs {
        /// Existing tab group.
        group: NodeId,
        /// Boundary in the destination's pre-removal tab list.
        index: usize,
    },
    /// Split an existing group or subtree. `fraction` is the new content's share.
    Edge {
        /// Existing group or split subtree.
        target: NodeId,
        /// Which side receives the selection.
        edge: Edge,
        /// Requested share before layout minima are applied by the adapter.
        fraction: f32,
    },
    /// Fill an already-created empty logical host.
    EmptyHost(HostId),
}

/// One complete model change. Every variant prepares/validates before publication.
#[derive(Clone, Debug)]
pub enum Command {
    /// Register and place a new feature-owned view; duplicate IDs are rejected.
    Open {
        /// Stable instance identity, allocated by the feature/adapter.
        id: ViewId,
        /// Restore metadata, not content or document authority.
        view: View,
        /// Initial placement.
        destination: Destination,
    },
    /// Move visible content without allocating a new view identity.
    Move {
        /// One view or whole group.
        selection: Selection,
        /// Resulting placement.
        destination: Destination,
    },
    /// After native readiness, create a logical floating host and move into it.
    /// Preparation alone does not claim that an OS window has been created.
    Float {
        /// One view or whole group.
        selection: Selection,
        /// Fresh logical host identity.
        host: HostId,
    },
    /// Hide a tool after feature approval, retaining descriptor and bookmark.
    Hide(ViewId),
    /// Reopen hidden content at an explicit, currently valid destination.
    Show {
        /// Hidden instance identity.
        view: ViewId,
        /// Caller resolves any stale bookmark before issuing this command.
        destination: Destination,
    },
    /// Forget an approved view, visible or hidden. Does not close its document.
    Remove(ViewId),
    /// Activate a visible tab, without reconstructing or relocating content.
    Activate(ViewId),
    /// Change one split. Interactive adapters use a candidate session until release.
    Resize {
        /// Existing split identity.
        split: NodeId,
        /// First child's share, strictly between zero and one.
        fraction: f32,
    },
    /// Maximize a group or restore the host's full tree without topology changes.
    Maximize {
        /// Owning logical surface.
        host: HostId,
        /// Group to maximize, or None to restore.
        group: Option<NodeId>,
    },
    /// Retire an empty host after content transfer. Last host cannot be removed.
    RetireHost(HostId),
    /// Hide all feature-approved tool views on a floating host and retire it as
    /// one model transaction. The adapter must approve every view beforehand.
    HideHost(HostId),
}

struct Payload {
    tabs: Vec<ViewId>,
    active: ViewId,
    group: Option<NodeId>,
}

impl Command {
    pub(crate) fn apply(self, state: &mut Snapshot) -> Result<()> {
        match self {
            Self::Open {
                id,
                view,
                destination,
            } => {
                if state.views.contains_key(&id) {
                    return Err(Error::Invalid("duplicate view ID".into()));
                }
                check_destination(state, destination)?;
                state.views.insert(id, view);
                place(
                    state,
                    Payload {
                        tabs: vec![id],
                        active: id,
                        group: None,
                    },
                    destination,
                )?;
            }
            Self::Move {
                selection,
                destination,
            } => relocate(state, selection, destination)?,
            Self::Float { selection, host } => {
                if state.hosts.contains_key(&host) {
                    return Err(Error::Invalid("duplicate host ID".into()));
                }
                state.hosts.insert(
                    host,
                    Host {
                        root: None,
                        maximized: None,
                    },
                );
                relocate(state, selection, Destination::EmptyHost(host))?;
            }
            Self::Hide(view) => {
                let (group, index) = state.location(view).ok_or(Error::Missing)?;
                let host = state.host_of(group).ok_or(Error::Missing)?;
                detach(state, Selection::View(view))?;
                state.hidden.insert(view, Bookmark { host, group, index });
            }
            Self::Show { view, destination } => {
                check_destination(state, destination)?;
                state.hidden.remove(&view).ok_or(Error::Missing)?;
                place(
                    state,
                    Payload {
                        tabs: vec![view],
                        active: view,
                        group: None,
                    },
                    destination,
                )?;
            }
            Self::Remove(view) => {
                if state.hidden.remove(&view).is_none() {
                    detach(state, Selection::View(view))?;
                }
                state.views.remove(&view).ok_or(Error::Missing)?;
            }
            Self::Activate(view) => {
                let (group, _) = state.location(view).ok_or(Error::Missing)?;
                let Some(Node::Group { active, .. }) = state.nodes.get_mut(&group) else {
                    return Err(Error::Missing);
                };
                *active = view;
            }
            Self::Resize { split, fraction } => {
                valid_fraction(fraction)?;
                let Some(Node::Split {
                    fraction: current, ..
                }) = state.nodes.get_mut(&split)
                else {
                    return Err(Error::Missing);
                };
                *current = fraction;
            }
            Self::Maximize { host, group } => {
                if let Some(group) = group
                    && (state.host_of(group) != Some(host)
                        || !matches!(state.nodes.get(&group), Some(Node::Group { .. })))
                {
                    return Err(Error::Missing);
                }
                state.hosts.get_mut(&host).ok_or(Error::Missing)?.maximized = group;
            }
            Self::RetireHost(host) => {
                if state.hosts.get(&host).ok_or(Error::Missing)?.root.is_some() {
                    return Err(Error::Invalid("host still owns content".into()));
                }
                if state.hosts.len() == 1 {
                    return Err(Error::Invalid("cannot retire last host".into()));
                }
                state.hosts.remove(&host);
            }
            Self::HideHost(host) => {
                if !state.hosts.contains_key(&host) {
                    return Err(Error::Missing);
                }
                let views: Vec<_> = state
                    .views
                    .keys()
                    .filter(|view| {
                        state
                            .location(**view)
                            .is_some_and(|(group, _)| state.host_of(group) == Some(host))
                    })
                    .copied()
                    .collect();
                for view in views {
                    Self::Hide(view).apply(state)?;
                }
                Self::RetireHost(host).apply(state)?;
            }
        }
        // Group retirement invalidates projection state, not the surviving tree.
        let invalid_maxima = state
            .hosts
            .iter()
            .filter_map(|(host_id, host)| {
                host.maximized
                    .filter(|node| state.host_of(*node) != Some(*host_id))
                    .map(|_| *host_id)
            })
            .collect::<Vec<_>>();
        for host in invalid_maxima {
            if let Some(host) = state.hosts.get_mut(&host) {
                host.maximized = None;
            }
        }
        Ok(())
    }
}

fn check_destination(state: &Snapshot, destination: Destination) -> Result<()> {
    match destination {
        Destination::Tabs { group, index } => {
            let Some(Node::Group { tabs, .. }) = state.nodes.get(&group) else {
                return Err(Error::Missing);
            };
            if index > tabs.len() {
                return Err(Error::Index);
            }
        }
        Destination::Edge {
            target, fraction, ..
        } => {
            valid_fraction(fraction)?;
            if !state.nodes.contains_key(&target) {
                return Err(Error::Missing);
            }
        }
        Destination::EmptyHost(host) => {
            if state.hosts.get(&host).ok_or(Error::Missing)?.root.is_some() {
                return Err(Error::Invalid("destination host is not empty".into()));
            }
        }
    }
    Ok(())
}

fn relocate(
    state: &mut Snapshot,
    selection: Selection,
    mut destination: Destination,
) -> Result<()> {
    check_destination(state, destination)?;
    let (source, removed) = match selection {
        Selection::View(view) => {
            let (group, index) = state.location(view).ok_or(Error::Missing)?;
            (group, Some(index))
        }
        Selection::Group(group) => {
            if !matches!(state.nodes.get(&group), Some(Node::Group { .. })) {
                return Err(Error::Missing);
            }
            (group, None)
        }
    };
    if let Destination::Tabs { group, index } = &mut destination
        && *group == source
    {
        let index_of_view = removed.ok_or(Error::SelfTarget)?;
        // Reordering the only tab is a valid no-op; do not remove its group.
        let Some(Node::Group { tabs, .. }) = state.nodes.get_mut(group) else {
            return Err(Error::Missing);
        };
        let view = tabs.remove(index_of_view);
        if index_of_view < *index {
            *index -= 1;
        }
        tabs.insert(*index, view);
        return Ok(());
    }
    if let Destination::Edge { target, .. } = destination
        && target == source
        && (removed.is_none()
            || matches!(state.nodes.get(&source), Some(Node::Group { tabs, .. }) if tabs.len() == 1))
    {
        return Err(Error::SelfTarget);
    }
    // A target subtree may be the source group's parent. Removing the last tab
    // collapses that parent onto its sibling. The requested edge still applies
    // to the same remaining target content, now reached through the sibling ID.
    let collapse = if removed.is_none()
        || matches!(state.nodes.get(&source), Some(Node::Group { tabs, .. }) if tabs.len() == 1)
    {
        parent_and_sibling(state, source)
    } else {
        None
    };
    let payload = detach(state, selection)?;
    if let (Destination::Edge { target, .. }, Some((parent, sibling))) =
        (&mut destination, collapse)
        && *target == parent
    {
        *target = sibling;
    }
    check_destination(state, destination)?;
    place(state, payload, destination)
}

fn detach(state: &mut Snapshot, selection: Selection) -> Result<Payload> {
    let (group, selected) = match selection {
        Selection::View(view) => (state.location(view).ok_or(Error::Missing)?.0, Some(view)),
        Selection::Group(group) => (group, None),
    };
    let Some(Node::Group { tabs, active }) = state.nodes.get_mut(&group) else {
        return Err(Error::Missing);
    };
    let payload = if let Some(view) = selected {
        let index = tabs
            .iter()
            .position(|id| *id == view)
            .ok_or(Error::Missing)?;
        tabs.remove(index);
        if *active == view && !tabs.is_empty() {
            *active = tabs[index.min(tabs.len() - 1)];
        }
        Payload {
            tabs: vec![view],
            active: view,
            group: tabs.is_empty().then_some(group),
        }
    } else {
        Payload {
            tabs: std::mem::take(tabs),
            active: *active,
            group: Some(group),
        }
    };
    if tabs.is_empty() {
        remove_group(state, group)?;
    }
    Ok(payload)
}

fn remove_group(state: &mut Snapshot, group: NodeId) -> Result<()> {
    let parent = parent_and_sibling(state, group);
    if let Some((parent, sibling)) = parent {
        replace_link(state, parent, Some(sibling))?;
        state.nodes.remove(&parent);
    } else {
        replace_link(state, group, None)?;
    }
    state.nodes.remove(&group);
    Ok(())
}

fn parent_and_sibling(state: &Snapshot, group: NodeId) -> Option<(NodeId, NodeId)> {
    state.nodes.iter().find_map(|(id, node)| match node {
        Node::Split { first, second, .. } if *first == group => Some((*id, *second)),
        Node::Split { first, second, .. } if *second == group => Some((*id, *first)),
        _ => None,
    })
}

fn replace_link(state: &mut Snapshot, old: NodeId, new: Option<NodeId>) -> Result<()> {
    for host in state.hosts.values_mut() {
        if host.root == Some(old) {
            host.root = new;
            return Ok(());
        }
    }
    let new = new.ok_or(Error::Missing)?;
    for node in state.nodes.values_mut() {
        if let Node::Split { first, second, .. } = node {
            if *first == old {
                *first = new;
                return Ok(());
            }
            if *second == old {
                *second = new;
                return Ok(());
            }
        }
    }
    Err(Error::Missing)
}

fn place(state: &mut Snapshot, payload: Payload, destination: Destination) -> Result<()> {
    if let Destination::Tabs { group, index } = destination {
        let Some(Node::Group { tabs, active }) = state.nodes.get_mut(&group) else {
            return Err(Error::Missing);
        };
        if index > tabs.len() {
            return Err(Error::Index);
        }
        tabs.splice(index..index, payload.tabs);
        *active = payload.active;
        return Ok(());
    }
    let group = payload.group.unwrap_or_else(|| fresh_node(state));
    state.nodes.insert(
        group,
        Node::Group {
            tabs: payload.tabs,
            active: payload.active,
        },
    );
    match destination {
        Destination::Edge {
            target,
            edge,
            fraction,
        } => {
            let split = fresh_node(state);
            replace_link(state, target, Some(split))?;
            let (first, second, fraction) = if edge.before() {
                (group, target, fraction)
            } else {
                (target, group, 1. - fraction)
            };
            state.nodes.insert(
                split,
                Node::Split {
                    axis: edge.axis(),
                    fraction,
                    first,
                    second,
                },
            );
        }
        Destination::EmptyHost(host) => {
            state.hosts.get_mut(&host).ok_or(Error::Missing)?.root = Some(group)
        }
        Destination::Tabs { .. } => return Err(Error::Missing),
    }
    Ok(())
}
fn fresh_node(state: &Snapshot) -> NodeId {
    loop {
        let id = NodeId::new();
        if !state.nodes.contains_key(&id) {
            return id;
        }
    }
}
