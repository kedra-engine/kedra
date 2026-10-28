//! Real Luau/host/Documents integration: atomic working publication, scope and history.
use super::*;
use crate::workspace::EditorWorkspace;
use kedra_authoring::persistence::{SourceFile, SourceSnapshot, content_hash};
use kedra_scene::document::*;
use kedra_script::{
    Program, Run, Scope, Status,
    lua::{self, Lua},
};
use std::collections::BTreeSet;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn bindings(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    crate::edit_scripts::install(
        lua,
        scope,
        ctx,
        vec![
            "game://first.prefab.ron".into(),
            "game://second.prefab.ron".into(),
        ],
    )
}
fn setup() -> Result<(App, DocumentId, DocumentId), Box<dyn std::error::Error>> {
    let mut app = crate::workspace::browser_test_app()?;
    let mut first = PrefabDocument::new("First");
    first.nodes[0].name = "First".into();
    first.nodes[0].components.insert(
        "ui.caption".into(),
        ComponentData::ron(1, "(text:\"Original\",justify:Left,wrap:false)")?,
    );
    let mut second = PrefabDocument::new("Second");
    second.nodes[0].name = "Second".into();
    second.instances.push(NestedInstance {
        id: InstanceId::new(),
        asset: first.id,
        parent: second.root.into(),
        bindings: default(),
        overrides: vec![],
    });
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = app.world().resource::<ComponentPolicies>();
    let files = [("first.prefab.ron", &first), ("second.prefab.ron", &second)]
        .into_iter()
        .map(|(path, doc)| {
            let source = doc.serialize()?;
            Ok(SourceFile {
                path: path.into(),
                hash: content_hash(&source),
                source,
            })
        })
        .collect::<kedra_scene::Result<Vec<_>>>()?;
    let (scope, compiled) = source::admit(
        SourceSnapshot {
            namespace: "game".into(),
            generation: 1,
            documents: files,
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
    app.world_mut().resource_mut::<EditorWorkspace>().documents = documents;
    app.finish();
    app.cleanup();
    Ok((app, first.id, second.id))
}
fn start(app: &mut App, source: &str) -> Result<Run, String> {
    kedra_script::start(
        app.world_mut(),
        Program::sequence("source edit", "source-edit.luau", source, bindings),
        serde_json::Value::Null,
    )
    .map_err(|error| error.to_string())
}
fn finish(app: &mut App, run: Run) -> Result<serde_json::Value, String> {
    for _ in 0..100 {
        app.update();
        match kedra_script::status(app.world(), run) {
            Some(Status::Completed(value)) => return Ok(value.clone()),
            Some(Status::Failed(error)) => return Err(error.to_string()),
            Some(Status::Cancelled) | None => return Err("Source edit retired".into()),
            _ => (),
        }
    }
    Err("Source edit did not finish in 100 updates".into())
}
fn baseline(app: &App) -> Result<String, Box<dyn std::error::Error>> {
    Ok(ron::to_string(
        &app.world()
            .resource::<EditorWorkspace>()
            .documents
            .scope
            .as_ref()
            .ok_or("scope")?
            .working,
    )?)
}
fn history(app: &mut App, redo: bool) -> Result<(), String> {
    // Same Documents history used by native commands, not a script-owned journal.
    app.world_mut()
        .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            let registry = world.resource::<AppTypeRegistry>().clone();
            let types = registry.read();
            let change = state.documents.prepare_history(
                redo,
                &types,
                world.resource::<ComponentPolicies>(),
            )?;
            state.documents.publish_change(change);
            Ok(())
        })
}

#[test]
fn structural_requests_publish_one_batch_and_rejections_preserve_the_candidate() -> TestResult {
    let (mut app, first, second) = setup()?;
    let before = baseline(&app)?;
    let run = start(
        &mut app,
        r#"
        function run(ctx)
            local edit = ctx.edit.begin("Remove caption")
            local nodes = edit:nodes("game://first.prefab.ron")
            local root = nodes[1]
            assert(not pcall(function() edit:remove_node(root) end), "root removal accepted")
            assert(not pcall(function() edit:reparent(root, root) end), "cycle accepted")
            assert(not pcall(function() edit:remove_component(root, "unknown") end))
            local foreign = edit:nodes("game://second.prefab.ron")[1]
            assert(not pcall(function() edit:reparent(root, foreign) end))
            edit:remove_component(root, "ui.caption")
            return edit:commit()
        end
    "#,
    )?;
    assert!(finish(&mut app, run)?.is_string());
    let state = app.world().resource::<EditorWorkspace>();
    assert!(
        !state.documents.compiled[&first]
            .graph()
            .nodes
            .values()
            .any(|node| node.components.contains_key("ui.caption"))
    );
    assert!(
        !state.documents.compiled[&second]
            .graph()
            .nodes
            .values()
            .any(|node| node.components.contains_key("ui.caption"))
    );
    history(&mut app, false)?;
    // Revisions advance, but source content is restored and saved content is clean.
    assert_ne!(baseline(&app)?, before);
    let state = app.world().resource::<EditorWorkspace>();
    assert!(
        !state
            .documents
            .scope
            .as_ref()
            .ok_or("scope")?
            .working
            .dirty(first)
    );
    assert!(
        state.documents.compiled[&first]
            .graph()
            .nodes
            .values()
            .any(|node| node.components.contains_key("ui.caption"))
    );
    assert!(!state.documents.can_history(false));
    Ok(())
}
#[test]
fn authorized_batch_commits_once_and_native_history_restores_without_rerunning_luau() -> TestResult
{
    let (mut app, first, second) = setup()?;
    let source = ron::to_string(
        &app.world()
            .resource::<EditorWorkspace>()
            .documents
            .scope
            .as_ref()
            .ok_or("scope")?
            .source,
    )?;
    let run = start(
        &mut app,
        r#"
        function run(ctx)
            local edit = ctx.edit.begin("Arrange prefabs")
            local first = edit:nodes("game://first.prefab.ron")[1]
            edit:set_property(first, "ui.caption", "text", "入口 · 日本語")
            for _, node in edit:nodes("game://second.prefab.ron") do
                if node.name == "Second" then edit:rename(node, "Exit") end
            end
            wait.updates(3)
            return {operation = edit:commit()}
        end
    "#,
    )?;
    let before = baseline(&app)?;
    app.update();
    assert_eq!(baseline(&app)?, before, "staging/yield cannot publish");
    let result = finish(&mut app, run)?;
    assert!(result["operation"].is_string());
    let state = app.world().resource::<EditorWorkspace>();
    let documents = &state.documents;
    let entry = documents
        .scope
        .as_ref()
        .ok_or("scope")?
        .working
        .undo_entry()
        .ok_or("undo")?;
    assert_eq!(entry.label, "Arrange prefabs");
    assert_eq!(entry.documents, BTreeSet::from([first, second]));
    assert_eq!(
        entry.id.0.to_string(),
        result["operation"].as_str().ok_or("id")?
    );
    assert_eq!(
        ron::to_string(&documents.scope.as_ref().ok_or("scope")?.source)?,
        source
    );
    let accepted = documents
        .scope
        .as_ref()
        .ok_or("scope")?
        .working
        .catalog()
        .documents;
    assert!(
        documents.compiled[&second]
            .graph()
            .nodes
            .values()
            .any(|node| node.name == "Exit")
    );
    history(&mut app, false)?;
    assert!(
        !app.world()
            .resource::<EditorWorkspace>()
            .documents
            .can_history(false)
    );
    history(&mut app, true)?;
    assert_eq!(
        app.world()
            .resource::<EditorWorkspace>()
            .documents
            .scope
            .as_ref()
            .ok_or("scope")?
            .working
            .catalog()
            .documents,
        accepted
    );
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Completed(result))
    );
    Ok(())
}

#[test]
fn exceptions_cancellation_and_foreign_handles_cannot_publish_staged_sources() -> TestResult {
    let (mut app, _, _) = setup()?;
    let before = baseline(&app)?;
    for tail in [
        "error('abort')",
        "edit:cancel(); return {}",
        "return {}",
        r#"
        local other = ctx.edit.begin("Foreign")
        other:rename(node, "Must reject")
    "#,
    ] {
        let script = format!(
            r#"function run(ctx)
            local edit = ctx.edit.begin("Unpublished")
            local node = edit:nodes("game://first.prefab.ron")[1]
            edit:set_property(node, "ui.caption", "text", "Unpublished")
            {tail}
        end"#
        );
        let run = start(&mut app, &script)?;
        let result = finish(&mut app, run);
        if tail.contains("error(") || tail.contains("other:") {
            assert!(result.is_err());
        } else {
            result?;
        }
        assert_eq!(baseline(&app)?, before);
    }
    let run = start(
        &mut app,
        r#"function run(ctx)
        local edit = ctx.edit.begin("Revoked")
        edit:rename(edit:nodes("game://first.prefab.ron")[1], "Never publish")
        wait.updates(20)
        edit:commit()
    end"#,
    )?;
    app.update();
    kedra_script::cancel(app.world_mut(), run)?;
    for _ in 0..25 {
        app.update();
    }
    assert_eq!(baseline(&app)?, before);
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Cancelled)
    );
    Ok(())
}

#[test]
fn no_op_preserves_redo_and_closed_ungranted_or_oversized_requests_cannot_change_it() -> TestResult
{
    let (mut app, _, _) = setup()?;
    let run = start(
        &mut app,
        r#"function run(ctx)
        local edit = ctx.edit.begin("Rename")
        edit:rename(edit:nodes("game://first.prefab.ron")[1], "Accepted")
        edit:commit()
    end"#,
    )?;
    finish(&mut app, run)?;
    history(&mut app, false)?;
    let before = baseline(&app)?;
    let run = start(
        &mut app,
        r#"function run(ctx)
        local edit = ctx.edit.begin("No-op")
        assert(not pcall(function() edit:nodes("game://not-granted.prefab.ron") end))
        local node = edit:nodes("game://first.prefab.ron")[1]
        assert(not pcall(function() edit:set_property(node, "ui.caption", "text", string.rep("x", 65537)) end))
        assert(edit:commit() == nil)
        assert(not pcall(function() edit:rename(node, "Closed") end))
        return {}
    end"#,
    )?;
    finish(&mut app, run)?;
    assert_eq!(baseline(&app)?, before);
    assert!(
        app.world()
            .resource::<EditorWorkspace>()
            .documents
            .can_history(true)
    );
    Ok(())
}

#[test]
fn resumed_lease_rejects_intervening_native_source_changes() -> TestResult {
    let (mut app, first, _) = setup()?;
    let run = start(
        &mut app,
        r#"function run(ctx)
        local edit = ctx.edit.begin("Stale")
        edit:rename(edit:nodes("game://first.prefab.ron")[1], "Staged")
        wait.updates(3)
        edit:commit()
    end"#,
    )?;
    app.update();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let policies = world.resource::<ComponentPolicies>();
            let mut native = state
                .documents
                .begin_edit("Native rename", &["game://first.prefab.ron".into()])?;
            let node = native
                .nodes(
                    state.documents.scope.as_ref().ok_or("scope")?,
                    "game://first.prefab.ron",
                )?
                .remove(0);
            assert_eq!(node.document, first);
            native.rename(
                state.documents.scope.as_ref().ok_or("scope")?,
                &node,
                "Native",
                &types,
                policies,
            )?;
            state.documents.commit_edit(&mut native, &types, policies)?;
            Ok(())
        },
    )?;
    let before_resume = baseline(&app)?;
    assert!(finish(&mut app, run).is_err());
    assert_eq!(baseline(&app)?, before_resume);
    Ok(())
}

#[test]
fn retired_source_session_cannot_be_reauthorized_by_beginning_again_in_the_same_run() -> TestResult
{
    let (mut app, _, _) = setup()?;
    let run = start(
        &mut app,
        r#"function run(ctx)
        ctx.edit.begin("Initial grant"):cancel()
        wait.updates(3)
        local edit = ctx.edit.begin("Must not regrant")
        edit:rename(edit:nodes("game://first.prefab.ron")[1], "Foreign session")
        edit:commit()
    end"#,
    )?;
    app.update();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let change = state
                .documents
                .prepare_discard(&types, world.resource::<ComponentPolicies>())?;
            state.documents.publish_change(change);
            Ok(())
        },
    )?;
    let before_resume = baseline(&app)?;
    let error = finish(&mut app, run)
        .err()
        .ok_or("retired source grant should refuse")?;
    assert!(error.contains("Script source grant retired"), "{error}");
    assert_eq!(baseline(&app)?, before_resume);
    Ok(())
}

#[test]
fn nested_property_edit_stays_in_receiving_prefab_and_failed_typed_request_preserves_candidate()
-> TestResult {
    let (mut app, first, second) = setup()?;
    let original = app
        .world()
        .resource::<EditorWorkspace>()
        .documents
        .scope
        .as_ref()
        .ok_or("scope")?
        .working
        .catalog()
        .documents[&first]
        .clone();
    let run = start(
        &mut app,
        r#"function run(ctx)
        local edit = ctx.edit.begin("Instance caption")
        for _, node in edit:nodes("game://second.prefab.ron") do
            if node.name == "First" then
                assert(not pcall(function() edit:set_property(node, "ui.caption", "wrap", "not a boolean") end))
                edit:set_property(node, "ui.caption", "text", "Instance only")
            end
        end
        return {operation = edit:commit()}
    end"#,
    )?;
    finish(&mut app, run)?;
    let state = app.world().resource::<EditorWorkspace>();
    let working = &state.documents.scope.as_ref().ok_or("scope")?.working;
    assert_eq!(working.catalog().documents[&first], original);
    assert_eq!(
        working.undo_entry().ok_or("undo")?.documents,
        BTreeSet::from([second])
    );
    let node = state.documents.compiled[&second]
        .graph()
        .nodes
        .iter()
        .find(|(_, node)| node.components.contains_key("ui.caption"))
        .ok_or("nested caption")?
        .0;
    let target = state
        .documents
        .target(second, node.clone())
        .ok_or("target")?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let types = registry.read();
    let inspection = state.documents.inspect(&target)?;
    let caption = inspection
        .value(
            "ui.caption",
            &types,
            app.world().resource::<ComponentPolicies>(),
        )?
        .try_downcast_ref::<kedra_ui::authored::components::UiCaption>()
        .ok_or("caption")?;
    assert_eq!(caption.text, "Instance only");
    assert!(!caption.wrap);
    Ok(())
}
