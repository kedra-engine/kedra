//! Projection-owned destinations preserve the real rail and menu through transitions.
use super::*;
use kedra_ui::docking::HeaderDestination;

#[test]
fn integrated_header_retains_tab_and_content_entities() -> Result<()> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(AuthoredUiPlugin);
    let skin = Skin {
        group: compile(&app, &["tabs", "body", "menu"])?,
        tab: compile(&app, &["label", "title", "close"])?,
        split: compile(&app, &["divider"])?,
    };
    let host_id = HostId::new();
    let mut model = Workspace::new(host_id);
    let a = ViewId::new();
    model.execute(Command::Open {
        id: a,
        view: View {
            kind: "a".into(),
            restore_key: None,
        },
        destination: Destination::EmptyHost(host_id),
    })?;
    let group = model.snapshot().location(a).ok_or("missing group")?.0;
    let world = app.world_mut();
    let host = world.spawn(Node::default()).id();
    let stash = world.spawn(Node::default()).id();
    let content = world
        .spawn((Node::default(), EditableText::new("Retained 日本語")))
        .id();
    let row = world.spawn(Node::default()).id();
    let menu_slot = world.spawn(Node::default()).id();
    let mut projection = Projection::default();
    projection.register(a, content)?;
    let hosts = BTreeMap::from([(host_id, host)]);
    projection.sync(
        world,
        model.snapshot(),
        &hosts,
        stash,
        &skin,
        &BTreeMap::new(),
        "test",
        1.,
    )?;
    let label = projection.label(a).ok_or("missing label")?;
    let rail = projection.rail(group).ok_or("missing rail")?;
    let menu = projection.menu(group).ok_or("missing menu")?;
    let original_parent = world.get::<ChildOf>(rail).ok_or("missing parent")?.parent();
    projection.set_header_destinations(
        world,
        BTreeMap::from([(
            group,
            HeaderDestination {
                row,
                menu: menu_slot,
            },
        )]),
    )?;
    // A setter prepares policy, not a partial publication.
    assert_eq!(
        world.get::<ChildOf>(rail).ok_or("missing parent")?.parent(),
        original_parent
    );
    projection.sync(
        world,
        model.snapshot(),
        &hosts,
        stash,
        &skin,
        &BTreeMap::new(),
        "test",
        1.,
    )?;
    assert_eq!(
        world
            .get::<ChildOf>(rail)
            .ok_or("missing rail parent")?
            .parent(),
        row
    );
    assert_eq!(
        world
            .get::<ChildOf>(menu)
            .ok_or("missing menu parent")?
            .parent(),
        menu_slot
    );
    assert_eq!(projection.label(a), Some(label));
    assert_eq!(projection.content(a), Some(content));
    // A failed destination change must not detach or steal the live rail.
    assert!(
        projection
            .set_header_destinations(
                world,
                BTreeMap::from([(
                    group,
                    HeaderDestination {
                        row: rail,
                        menu: menu_slot
                    }
                )])
            )
            .is_err()
    );
    assert_eq!(
        world.get::<ChildOf>(rail).ok_or("missing parent")?.parent(),
        row
    );
    // A split composition removes external destinations; same rail returns.
    projection.set_header_destinations(world, BTreeMap::new())?;
    projection.sync(
        world,
        model.snapshot(),
        &hosts,
        stash,
        &skin,
        &BTreeMap::new(),
        "test",
        1.,
    )?;
    assert_eq!(
        world.get::<ChildOf>(rail).ok_or("missing parent")?.parent(),
        original_parent
    );
    assert_eq!(projection.label(a), Some(label));
    assert_eq!(
        world
            .get::<EditableText>(content)
            .ok_or("missing retained value")?
            .editor
            .text(),
        "Retained 日本語"
    );
    Ok(())
}
