//! Production coordinator/panel binding tests without a native renderer.
use super::*;
use test_support::TestResult;

#[test]
fn deferred_hierarchy_input_rejects_inactive_documents_and_hidden_tools() -> TestResult {
    use crate::hierarchy::{Action, Binding};
    use kedra_scene::document::{Node as SourceNode, NodeId, PrefabDocument};
    let directory = tempfile::tempdir()?;
    let project = kedra_project::create(directory.path(), "Hierarchy actions")?;
    let mut first = PrefabDocument::new("First");
    let leaf = NodeId::new();
    first.nodes.push(SourceNode {
        id: leaf,
        name: "Leaf".into(),
        parent: Some(first.root.into()),
        components: BTreeMap::new(),
    });
    let second = PrefabDocument::new("Second");
    for (name, doc) in [("first.prefab.ron", &first), ("second.prefab.ron", &second)] {
        std::fs::write(project.assets.join(name), doc.serialize()?)?;
    }
    let root = project.assets.clone();
    let (mut app, _, _) = test_support::setup()?;
    let world = app.world_mut();
    test_support::sources(world)?;
    world.insert_resource(test_support::project_manager(project, None)?);
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            panels::preset(&mut state)?;
            panels::mount(world, &mut state)
        },
    )?;
    fn open(
        world: &mut World,
        root: &std::path::Path,
        path: &str,
        id: kedra_scene::document::DocumentId,
    ) -> Result<(), String> {
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            state.documents.request(world, root.into(), path.into())
        })?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                panels::poll(world, &mut state)
            });
            if world.resource::<EditorWorkspace>().documents.active == Some(id) {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        Ok(())
    }
    open(world, &root, "first.prefab.ron", first.id)?;
    let view = *world
        .resource::<EditorWorkspace>()
        .panels
        .hierarchies
        .keys()
        .next()
        .ok_or("Hierarchy")?;
    let binding = Binding {
        view,
        document: Some(first.id),
        action: Action::Select(leaf.into()),
    };
    hierarchy_action(world, binding.clone());
    assert_eq!(
        world.resource::<EditorWorkspace>().documents.opened[&first.id].selected,
        Some(leaf.into())
    );
    let element = |world: &mut World, key: &str| {
        world
            .query::<(Entity, &kedra_ui::ElementId)>()
            .iter(world)
            .find_map(|(entity, id)| {
                (id.0 == format!("hierarchy/{view:?}/{key}")).then_some(entity)
            })
            .ok_or_else(|| format!("Hierarchy element missing: {key}"))
    };
    let toolbar = element(world, "toolbar")?;
    let status = element(world, "status")?;
    let entries = element(world, "entries")?;
    world
        .entity_mut(entries)
        .insert(ScrollPosition(Vec2::new(0., 99.)));
    hierarchy_action(
        world,
        Binding {
            action: Action::Key(leaf.into(), KeyCode::Home),
            ..binding.clone()
        },
    );
    assert_eq!(
        world
            .get::<ScrollPosition>(entries)
            .ok_or("Hierarchy scroll missing")?
            .y,
        0.,
        "Home must preserve the four-pixel first-row inset"
    );
    hierarchy_action(world, binding.clone());
    // Synthetic row sources must expose selector state just like production
    // UiClasses; otherwise a hidden-toggle regression would be invisible here.
    world.entity_mut(entries).insert(ScrollPosition::default());
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| panels::poll(world, &mut state));
    for key in ["toggle/0", "toggle/1"] {
        let toggle = element(world, key)?;
        world
            .entity_mut(toggle)
            .insert(bevy_flair::style::components::ClassList::default());
    }
    hierarchy_action(
        world,
        Binding {
            action: Action::CloseSearch,
            ..binding.clone()
        },
    );
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| panels::poll(world, &mut state));
    let hidden = |world: &World, entity| {
        world
            .get::<bevy_flair::style::components::ClassList>(entity)
            .is_some_and(|classes| classes.contains("hidden"))
    };
    assert!(
        hidden(world, toolbar) && hidden(world, status),
        "Default tree retained extra chrome"
    );
    let branch_toggle = element(world, "toggle/0")?;
    let leaf_toggle = element(world, "toggle/1")?;
    assert!(!hidden(world, branch_toggle));
    assert!(
        !hidden(world, leaf_toggle),
        "Leaf rows must reserve the same disclosure slot as branch rows"
    );
    assert!(
        world
            .get::<bevy::ui::InteractionDisabled>(leaf_toggle)
            .is_some(),
        "The reserved leaf slot must not be actionable"
    );
    assert!(
        world
            .get::<bevy::ui::InteractionDisabled>(branch_toggle)
            .is_none()
    );
    hierarchy_action(
        world,
        Binding {
            action: Action::FocusSearch,
            ..binding.clone()
        },
    );
    assert!(!hidden(world, toolbar));
    hierarchy_action(
        world,
        Binding {
            action: Action::CloseSearch,
            ..binding.clone()
        },
    );
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| panels::poll(world, &mut state));
    assert!(hidden(world, toolbar));
    assert_eq!(
        world.resource::<EditorWorkspace>().documents.opened[&first.id].selected,
        Some(leaf.into())
    );
    hierarchy_action(
        world,
        Binding {
            action: Action::Details,
            ..binding.clone()
        },
    );
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| panels::poll(world, &mut state));
    assert!(!hidden(world, status));
    open(world, &root, "second.prefab.ron", second.id)?;
    hierarchy_action(world, binding.clone());
    assert_eq!(
        world.resource::<EditorWorkspace>().documents.opened[&second.id].selected,
        Some(second.root.into())
    );
    let first_view = world.resource::<EditorWorkspace>().documents.opened[&first.id].view;
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        apply(world, &mut state, Command::Activate(first_view))
    })?;
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| panels::poll(world, &mut state));
    world
        .resource_mut::<EditorWorkspace>()
        .layout
        .execute(Command::Hide(view))?;
    hierarchy_action(
        world,
        Binding {
            action: Action::Select(first.root.into()),
            ..binding
        },
    );
    let state = world.resource::<EditorWorkspace>();
    assert_eq!(
        state.documents.opened[&first.id].selected,
        Some(leaf.into())
    );
    assert_eq!(
        std::fs::read_to_string(root.join("first.prefab.ron"))?,
        first.serialize()?
    );
    Ok(())
}
