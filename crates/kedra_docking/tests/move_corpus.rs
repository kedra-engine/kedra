//! Common operation corpus: every move publishes a valid ownership-preserving tree.
#![allow(clippy::unwrap_used, reason = "Assertions on bounded corpus fixtures")]
use kedra_docking::{command::*, model::*};

#[test]
fn all_selection_destination_pairs_preserve_content_or_reject_atomically() {
    let host = HostId::new();
    let mut base = Workspace::new(host);
    let views: Vec<_> = (0..5).map(|_| ViewId::new()).collect();
    for (index, id) in views.iter().enumerate() {
        let destination = if index == 0 {
            Destination::EmptyHost(host)
        } else {
            let group = base.snapshot().location(views[0]).unwrap().0;
            if index == 1 {
                Destination::Tabs { group, index: 1 }
            } else {
                Destination::Edge {
                    target: group,
                    edge: Edge::Right,
                    fraction: 0.5,
                }
            }
        };
        base.execute(Command::Open {
            id: *id,
            view: View {
                kind: format!("fixture.{index}"),
                restore_key: None,
            },
            destination,
        })
        .unwrap();
    }
    let second_host = HostId::new();
    base.execute(Command::Float {
        selection: Selection::View(views[4]),
        host: second_host,
    })
    .unwrap();
    let mut selections: Vec<_> = views.iter().copied().map(Selection::View).collect();
    let mut destinations = Vec::new();
    for (id, node) in &base.snapshot().nodes {
        if let Node::Group { tabs, .. } = node {
            selections.push(Selection::Group(*id));
            for index in 0..=tabs.len() + 1 {
                destinations.push(Destination::Tabs { group: *id, index });
            }
        }
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            destinations.push(Destination::Edge {
                target: *id,
                edge,
                fraction: 0.4,
            });
        }
    }
    destinations.push(Destination::Tabs {
        group: NodeId::new(),
        index: 0,
    });
    destinations.push(Destination::EmptyHost(host));
    destinations.push(Destination::EmptyHost(HostId::new()));
    let mut accepted = 0;
    let mut rejected = 0;
    for selection in selections {
        for destination in &destinations {
            let mut w = base.snapshot().clone().into_workspace().unwrap();
            let before = w.snapshot().clone();
            match w.execute(Command::Move {
                selection,
                destination: *destination,
            }) {
                Ok(changed) => {
                    accepted += 1;
                    w.snapshot().validate().unwrap();
                    assert_eq!(w.snapshot().views, before.views);
                    assert_eq!(w.snapshot().hidden, before.hidden);
                    assert_eq!(w.revision(), u64::from(changed));
                    for view in &views {
                        assert!(w.snapshot().location(*view).is_some());
                    }
                    assert_eq!(
                        w.snapshot().encode().unwrap(),
                        w.snapshot()
                            .clone()
                            .into_workspace()
                            .unwrap()
                            .snapshot()
                            .encode()
                            .unwrap()
                    );
                }
                Err(_) => {
                    rejected += 1;
                    assert_eq!(w.snapshot(), &before);
                    assert_eq!(w.revision(), 0);
                }
            }
        }
    }
    assert!(
        accepted > 100 && rejected > 20,
        "corpus must exercise both paths: {accepted}/{rejected}"
    );
}
