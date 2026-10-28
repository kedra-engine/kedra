//! Shared field access is native, retained and independent of editor documents.
use bevy::{
    asset::{AssetApp, AssetPlugin},
    input_focus::tab_navigation::TabIndex,
    prelude::*,
    text::{EditableText, TextReadWriteMode},
    ui::InteractionDisabled,
};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, NodeId, PrefabDocument},
    instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use kedra_ui::fields::{FieldTemplates, PropertyRow, SectionView, ValueView};
use kedra_ui::{
    authored::{AuthoredUiPlugin, components::UiEditable},
    fields::FieldAccess,
};
use std::{collections::BTreeMap, sync::Arc};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn row_updates_preserve_inline_label_and_editor_at_narrow_widths() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut row = PropertyRow::mount(app.world_mut(), parent, &templates, "late-layout")?;
    row.update(app.world_mut(), "Label", None)?;
    let editor = row.editor();
    for width in [260., 350., 280., 200.] {
        let mut computed = app
            .world_mut()
            .get_mut::<ComputedNode>(row.root())
            .ok_or("computed node")?;
        computed.size.x = width;
        computed.inverse_scale_factor = 1.;
        app.update();
        row.update(app.world_mut(), "Label", Some("Validation failed"))?;
        let classes = app
            .world()
            .get::<bevy_flair::style::components::ClassList>(row.root())
            .ok_or("classes")?;
        assert!(!classes.contains("compact"));
        assert_eq!(row.editor(), editor);
        assert_eq!(
            app.world()
                .get::<Text>(row.diagnostic())
                .map(|t| t.0.as_str()),
            Some("Validation failed")
        );
    }
    Ok(())
}

fn fixture() -> TestResult<(App, Entity, FieldTemplates)> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(AuthoredUiPlugin);
    let template = |keys: &[&str]| -> TestResult<Arc<CompiledPrefab>> {
        let mut doc = PrefabDocument::new("Reusable field fixture");
        let root = doc.root;
        doc.nodes[0]
            .components
            .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
        doc.nodes[0]
            .components
            .insert("ui.classes".into(), ComponentData::ron(1, "(names:[])")?);
        if keys == ["caption"] {
            doc.nodes[0]
                .components
                .insert("ui.button".into(), ComponentData::ron(1, "()")?);
        }
        for key in keys {
            let mut components = BTreeMap::from([
                ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
                (
                    "ui.id".into(),
                    ComponentData::ron(1, &format!("(\"{key}\")"))?,
                ),
                ("ui.classes".into(), ComponentData::ron(1, "(names:[])")?),
            ]);
            if ["label", "diagnostic", "value", "title", "caption"].contains(key) {
                components.insert(
                    "ui.caption".into(),
                    ComponentData::ron(1, "(text:\"\",justify:Left,wrap:false)")?,
                );
            }
            if *key == "value" {
                components.insert(
                    "ui.editable".into(),
                    ComponentData::ron(1, "(autofocus:false,select_all:false)")?,
                );
            }
            doc.nodes.push(SourceNode {
                id: NodeId::new(),
                name: (*key).into(),
                parent: Some(root.into()),
                components,
            });
        }
        let mut catalog = Catalog::default();
        let id = doc.id;
        catalog.insert(doc)?;
        Ok(Arc::new(CompiledPrefab::compile(
            catalog.resolve(id)?,
            &app.world().resource::<AppTypeRegistry>().read(),
            app.world().resource::<ComponentPolicies>(),
            &mut app.world().resource::<AssetServer>(),
            &BTreeMap::new(),
        )?))
    };
    let templates = FieldTemplates {
        command: template(&["caption"])?,
        row: template(&["label", "editor", "diagnostic"])?,
        section: template(&["title", "toggle", "body"])?,
        value: template(&["value", "swatch"])?,
        vector: template(&[
            "axis-x", "value-x", "axis-y", "value-y", "axis-z", "value-z", "axis-w", "value-w",
        ])?,
        density: 1.,
    };
    let sheet = app
        .world_mut()
        .resource_mut::<Assets<bevy_flair::style::StyleSheet>>()
        .add(bevy_flair::style::StyleSheet::empty());
    let parent = app
        .world_mut()
        .spawn((
            Node::default(),
            bevy_flair::style::components::Styled::new(sheet),
        ))
        .id();
    Ok((app, parent, templates))
}

#[test]
fn generic_property_slots_retain_native_values_and_retire_only_owned_subtrees() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut row = PropertyRow::mount(app.world_mut(), parent, &templates, "independent/row")?;
    let mut value = ValueView::mount(
        app.world_mut(),
        row.editor(),
        &templates,
        "independent/value",
    )?;
    let sibling = app.world_mut().spawn(ChildOf(parent)).id();
    row.update(app.world_mut(), "Unicode 名称", Some("Read-only source"))?;
    value.update(
        app.world_mut(),
        " 👩🏽‍🚀 e\u{301} ",
        &FieldAccess::ReadOnly {
            reason: "Read-only source".into(),
        },
    )?;
    value.describe(app.world_mut(), "Unicode 名称", "Read-only source")?;
    let root = row.root();
    let input = value.input();
    for _ in 0..3 {
        row.update(app.world_mut(), "Unicode 名称", Some("Read-only source"))?;
        value.update(
            app.world_mut(),
            " 👩🏽‍🚀 e\u{301} ",
            &FieldAccess::ReadOnly {
                reason: "Read-only source".into(),
            },
        )?;
    }
    assert_eq!(row.root(), root);
    assert_eq!(value.input(), input);
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .map(|t| t.value().to_string()),
        Some(" 👩🏽‍🚀 e\u{301} ".into())
    );
    assert_eq!(
        app.world().get::<TextReadWriteMode>(input),
        Some(&TextReadWriteMode::ReadOnly)
    );
    assert_eq!(
        app.world()
            .get::<bevy::a11y::AccessibilityNode>(input)
            .and_then(|node| node.label()),
        Some("Unicode 名称")
    );
    row.retire(app.world_mut());
    row.retire(app.world_mut());
    assert!(app.world().get_entity(input).is_err());
    assert!(app.world().get_entity(sibling).is_ok());
    Ok(())
}

#[test]
fn color_sample_uses_its_authored_slot_and_retired_surfaces_refuse_without_touching_siblings()
-> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let view = ValueView::mount(app.world_mut(), parent, &templates, "color-sample")?;
    let sibling = ValueView::mount(app.world_mut(), parent, &templates, "color-sibling")?;
    view.color_sample(app.world_mut(), Color::srgb(0., 1., 0.))?;
    let input = view.input();
    view.retire(app.world_mut());
    assert!(view.color_sample(app.world_mut(), Color::WHITE).is_err());
    assert!(!app.world().entities().contains(input));
    assert!(app.world().entities().contains(sibling.root()));
    sibling.color_sample(app.world_mut(), Color::srgb(1., 0., 0.))?;
    sibling.retire(app.world_mut());
    Ok(())
}

#[test]
fn compound_values_retain_order_access_and_owned_controls_and_reject_dimension_changes()
-> TestResult {
    use kedra_ui::fields::VectorView;
    let (mut app, parent, templates) = fixture()?;
    let before = app.world().entities().count_spawned();
    assert!(VectorView::mount(app.world_mut(), parent, &templates, "invalid", 1).is_err());
    assert_eq!(app.world().entities().count_spawned(), before);
    let mut vector = VectorView::mount(app.world_mut(), parent, &templates, "vector", 3)?;
    let sibling = app.world_mut().spawn(ChildOf(parent)).id();
    let access = FieldAccess::ReadOnly {
        reason: "Source has no write capability".into(),
    };
    let values = ["0.33333334".into(), "-12.5".into(), "1000000".into()];
    vector.update(app.world_mut(), &values, "Position", "Local space", &access)?;
    let inputs = vector.inputs().collect::<Vec<_>>();
    for (input, expected) in inputs.iter().zip(&values) {
        assert_eq!(
            app.world()
                .get::<bevy::text::EditableText>(*input)
                .ok_or("input")?
                .value(),
            expected
        );
        assert_eq!(app.world().get::<FieldAccess>(*input), Some(&access));
    }
    assert!(
        vector
            .update(
                app.world_mut(),
                &values[..2],
                "Position",
                "Local space",
                &access
            )
            .is_err()
    );
    vector.update(app.world_mut(), &values, "Position", "Local space", &access)?;
    assert_eq!(vector.inputs().collect::<Vec<_>>(), inputs);
    vector.retire(app.world_mut());
    assert!(
        inputs
            .iter()
            .all(|entity| !app.world().entities().contains(*entity))
    );
    assert!(app.world().entities().contains(sibling));
    Ok(())
}

#[test]
fn malformed_template_does_not_leak_a_partial_row() -> TestResult {
    let (mut app, parent, mut templates) = fixture()?;
    templates.row = templates.value.clone();
    let before = app.world().entities().count_spawned();
    assert!(PropertyRow::mount(app.world_mut(), parent, &templates, "broken").is_err());
    assert_eq!(before, app.world().entities().count_spawned());
    Ok(())
}

#[test]
fn collapsed_sections_keep_owned_controls_and_other_hosts_intact() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut section = SectionView::mount(app.world_mut(), parent, &templates, "host-a/section")?;
    let row = PropertyRow::mount(app.world_mut(), section.body(), &templates, "host-a/row")?;
    let other = PropertyRow::mount(app.world_mut(), parent, &templates, "host-b/row")?;
    for expanded in [true, false, false, true] {
        section.update(app.world_mut(), "Components", expanded)?;
        assert!(app.world().get_entity(row.root()).is_ok());
        assert!(app.world().get_entity(other.root()).is_ok());
    }
    section.retire(app.world_mut());
    assert!(app.world().get_entity(row.root()).is_err());
    assert!(app.world().get_entity(other.root()).is_ok());
    Ok(())
}

#[test]
fn read_only_access_survives_native_mode_updates_without_disabling_copy() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(AuthoredUiPlugin);
    let entity = app
        .world_mut()
        .spawn((
            UiEditable {
                autofocus: false,
                select_all: false,
            },
            EditableText::new(" 日本語 👩🏽‍🚀 "),
            TextReadWriteMode::Editable,
            TabIndex(0),
            FieldAccess::ReadOnly {
                reason: "Source editing is unavailable".into(),
            },
        ))
        .id();
    app.update();
    app.update();
    assert_eq!(
        app.world().get::<TextReadWriteMode>(entity),
        Some(&TextReadWriteMode::ReadOnly)
    );
    assert!(app.world().get::<InteractionDisabled>(entity).is_none());
    assert!(app.world().get::<TabIndex>(entity).is_some());
    assert_eq!(
        app.world()
            .get::<EditableText>(entity)
            .map(|t| t.value().to_string()),
        Some(" 日本語 👩🏽‍🚀 ".into())
    );
    app.world_mut().entity_mut(entity).remove::<FieldAccess>();
    app.update();
    assert_eq!(
        app.world().get::<TextReadWriteMode>(entity),
        Some(&TextReadWriteMode::Editable)
    );
}
