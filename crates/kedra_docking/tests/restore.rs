//! Persistence admission and malformed-input regression corpus.
#![allow(
    clippy::unwrap_used,
    reason = "Test fixtures assert restoration boundaries"
)]
use kedra_docking::{command::*, model::*, snapshot::*};

fn fixture() -> Snapshot {
    let host = HostId::new();
    let mut w = Workspace::new(host);
    w.execute(Command::Open {
        id: ViewId::new(),
        view: View {
            kind: "unknown.extension".into(),
            restore_key: None,
        },
        destination: Destination::EmptyHost(host),
    })
    .unwrap();
    w.snapshot().clone()
}

#[test]
fn malformed_schema_topology_and_ownership_are_rejected_before_publication() {
    let original = fixture();
    let group = *original.nodes.keys().next().unwrap();
    let host = *original.hosts.keys().next().unwrap();
    let view = *original.views.keys().next().unwrap();
    let mut cases = Vec::new();
    let mut state = original.clone();
    state.version += 1;
    cases.push(state);
    let mut state = original.clone();
    state.hosts.clear();
    cases.push(state);
    let mut state = original.clone();
    state.nodes.clear();
    cases.push(state);
    let mut state = original.clone();
    state.views.clear();
    cases.push(state);
    let mut state = original.clone();
    state.views.get_mut(&view).unwrap().kind.clear();
    cases.push(state);
    let mut state = original.clone();
    state.hosts.get_mut(&host).unwrap().maximized = Some(NodeId::new());
    cases.push(state);
    let mut state = original.clone();
    state
        .hosts
        .insert(HostId::new(), state.hosts[&host].clone());
    cases.push(state);
    let mut state = original.clone();
    state
        .nodes
        .insert(NodeId::new(), state.nodes[&group].clone());
    cases.push(state);
    let mut state = original.clone();
    state.nodes.insert(
        group,
        Node::Group {
            tabs: vec![view, view],
            active: view,
        },
    );
    cases.push(state);
    let mut state = original.clone();
    state.nodes.insert(
        group,
        Node::Group {
            tabs: vec![view],
            active: ViewId::new(),
        },
    );
    cases.push(state);
    let mut state = original.clone();
    state.nodes.insert(
        group,
        Node::Group {
            tabs: vec![],
            active: view,
        },
    );
    cases.push(state);
    let mut state = original.clone();
    state.hidden.insert(
        view,
        Bookmark {
            host,
            group,
            index: 0,
        },
    );
    cases.push(state);
    let mut state = original.clone();
    state.nodes.insert(
        group,
        Node::Split {
            axis: Axis::Vertical,
            fraction: 0.5,
            first: group,
            second: group,
        },
    );
    cases.push(state);
    for state in cases {
        assert!(
            state.clone().into_workspace().is_err(),
            "invalid state accepted: {state:?}"
        );
        // The restoration boundary is equally strict for bytes and values.
        assert!(Snapshot::decode(&serde_json::to_vec(&state).unwrap()).is_err());
    }
    original.validate().unwrap();
}

#[test]
fn schema_and_map_duplicates_are_not_last_writer_wins() {
    let original = fixture();
    let mut json = serde_json::to_value(&original).unwrap();
    json["unexpected"] = true.into();
    assert!(Snapshot::decode(&serde_json::to_vec(&json).unwrap()).is_err());
    let encoded = String::from_utf8(original.encode().unwrap()).unwrap();
    let host_json = serde_json::to_string(&original.hosts).unwrap();
    let entry = &host_json[1..host_json.len() - 1];
    let duplicate = encoded.replacen(&host_json, &format!("{{{entry},{entry}}}"), 1);
    assert_ne!(duplicate, encoded);
    assert!(Snapshot::decode(duplicate.as_bytes()).is_err());
    for bytes in [b"{".as_slice(), b"null", b"{}", b"[]"] {
        assert!(Snapshot::decode(bytes).is_err());
    }
    assert!(Snapshot::decode(&vec![b' '; MAX_BYTES + 1]).is_err());
}

#[test]
fn nested_topology_and_feature_locator_limits_are_enforced() {
    let mut state = fixture();
    let host = *state.hosts.keys().next().unwrap();
    let mut root = state.hosts[&host].root.unwrap();
    for _ in 0..MAX_DEPTH {
        let view = ViewId::new();
        let group = NodeId::new();
        let parent = NodeId::new();
        state.views.insert(
            view,
            View {
                kind: "a".into(),
                restore_key: None,
            },
        );
        state.nodes.insert(
            group,
            Node::Group {
                tabs: vec![view],
                active: view,
            },
        );
        state.nodes.insert(
            parent,
            Node::Split {
                axis: Axis::Horizontal,
                fraction: 0.5,
                first: root,
                second: group,
            },
        );
        root = parent;
    }
    state.hosts.get_mut(&host).unwrap().root = Some(root);
    assert!(state.validate().is_err());
    assert!(Snapshot::decode(&serde_json::to_vec(&state).unwrap()).is_err());
    let mut state = fixture();
    state.views.values_mut().next().unwrap().restore_key = Some("x".repeat(MAX_BYTES));
    assert!(state.encode().is_err());
    for _ in 0..MAX_HOSTS {
        state.hosts.insert(
            HostId::new(),
            Host {
                root: None,
                maximized: None,
            },
        );
    }
    assert!(state.validate().is_err());
}

#[test]
fn diagnostics_on_unvalidated_cyclic_snapshot_are_bounded() {
    let mut state = fixture();
    let root = *state.nodes.keys().next().unwrap();
    state.nodes.insert(
        root,
        Node::Split {
            axis: Axis::Horizontal,
            fraction: 0.5,
            first: root,
            second: root,
        },
    );
    assert_eq!(state.host_of(NodeId::new()), None);
    assert!(state.validate().is_err());
}
