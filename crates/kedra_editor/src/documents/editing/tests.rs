//! Editor source staging owns validation/publication isolation, not widget behavior.
use super::*;
use kedra_authoring::persistence::{SourceFile, SourceSnapshot, content_hash};
use kedra_scene::document::{ComponentData, PrefabDocument};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn native_caption_leaves_share_staged_authoring_and_source_history() -> TestResult {
    use kedra_inspector::{
        binding::{BindingToken, ValueBinding},
        fields::ReflectedField,
    };
    use kedra_ui::{authored::components::UiCaption, fields::FieldAccess};
    let app = crate::workspace::browser_test_app()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = app.world().resource::<ComponentPolicies>();
    let mut doc = PrefabDocument::new("Caption source");
    doc.nodes[0].components.insert(
        "ui.caption".into(),
        ComponentData::ron(1, "(text:\"Original\",justify:Left,wrap:false)")?,
    );
    let text = doc.serialize()?;
    let (scope, compiled) = source::admit(
        SourceSnapshot {
            namespace: "game".into(),
            generation: 1,
            documents: vec![SourceFile {
                path: "caption.prefab.ron".into(),
                hash: content_hash(&text),
                source: text,
            }],
            assets: vec![],
            index: None,
        },
        &types,
        policies,
    )?;
    let mut documents = Documents {
        scope: Some(scope),
        compiled,
        ..default()
    };
    documents.compiled_revisions = documents.revisions();
    let target = documents
        .target(doc.id, doc.root.into())
        .ok_or("caption target")?;
    let admission = ron::to_string(&documents.scope.as_ref().ok_or("scope")?.source)?;
    for (path, text) in [
        ("text", "Кедра · 日本語"),
        ("justify", "Center"),
        ("wrap", "true"),
    ] {
        let inspection = documents.inspect(&target)?;
        let value = inspection.value("ui.caption", &types, policies)?;
        let field = ReflectedField::bind(
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            ValueBinding {
                token: BindingToken(1),
                value,
                label: path,
                access: &FieldAccess::Editable,
                owner_type: Some(std::any::TypeId::of::<UiCaption>()),
                path: None,
                metadata: None,
                diagnostic: None,
            },
            &bevy::reflect::ParsedPath::parse(path)?,
        )?;
        let proposal = field.propose(text)?;
        let edit = PropertyEdit {
            path: field.path,
            value: proposal.as_ref(),
        };
        let baseline = documents.revisions();
        let change = documents.prepare_property(
            &target,
            &baseline,
            "ui.caption",
            &[edit],
            &types,
            policies,
        )?;
        assert_eq!(documents.revisions(), baseline);
        documents.publish_change(change);
    }
    let inspection = documents.inspect(&target)?;
    let accepted = inspection
        .value("ui.caption", &types, policies)?
        .try_downcast_ref::<UiCaption>()
        .ok_or("caption")?;
    assert_eq!(accepted.text, "Кедра · 日本語");
    assert_eq!(accepted.justify, Justify::Center);
    assert!(accepted.wrap);
    assert_eq!(
        ron::to_string(&documents.scope.as_ref().ok_or("scope")?.source)?,
        admission
    );
    for _ in 0..3 {
        let change = documents.prepare_history(false, &types, policies)?;
        documents.publish_change(change);
    }
    let inspection = documents.inspect(&target)?;
    let restored = inspection
        .value("ui.caption", &types, policies)?
        .try_downcast_ref::<UiCaption>()
        .ok_or("caption")?;
    assert_eq!(restored.text, "Original");
    assert_eq!(restored.justify, Justify::Left);
    assert!(!restored.wrap);
    let before = documents.revisions();
    let inspection = documents.inspect(&target)?;
    let field = ReflectedField::bind(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        ValueBinding {
            token: BindingToken(1),
            value: inspection.value("ui.caption", &types, policies)?,
            label: "wrap",
            access: &FieldAccess::Editable,
            owner_type: None,
            path: None,
            metadata: None,
            diagnostic: None,
        },
        &bevy::reflect::ParsedPath::parse("wrap")?,
    )?;
    assert!(field.propose("yes").is_err());
    assert_eq!(documents.revisions(), before);
    assert!(documents.can_history(true));
    Ok(())
}
fn documents() -> Result<
    (
        Documents,
        inspection::Target,
        TypeRegistry,
        ComponentPolicies,
    ),
    Box<dyn std::error::Error>,
> {
    let mut types = TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut types, &mut policies)?;
    let mut doc = PrefabDocument::new("Transform source");
    doc.nodes[0].components.insert(
        "scene.transform".into(),
        ComponentData::ron(
            1,
            "(translation:(0.0,0.0,0.0),rotation:(0.0,0.0,0.0,1.0),scale:(1.0,1.0,1.0))",
        )?,
    );
    let text = doc.serialize()?;
    let (scope, compiled) = source::admit(
        SourceSnapshot {
            namespace: "game".into(),
            generation: 1,
            documents: vec![SourceFile {
                path: "transform.prefab.ron".into(),
                hash: content_hash(&text),
                source: text,
            }],
            assets: vec![],
            index: None,
        },
        &types,
        &policies,
    )?;
    let mut documents = Documents {
        scope: Some(scope),
        compiled,
        ..default()
    };
    documents.compiled_revisions = documents.revisions();
    let target = documents.target(doc.id, doc.root.into()).ok_or("target")?;
    Ok((documents, target, types, policies))
}
#[test]
fn typed_transform_staging_publishes_once_and_history_refreshes_the_effective_source() -> TestResult
{
    let (mut documents, target, types, policies) = documents()?;
    let baseline = documents.revisions();
    let edits = [PropertyEdit {
        path: bevy::reflect::ParsedPath::parse("translation")?,
        value: &Vec3::new(12.5, 0., 0.),
    }];
    let change = documents.prepare_property(
        &target,
        &baseline,
        "scene.transform",
        &edits,
        &types,
        &policies,
    )?;
    assert_eq!(documents.revisions(), baseline);
    assert!(!documents.can_history(false));
    documents.publish_change(change);
    let inspected = documents.inspect(&target)?;
    let value = inspected
        .value("scene.transform", &types, &policies)?
        .try_downcast_ref::<Transform>()
        .ok_or("Transform")?;
    assert_eq!(value.translation, Vec3::new(12.5, 0., 0.));
    assert!(documents.can_history(false));
    assert!(
        documents
            .prepare_property(
                &target,
                &baseline,
                "scene.transform",
                &edits,
                &types,
                &policies
            )
            .is_err()
    );
    let change = documents.prepare_history(false, &types, &policies)?;
    documents.publish_change(change);
    assert_eq!(
        documents
            .inspect(&target)?
            .value("scene.transform", &types, &policies)?
            .try_downcast_ref::<Transform>()
            .ok_or("Transform")?
            .translation,
        Vec3::ZERO
    );
    assert!(documents.can_history(true));
    // Re-entering the exact accepted source does not consume the preserved redo.
    let edits = [PropertyEdit {
        path: bevy::reflect::ParsedPath::parse("translation")?,
        value: &Vec3::ZERO,
    }];
    let change = documents.prepare_property(
        &target,
        &documents.revisions(),
        "scene.transform",
        &edits,
        &types,
        &policies,
    )?;
    documents.publish_change(change);
    assert!(documents.can_history(true));
    let change = documents.prepare_history(true, &types, &policies)?;
    documents.publish_change(change);
    assert_eq!(
        documents
            .inspect(&target)?
            .value("scene.transform", &types, &policies)?
            .try_downcast_ref::<Transform>()
            .ok_or("Transform")?
            .translation
            .x,
        12.5
    );
    Ok(())
}
#[test]
fn rejected_or_abandoned_transform_candidates_preserve_sources_history_and_save_baselines()
-> TestResult {
    let (documents, target, types, policies) = documents()?;
    let before = ron::to_string(documents.scope.as_ref().ok_or("scope")?)?;
    for value in [Vec3::new(f32::NAN, 0., 0.), Vec3::new(1_000_001., 0., 0.)] {
        let edits = [PropertyEdit {
            path: bevy::reflect::ParsedPath::parse("translation")?,
            value: &value,
        }];
        assert!(
            documents
                .prepare_property(
                    &target,
                    &documents.revisions(),
                    "scene.transform",
                    &edits,
                    &types,
                    &policies
                )
                .is_err()
        );
        assert_eq!(
            ron::to_string(documents.scope.as_ref().ok_or("scope")?)?,
            before
        );
    }
    let value = Vec3::ONE;
    let edits = [PropertyEdit {
        path: bevy::reflect::ParsedPath::parse("translation")?,
        value: &value,
    }];
    drop(documents.prepare_property(
        &target,
        &documents.revisions(),
        "scene.transform",
        &edits,
        &types,
        &policies,
    )?);
    assert_eq!(
        ron::to_string(documents.scope.as_ref().ok_or("scope")?)?,
        before
    );
    assert!(!documents.can_history(false));
    assert!(!documents.can_history(true));
    Ok(())
}

#[test]
fn explicit_discard_restores_admission_and_retires_old_source_authority() -> TestResult {
    let (mut documents, target, types, policies) = documents()?;
    assert!(documents.set_node_selection(target.document, target.node.clone(), false));
    let asset = documents
        .library()
        .ok_or("library")?
        .asset("game://transform.prefab.ron")
        .ok_or("document asset")?;
    let node = documents.selection.active().ok_or("node")?.clone();
    documents.replace_selection(vec![
        kedra_editor_context::SelectionItem::Asset(asset.clone()),
        node,
    ])?;
    let old_selection = documents.selection.clone();
    let original = documents.scope.as_ref().ok_or("scope")?.source.clone();
    let edits = [PropertyEdit {
        path: bevy::reflect::ParsedPath::parse("translation")?,
        value: &Vec3::ONE,
    }];
    let change = documents.prepare_property(
        &target,
        &documents.revisions(),
        "scene.transform",
        &edits,
        &types,
        &policies,
    )?;
    documents.publish_change(change);
    assert!(!documents.update_clean());
    let change = documents.prepare_discard(&types, &policies)?;
    assert!(!documents.update_clean());
    documents.publish_change(change);
    assert!(documents.update_clean());
    assert!(!documents.can_history(false));
    assert!(!documents.can_history(true));
    assert!(documents.inspect(&target).is_err());
    assert_ne!(documents.selection, old_selection);
    assert_eq!(
        documents.selected_nodes(target.document),
        vec![target.node.clone()]
    );
    let library = documents.library().ok_or("library")?;
    let kedra_editor_context::SelectionItem::Node(old_node) =
        old_selection.active().ok_or("old node")?
    else {
        return Err("Expected node selection".into());
    };
    assert!(!library.contains_node(old_node));
    assert!(!library.contains_asset(&asset));
    assert!(
        matches!(&documents.selection.items()[0], kedra_editor_context::SelectionItem::Asset(current) if current.path() == asset.path() && library.contains_asset(current))
    );
    assert!(documents.selection_target().is_some());
    let new_target = documents
        .target(target.document, target.node.clone())
        .ok_or("new target")?;
    assert_eq!(
        documents
            .inspect(&new_target)?
            .value("scene.transform", &types, &policies)?
            .try_downcast_ref::<Transform>()
            .ok_or("Transform")?
            .translation,
        Vec3::ZERO
    );
    assert_eq!(
        ron::to_string(&documents.scope.as_ref().ok_or("scope")?.source)?,
        ron::to_string(&original)?
    );
    Ok(())
}

#[test]
fn discard_prunes_selected_nodes_that_only_exist_in_working_changes() -> TestResult {
    use kedra_authoring::transaction::EditTransaction;
    use kedra_scene::document::{Node, NodeId};
    use std::collections::BTreeSet;
    let (mut documents, target, types, policies) = documents()?;
    let child = NodeId::new();
    let working = &mut documents.scope.as_mut().ok_or("scope")?.working;
    let mut edit = EditTransaction::begin(
        working,
        "Add working child",
        BTreeSet::from([target.document]),
    )?;
    edit.edit(target.document, |document| {
        document.nodes.push(Node {
            id: child,
            name: "Working child".into(),
            parent: Some(document.root.into()),
            components: BTreeMap::new(),
        });
        Ok(())
    })?;
    edit.commit(working, |_| Ok(()))?;
    assert!(documents.set_node_selection(target.document, child.into(), false));
    let selected = documents.selection.clone();
    let baseline = ron::to_string(&documents.scope.as_ref().ok_or("scope")?.source)?;
    let discarded = documents.prepare_discard(&types, &policies)?;
    documents.publish_change(discarded);
    assert!(documents.selection.items().is_empty());
    assert!(documents.selection_target().is_none());
    assert!(!documents.can_history(false));
    assert!(documents.update_clean());
    let library = documents.library().ok_or("library")?;
    let kedra_editor_context::SelectionItem::Node(old_node) =
        selected.active().ok_or("old node")?
    else {
        return Err("Expected node selection".into());
    };
    assert!(!library.contains_node(old_node));
    assert_eq!(
        ron::to_string(&documents.scope.as_ref().ok_or("scope")?.source)?,
        baseline
    );
    Ok(())
}
