//! Editor source admission owns inventory bounds and explicit native grants.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn admitted_project_modules_run_with_readonly_native_grants() -> TestResult {
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("Editor"))?;
    std::fs::write(
        root.path().join("Editor/module.luau"),
        "return { label = '入口' }",
    )?;
    std::fs::write(
        root.path().join("Editor/main.luau"),
        r#"
        local module = require('./module')
        function run(ctx)
            assert(not pcall(function() ctx.params.sources[1] = 'game://foreign.scene.ron' end))
            assert(ctx.params.sources[1] == 'game://Main.scene.ron')
            assert(type(ctx.edit.begin) == 'function')
            return module.label
        end
    "#,
    )?;
    let program = load(
        root.path(),
        "Editor/main.luau",
        vec!["game://Main.scene.ron".into()],
    )?;
    let mut app = crate::workspace::browser_test_app()?;
    app.finish();
    app.cleanup();
    let run = kedra_script::start(app.world_mut(), program, serde_json::Value::Null)?;
    for _ in 0..100 {
        app.update();
        match kedra_script::status(app.world(), run) {
            Some(kedra_script::Status::Completed(value)) => {
                assert_eq!(value, &serde_json::json!("入口"));
                return Ok(());
            }
            Some(kedra_script::Status::Failed(error)) => return Err(error.to_string().into()),
            _ => (),
        }
    }
    Err("Project module did not finish".into())
}
#[test]
fn snapshot_loading_never_evaluates_code_and_preserves_modules_and_native_grants() -> TestResult {
    let root = tempfile::tempdir()?;
    assert!(discover(root.path())?.is_empty());
    std::fs::create_dir(root.path().join("Editor"))?;
    std::fs::write(
        root.path().join("Editor/main.luau"),
        "error('must not run during load')",
    )?;
    std::fs::write(
        root.path().join("Editor/module.luau"),
        "return { name = '入口' }",
    )?;
    std::fs::write(root.path().join("Editor/.luaurc"), "{}")?;
    std::fs::write(
        root.path().join("outside.luau"),
        "error('outside snapshot')",
    )?;
    let grants = vec!["game://Main.scene.ron".into()];
    let program = load(root.path(), "Editor/main.luau", grants.clone())?;
    assert_eq!(
        program.limits.callback_time,
        std::time::Duration::from_secs(1)
    );
    assert_eq!(
        program.limits.total_time,
        Some(std::time::Duration::from_secs(45))
    );
    assert_eq!(program.params, serde_json::json!({"sources":grants}));
    assert_eq!(
        discover(root.path())?
            .into_iter()
            .map(|script| script.path)
            .collect::<Vec<_>>(),
        ["Editor/main.luau", "Editor/module.luau"]
    );
    for entry in ["outside.luau", "Editor/../outside.luau", "Editor/MAIN.luau"] {
        assert!(load(root.path(), entry, grants.clone()).is_err());
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("Editor/module.luau"))?,
        "return { name = '入口' }"
    );
    Ok(())
}

#[test]
fn invalid_grants_and_oversized_sources_refuse_without_writing_project_files() -> TestResult {
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("Editor"))?;
    let path = root.path().join("Editor/main.luau");
    std::fs::write(&path, "return true")?;
    for grants in [
        vec![],
        vec!["editor://Main.scene.ron"],
        vec!["game://../Main.scene.ron"],
        vec!["game://Main.scene.ron", "game://Main.scene.ron"],
        vec!["game://a\\b.scene.ron"],
    ] {
        assert!(
            load(
                root.path(),
                "Editor/main.luau",
                grants.into_iter().map(str::to_owned).collect()
            )
            .is_err()
        );
    }
    assert_eq!(std::fs::read_to_string(&path)?, "return true");
    std::fs::write(&path, "x".repeat(65_537))?;
    assert!(discover(root.path()).is_err());
    assert_eq!(std::fs::metadata(path)?.len(), 65_537);
    Ok(())
}

#[cfg(unix)]
#[test]
fn linked_script_roots_and_modules_are_not_admitted() -> TestResult {
    let root = tempfile::tempdir()?;
    let foreign = tempfile::tempdir()?;
    let path = foreign.path().join("secret.luau");
    std::fs::write(&path, "return 'foreign'")?;
    std::os::unix::fs::symlink(foreign.path(), root.path().join("Editor"))?;
    assert!(discover(root.path()).is_err());
    std::fs::remove_file(root.path().join("Editor"))?;
    std::fs::create_dir(root.path().join("Editor"))?;
    std::os::unix::fs::symlink(&path, root.path().join("Editor/module.luau"))?;
    assert!(discover(root.path()).is_err());
    assert_eq!(std::fs::read_to_string(path)?, "return 'foreign'");
    Ok(())
}
