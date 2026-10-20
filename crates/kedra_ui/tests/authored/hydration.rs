//! Native hydration and observation bindings without a renderer or OS window.
use super::*;
use bevy::{
    input_focus::AutoFocus,
    text::EditableText,
    ui_widgets::{Button, ModalDialog, ModalDialogBarrier, SelectAllOnFocus, TextInput},
};
use bevy_flair::style::{
    StyleSheet,
    components::{ClassList, Styled},
};

fn node(
    document: &mut PrefabDocument,
    parent: NodeId,
    key: &str,
    components: &[(&str, &str)],
) -> Result<NodeId> {
    let id = NodeId::new();
    let mut data = BTreeMap::from([
        ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
        (
            "ui.id".into(),
            ComponentData::ron(1, &format!("(\"{key}\")"))?,
        ),
    ]);
    for (key, value) in components {
        data.insert((*key).into(), ComponentData::ron(1, value)?);
    }
    document.nodes.push(SourceNode {
        id,
        name: key.into(),
        parent: Some(parent.into()),
        components: data,
    });
    Ok(id)
}

fn styled_parent(app: &mut App) -> Entity {
    let sheet = app
        .world_mut()
        .resource_mut::<Assets<StyleSheet>>()
        .add(StyleSheet::empty());
    app.world_mut()
        .spawn((Node::default(), Styled::new(sheet)))
        .id()
}

#[test]
fn authored_hydration_keeps_source_inert_and_owns_native_widgets_and_bindings() -> Result<()> {
    let mut app = setup();
    let mut source = PrefabDocument::new("Native widgets");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    source.nodes[0].components.insert(
        "ui.classes".into(),
        ComponentData::ron(1, "(names:[\"panel\",\"theme\"])")?,
    );
    source.nodes[0]
        .components
        .insert("ui.modal".into(), ComponentData::ron(1, "Dialog")?);
    let root = source.root;
    let button = node(&mut source, root, "button", &[("ui.button", "()")])?;
    node(
        &mut source,
        button,
        "label",
        &[("ui.caption", "(text:\"Run Кедра\",justify:Left,wrap:false)")],
    )?;
    node(
        &mut source,
        root,
        "field",
        &[
            (
                "ui.caption",
                "(text:\"Draft e\\u{301} 日本語\",justify:Center,wrap:true)",
            ),
            ("ui.editable", "(autofocus:true,select_all:true)"),
        ],
    )?;
    node(&mut source, root, "barrier", &[("ui.modal", "Barrier")])?;
    let snapshot = compile(&app, source)?;
    for entity in snapshot.source_entities().values() {
        let inert = snapshot.authoring_world();
        assert!(inert.get::<Node>(*entity).is_none());
        assert!(inert.get::<TextInput>(*entity).is_none());
        assert!(inert.get::<Styled>(*entity).is_none());
    }
    let parent = styled_parent(&mut app);
    let view = UiView::mount(
        app.world_mut(),
        &snapshot,
        Some(parent),
        &BTreeMap::new(),
        "test",
        1.,
    )?;
    let world = app.world_mut();
    assert!(world.get::<UiReady>(view.root).is_some());
    assert!(world.get::<ModalDialog>(view.root).is_some());
    assert!(world.get::<TabGroup>(view.root).is_some_and(|g| g.modal));
    assert!(
        world
            .get::<ClassList>(view.root)
            .is_some_and(|c| c.contains("panel") && c.contains("theme"))
    );
    assert!(
        world
            .get::<ModalDialogBarrier>(view.element("barrier")?)
            .is_some()
    );
    assert!(world.get::<Button>(view.element("button")?).is_some());
    assert_eq!(
        world
            .get::<ElementId>(view.element("label")?)
            .ok_or("label ID")?
            .0,
        "test/label"
    );
    assert_eq!(view.text(world, "button")?, view.element("label")?);
    assert_eq!(view.text(world, "label")?, view.element("label")?);
    assert!(view.text(world, "barrier").is_err());
    let field = view.element("field")?;
    assert!(world.get::<TextInput>(field).is_some());
    assert!(world.get::<AutoFocus>(field).is_some());
    assert!(world.get::<SelectAllOnFocus>(field).is_some());
    assert_eq!(
        world
            .get::<EditableText>(field)
            .ok_or("editable text")?
            .value()
            .to_string(),
        "Draft e\u{301} 日本語"
    );
    assert_eq!(
        world.get::<TextLayout>(field).ok_or("layout")?.justify,
        Justify::Center
    );
    assert_eq!(
        world.get::<TextLayout>(field).ok_or("layout")?.linebreak,
        bevy::text::LineBreak::WordBoundary
    );
    // Feature-owned text updates remain native data, not source rehydration.
    world
        .get_mut::<Text>(view.element("label")?)
        .ok_or("label")?
        .0 = "Changed".into();
    assert_eq!(
        world
            .get::<Text>(view.text(world, "button")?)
            .ok_or("label")?
            .0,
        "Changed"
    );
    let duplicate = world
        .spawn((Text::new("Ambiguous"), ChildOf(view.element("button")?)))
        .id();
    assert!(view.text(world, "button").is_err());
    world.despawn(duplicate);
    let owned: Vec<_> = view.elements().values().copied().collect();
    assert!(Arc::ptr_eq(
        &world
            .get::<kedra_scene::instance::PrefabInstance>(view.root)
            .ok_or("owner")?
            .source,
        &snapshot
    ));
    world.despawn(view.root);
    assert!(owned.into_iter().all(|e| !world.entities().contains(e)));
    assert!(world.entities().contains(parent));
    Ok(())
}

#[test]
fn authored_hydration_rejects_widget_contract_and_runtime_failures_without_allocation() -> Result<()>
{
    let mut app = setup();
    let parent = styled_parent(&mut app);
    for (component, value, missing_layout) in [
        ("ui.editable", "(autofocus:false,select_all:false)", false),
        ("ui.button", "()", true),
        (
            "ui.caption",
            "(text:\"No layout\",justify:Left,wrap:false)",
            true,
        ),
    ] {
        let mut source = PrefabDocument::new("Invalid widget");
        source.nodes[0]
            .components
            .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
        let root = source.root;
        let id = node(&mut source, root, "invalid", &[(component, value)])?;
        if missing_layout {
            source
                .nodes
                .iter_mut()
                .find(|n| n.id == id)
                .ok_or("node")?
                .components
                .remove("ui.layout");
        }
        let snapshot = compile(&app, source)?;
        let before = app.world().entities().count_spawned();
        assert!(
            UiView::mount(
                app.world_mut(),
                &snapshot,
                Some(parent),
                &BTreeMap::new(),
                "",
                1.
            )
            .is_err()
        );
        assert_eq!(app.world().entities().count_spawned(), before);
    }
    let mut source = PrefabDocument::new("Runtime failure");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    let snapshot = compile(&app, source)?;
    app.world_mut().resource_mut::<UiFailure>().0 = Some("Explicit preparation failure".into());
    let before = app.world().entities().count_spawned();
    let error = UiView::mount(
        app.world_mut(),
        &snapshot,
        Some(parent),
        &BTreeMap::new(),
        "",
        1.,
    )
    .err()
    .ok_or("runtime error accepted")?;
    assert!(error.to_string().contains("Explicit preparation failure"));
    assert_eq!(app.world().entities().count_spawned(), before);
    app.world_mut().resource_mut::<UiFailure>().0 = None;
    let retired = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(retired);
    assert!(
        UiView::mount(
            app.world_mut(),
            &snapshot,
            Some(retired),
            &BTreeMap::new(),
            "",
            1.
        )
        .is_err()
    );
    assert!(
        UiView::mount(
            app.world_mut(),
            &snapshot,
            Some(parent),
            &BTreeMap::from([("unknown".into(), None)]),
            "",
            1.
        )
        .is_err()
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    let view = UiView::mount(
        app.world_mut(),
        &snapshot,
        Some(parent),
        &BTreeMap::new(),
        "",
        1.,
    )?;
    assert!(app.world().get::<UiReady>(view.root).is_some());
    Ok(())
}

#[test]
fn blocked_native_stylesheet_scope_does_not_inherit_an_outer_sheet() -> Result<()> {
    let mut app = setup();
    let parent = styled_parent(&mut app);
    let block = app
        .world_mut()
        .spawn((Node::default(), Styled::Block, ChildOf(parent)))
        .id();
    let mut source = PrefabDocument::new("Blocked scope");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    source.nodes[0].components.insert(
        "ui.caption".into(),
        ComponentData::ron(1, "(text:\"No inherited style\",justify:Left,wrap:false)")?,
    );
    let snapshot = compile(&app, source)?;
    let before = app.world().entities().count_spawned();
    assert!(
        UiView::mount(
            app.world_mut(),
            &snapshot,
            Some(block),
            &BTreeMap::new(),
            "",
            1.
        )
        .is_err()
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    assert!(
        UiView::mount(
            app.world_mut(),
            &snapshot,
            Some(parent),
            &BTreeMap::new(),
            "",
            1.
        )
        .is_ok()
    );
    Ok(())
}
