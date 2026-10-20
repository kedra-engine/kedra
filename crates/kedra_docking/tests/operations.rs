//! Topology, identity and unpublished interaction regression corpus.
#![allow(clippy::unwrap_used, reason = "Test fixtures assert operation results")]
use kedra_docking::{Error, command::*, model::*, session::Session, snapshot::Snapshot};

fn view() -> View {
    View {
        kind: "test.日本語".into(),
        restore_key: Some("document:Кедра".into()),
    }
}
fn open(workspace: &mut Workspace, destination: Destination) -> ViewId {
    let id = ViewId::new();
    workspace
        .execute(Command::Open {
            id,
            view: view(),
            destination,
        })
        .unwrap();
    id
}
fn fixture() -> (Workspace, HostId, NodeId, ViewId, ViewId, ViewId) {
    let host = HostId::new();
    let mut w = Workspace::new(host);
    let a = open(&mut w, Destination::EmptyHost(host));
    let group = w.snapshot().location(a).unwrap().0;
    let b = open(&mut w, Destination::Tabs { group, index: 1 });
    let c = open(&mut w, Destination::Tabs { group, index: 2 });
    (w, host, group, a, b, c)
}
fn tabs(w: &Workspace, group: NodeId) -> (Vec<ViewId>, ViewId) {
    match &w.snapshot().nodes[&group] {
        Node::Group { tabs, active } => (tabs.clone(), *active),
        _ => panic!("expected group"),
    }
}

#[test]
fn reordering_uses_pre_removal_boundaries_and_noop_does_not_publish() {
    let (mut w, _, group, a, b, c) = fixture();
    w.execute(Command::Move {
        selection: Selection::View(a),
        destination: Destination::Tabs { group, index: 3 },
    })
    .unwrap();
    assert_eq!(tabs(&w, group), (vec![b, c, a], c));
    let revision = w.revision();
    assert!(
        !w.execute(Command::Move {
            selection: Selection::View(a),
            destination: Destination::Tabs { group, index: 2 }
        })
        .unwrap()
    );
    assert_eq!(w.revision(), revision);
    w.execute(Command::Move {
        selection: Selection::View(a),
        destination: Destination::Tabs { group, index: 0 },
    })
    .unwrap();
    assert_eq!(tabs(&w, group).0, vec![a, b, c]);
    let before = w.snapshot().clone();
    assert_eq!(
        w.execute(Command::Move {
            selection: Selection::View(a),
            destination: Destination::Tabs { group, index: 4 }
        }),
        Err(Error::Index)
    );
    assert_eq!(w.snapshot(), &before);
}

#[test]
fn split_float_redock_preserve_view_and_group_identity_and_collapse_empty_branches() {
    let (mut w, main, original, a, b, c) = fixture();
    w.execute(Command::Move {
        selection: Selection::View(a),
        destination: Destination::Edge {
            target: original,
            edge: Edge::Right,
            fraction: 0.3,
        },
    })
    .unwrap();
    let moved = w.snapshot().location(a).unwrap().0;
    assert_ne!(moved, original);
    assert_eq!(tabs(&w, original).0, vec![b, c]);
    let float = HostId::new();
    w.execute(Command::Float {
        selection: Selection::Group(moved),
        host: float,
    })
    .unwrap();
    assert_eq!(w.snapshot().hosts[&float].root, Some(moved));
    assert_eq!(w.snapshot().hosts[&main].root, Some(original));
    assert_eq!(w.snapshot().views[&a], view());
    w.execute(Command::Move {
        selection: Selection::Group(moved),
        destination: Destination::Tabs {
            group: original,
            index: 1,
        },
    })
    .unwrap();
    assert_eq!(tabs(&w, original), (vec![b, a, c], a));
    assert!(w.snapshot().hosts[&float].root.is_none());
    w.execute(Command::RetireHost(float)).unwrap();
    assert_eq!(w.snapshot().nodes.len(), 1);
    assert_eq!(w.snapshot().views.len(), 3);
}

#[test]
fn whole_group_keeps_order_active_and_identity_across_float_and_edge_redock() {
    let (mut w, main, group, a, b, c) = fixture();
    w.execute(Command::Activate(b)).unwrap();
    let float = HostId::new();
    w.execute(Command::Maximize {
        host: main,
        group: Some(group),
    })
    .unwrap();
    w.execute(Command::Float {
        selection: Selection::Group(group),
        host: float,
    })
    .unwrap();
    assert_eq!(w.snapshot().hosts[&main].maximized, None);
    assert_eq!(tabs(&w, group), (vec![a, b, c], b));
    let d = open(&mut w, Destination::EmptyHost(main));
    let target = w.snapshot().location(d).unwrap().0;
    w.execute(Command::Move {
        selection: Selection::Group(group),
        destination: Destination::Edge {
            target,
            edge: Edge::Top,
            fraction: 0.4,
        },
    })
    .unwrap();
    assert_eq!(tabs(&w, group), (vec![a, b, c], b));
    assert_eq!(w.snapshot().host_of(group), Some(main));
}

#[test]
fn last_tab_self_edge_is_rejected_atomically_in_root_and_nested_leaf() {
    let host = HostId::new();
    let mut w = Workspace::new(host);
    let a = open(&mut w, Destination::EmptyHost(host));
    let root = w.snapshot().location(a).unwrap().0;
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        let before = w.snapshot().clone();
        let revision = w.revision();
        assert_eq!(
            w.execute(Command::Move {
                selection: Selection::View(a),
                destination: Destination::Edge {
                    target: root,
                    edge,
                    fraction: 0.5
                }
            }),
            Err(Error::SelfTarget)
        );
        assert_eq!(w.snapshot(), &before);
        assert_eq!(w.revision(), revision);
    }
    let b = open(
        &mut w,
        Destination::Edge {
            target: root,
            edge: Edge::Left,
            fraction: 0.5,
        },
    );
    let nested = w.snapshot().location(b).unwrap().0;
    let before = w.snapshot().clone();
    assert_eq!(
        w.execute(Command::Move {
            selection: Selection::View(b),
            destination: Destination::Edge {
                target: nested,
                edge: Edge::Bottom,
                fraction: 0.5
            }
        }),
        Err(Error::SelfTarget)
    );
    assert_eq!(w.snapshot(), &before);
}

#[test]
fn hidden_tools_and_removed_views_are_different_and_bookmarks_can_outlive_topology() {
    let (mut w, main, group, a, b, c) = fixture();
    w.execute(Command::Hide(a)).unwrap();
    assert_eq!(
        w.snapshot().hidden[&a],
        Bookmark {
            host: main,
            group,
            index: 0
        }
    );
    assert!(w.snapshot().views.contains_key(&a));
    assert!(
        w.execute(Command::Show {
            view: a,
            destination: Destination::Tabs {
                group: NodeId::new(),
                index: 0
            }
        })
        .is_err()
    );
    assert!(w.snapshot().hidden.contains_key(&a));
    w.execute(Command::Show {
        view: a,
        destination: Destination::Tabs { group, index: 2 },
    })
    .unwrap();
    assert_eq!(tabs(&w, group), (vec![b, c, a], a));
    for id in [a, b, c] {
        w.execute(Command::Hide(id)).unwrap();
    }
    assert!(w.snapshot().hosts[&main].root.is_none());
    assert!(w.snapshot().nodes.is_empty());
    w.snapshot().validate().unwrap();
    w.execute(Command::Show {
        view: b,
        destination: Destination::EmptyHost(main),
    })
    .unwrap();
    w.execute(Command::Remove(a)).unwrap();
    assert!(!w.snapshot().views.contains_key(&a));
    assert!(w.snapshot().views.contains_key(&c));
    assert!(w.execute(Command::RetireHost(main)).is_err());
}

#[test]
fn candidates_are_revision_and_lineage_bound_and_cancel_has_no_side_effects() {
    let (mut w, _, group, a, _, c) = fixture();
    let candidate = w.prepare(Command::Activate(a)).unwrap();
    let other_candidate = w.prepare(Command::Activate(a)).unwrap();
    let mut other = w.snapshot().clone().into_workspace().unwrap();
    assert_eq!(other.commit(other_candidate), Err(Error::Stale));
    w.execute(Command::Activate(a)).unwrap();
    assert_eq!(w.commit(candidate), Err(Error::Stale));
    let before = w.snapshot().clone();
    let revision = w.revision();
    {
        let mut session = Session::new(&w);
        session
            .update(
                &w,
                Command::Move {
                    selection: Selection::View(c),
                    destination: Destination::Edge {
                        target: group,
                        edge: Edge::Left,
                        fraction: 0.4,
                    },
                },
            )
            .unwrap();
        assert_eq!(w.snapshot(), &before);
    }
    assert_eq!(w.snapshot(), &before);
    assert_eq!(w.revision(), revision);
    let mut session = Session::new(&w);
    session.update(&w, Command::Activate(c)).unwrap();
    assert!(
        session
            .update(&w, Command::Activate(ViewId::new()))
            .is_err()
    );
    assert!(session.candidate().is_none());
    assert_eq!(session.finish(&mut w), Err(Error::Missing));
    assert_eq!(w.snapshot(), &before);
}

#[test]
fn resize_preview_does_not_accumulate_and_maximize_does_not_destroy_topology() {
    let (mut w, host, group, a, _, _) = fixture();
    w.execute(Command::Move {
        selection: Selection::View(a),
        destination: Destination::Edge {
            target: group,
            edge: Edge::Bottom,
            fraction: 0.4,
        },
    })
    .unwrap();
    let root = w.snapshot().hosts[&host].root.unwrap();
    let original = w.snapshot().nodes.clone();
    w.execute(Command::Maximize {
        host,
        group: Some(group),
    })
    .unwrap();
    assert_eq!(w.snapshot().nodes, original);
    w.execute(Command::Maximize { host, group: None }).unwrap();
    let mut session = Session::new(&w);
    for fraction in [0.3, 0.7] {
        session
            .update(
                &w,
                Command::Resize {
                    split: root,
                    fraction,
                },
            )
            .unwrap();
        assert_eq!(w.snapshot().nodes, original);
    }
    let candidate = session.candidate().unwrap().snapshot().clone();
    session.finish(&mut w).unwrap();
    assert_eq!(w.snapshot(), &candidate);
    for fraction in [f32::NAN, f32::INFINITY, -1., 0., 1., 2.] {
        let before = w.snapshot().clone();
        assert_eq!(
            w.execute(Command::Resize {
                split: root,
                fraction
            }),
            Err(Error::Fraction)
        );
        assert_eq!(w.snapshot(), &before);
    }
}

#[test]
fn roundtrip_keeps_multiple_views_of_one_document_and_unavailable_feature_kinds() {
    let (w, _, _, _, _, _) = fixture();
    let encoded = w.snapshot().encode().unwrap();
    let restored = Snapshot::decode(&encoded).unwrap();
    assert_eq!(restored.snapshot(), w.snapshot());
    assert_eq!(restored.revision(), 0);
    assert_eq!(
        restored
            .snapshot()
            .views
            .values()
            .filter(|record| record == &&view())
            .count(),
        3
    );
}

#[test]
fn edge_move_rebases_a_collapsed_ancestor_without_losing_source_or_target() {
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        let (mut w, host, group, a, _, _) = fixture();
        w.execute(Command::Move {
            selection: Selection::View(a),
            destination: Destination::Edge {
                target: group,
                edge: Edge::Left,
                fraction: 0.5,
            },
        })
        .unwrap();
        let old_root = w.snapshot().hosts[&host].root.unwrap();
        let source = w.snapshot().location(a).unwrap().0;
        w.execute(Command::Move {
            selection: Selection::Group(source),
            destination: Destination::Edge {
                target: old_root,
                edge,
                fraction: 0.3,
            },
        })
        .unwrap();
        assert_eq!(w.snapshot().location(a).unwrap().0, source);
        assert_eq!(w.snapshot().views.len(), 3);
        assert_eq!(w.snapshot().nodes.len(), 3);
        w.snapshot().validate().unwrap();
    }
}

#[test]
fn sessions_reject_revision_changes_and_failed_float_leaves_no_orphan_host() {
    let (mut w, _, group, a, b, _) = fixture();
    let mut session = Session::new(&w);
    session.update(&w, Command::Activate(a)).unwrap();
    w.execute(Command::Activate(b)).unwrap();
    assert_eq!(session.finish(&mut w), Err(Error::Stale));
    let mut session = Session::new(&w);
    w.execute(Command::Activate(a)).unwrap();
    assert!(matches!(
        session.update(&w, Command::Activate(b)),
        Err(Error::Stale)
    ));
    let before = w.snapshot().clone();
    assert_eq!(
        w.execute(Command::Float {
            selection: Selection::View(ViewId::new()),
            host: HostId::new()
        }),
        Err(Error::Missing)
    );
    assert_eq!(w.snapshot(), &before);
    assert_eq!(
        w.execute(Command::Move {
            selection: Selection::Group(group),
            destination: Destination::Tabs { group, index: 0 }
        }),
        Err(Error::SelfTarget)
    );
    assert_eq!(w.snapshot(), &before);
}

#[test]
fn floating_host_close_hides_all_views_atomically_without_forgetting_records() {
    let (mut w, main, group, a, b, c) = fixture();
    let before = w.snapshot().clone();
    assert!(w.execute(Command::HideHost(main)).is_err());
    assert_eq!(w.snapshot(), &before);
    let float = HostId::new();
    w.execute(Command::Float {
        selection: Selection::Group(group),
        host: float,
    })
    .unwrap();
    w.execute(Command::Move {
        selection: Selection::View(c),
        destination: Destination::Edge {
            target: group,
            edge: Edge::Bottom,
            fraction: 0.5,
        },
    })
    .unwrap();
    w.execute(Command::HideHost(float)).unwrap();
    assert!(!w.snapshot().hosts.contains_key(&float));
    for view in [a, b, c] {
        assert!(w.snapshot().views.contains_key(&view));
        assert_eq!(w.snapshot().hidden[&view].host, float);
    }
    assert!(w.snapshot().nodes.is_empty());
    assert!(w.snapshot().hosts[&main].root.is_none());
}
