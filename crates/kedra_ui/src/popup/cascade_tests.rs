//! Cascading dismissal keeps parent focus and never steals a foreign focus.
use super::*;

#[test]
fn escape_returns_to_branch_and_outside_focus_retires_only_the_owned_chain() -> Result<(), String> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CascadePlugin))
        .init_resource::<InputFocus>();
    let world = app.world_mut();
    let anchor = world.spawn(Node::default()).id();
    let foreign = world.spawn(Node::default()).id();
    let root = crate::popup::attach(world, anchor, "menu", "root")?;
    let item = world.spawn((Node::default(), ChildOf(root))).id();
    branch(world, item);
    let submenu = attach(world, item, "menu", "child")?;
    let leaf = world.spawn((Node::default(), ChildOf(submenu))).id();
    world
        .resource_mut::<InputFocus>()
        .set(leaf, FocusCause::Navigated);
    world.trigger(MenuEvent {
        source: submenu,
        action: MenuAction::CloseAll,
    });
    world.flush();
    assert_eq!(world.resource::<InputFocus>().get(), Some(item));
    assert!(world.get_entity(root).is_ok());
    assert!(world.get_entity(submenu).is_err());
    let submenu = attach(world, item, "menu", "child")?;
    world
        .resource_mut::<InputFocus>()
        .set(foreign, FocusCause::Pressed);
    world.trigger(MenuEvent {
        source: submenu,
        action: MenuAction::CloseAll,
    });
    world.flush();
    assert!(world.get_entity(root).is_err());
    assert!(world.get_entity(submenu).is_err());
    assert!(world.get_entity(anchor).is_ok());
    assert_eq!(world.resource::<InputFocus>().get(), Some(foreign));
    Ok(())
}
