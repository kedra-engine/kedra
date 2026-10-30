//! Shared command paths and branch ownership; no editor-specific command dispatch.
use bevy::prelude::*;
use kedra_ui::popup::{self, Entry, cascade};

#[test]
fn display_paths_merge_branches_preserve_unicode_commands_and_reject_conflicts()
-> Result<(), String> {
    let menu = popup::from_paths([
        ("Create/Scene…", 1),
        ("Create/Prefab…", 2),
        ("Create/UI/日本語", 3),
        ("Import…", 4),
    ])?;
    let Entry::Submenu { label, children } = &menu[0] else {
        return Err("Missing branch".into());
    };
    assert_eq!(label, "Create");
    assert!(matches!(&children[0], Entry::Command { label, command: 1 } if label == "Scene…"));
    assert!(
        matches!(&children[2], Entry::Submenu { label, children } if label == "UI" && matches!(&children[0], Entry::Command { label, command: 3 } if label == "日本語"))
    );
    for paths in [
        vec![("Create", 1), ("Create/Scene", 2)],
        vec![("Create/Scene", 1), ("Create", 2)],
        vec![("Create/Scene", 1), ("Create/Scene", 2)],
        vec![("Create//Scene", 1)],
    ] {
        assert!(popup::from_paths(paths).is_err());
    }
    Ok(())
}

#[test]
fn submenu_replacement_and_disabled_branch_preserve_foreign_ownership() -> Result<(), String> {
    let mut world = World::new();
    let anchor = world.spawn(Node::default()).id();
    let foreign = world.spawn(Node::default()).id();
    let root = popup::attach(&mut world, anchor, "menu", "root")?;
    let a = world.spawn((Node::default(), ChildOf(root))).id();
    let b = world.spawn((Node::default(), ChildOf(root))).id();
    cascade::branch(&mut world, a);
    cascade::branch(&mut world, b);
    let first = cascade::attach(&mut world, a, "menu", "first")?;
    assert_eq!(cascade::attach(&mut world, a, "menu", "first")?, first);
    let leaf = world.spawn((Node::default(), ChildOf(first))).id();
    world.entity_mut(b).insert(bevy::ui::InteractionDisabled);
    assert!(cascade::attach(&mut world, b, "menu", "disabled").is_err());
    assert!(world.get_entity(leaf).is_ok());
    world
        .entity_mut(b)
        .remove::<bevy::ui::InteractionDisabled>();
    let second = cascade::attach(&mut world, b, "menu", "second")?;
    assert!(world.get_entity(first).is_err());
    assert!(world.get_entity(leaf).is_err());
    assert!(world.get_entity(second).is_ok());
    assert!(world.get_entity(root).is_ok());
    assert!(world.get_entity(foreign).is_ok());
    Ok(())
}
