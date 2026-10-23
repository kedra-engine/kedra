//! Runtime owns validated, independent and deeply immutable parameter snapshots.
#![allow(clippy::unwrap_used, reason = "Isolated parameter contract assertions")]
use crate::support::*;
use kedra_script::{ErrorKind, Program};
use serde_json::{Value, json};

fn validate(value: &Value) -> Result<(), String> {
    if value["config"]["speed"]
        .as_u64()
        .is_some_and(|speed| speed <= 10)
    {
        Ok(())
    } else {
        Err("Expected config.speed in 0..=10".into())
    }
}

#[test]
fn invalid_parameters_preserve_live_generation_and_allocate_no_owner() {
    let mut app = app();
    let old = start(&mut app, "function run(ctx) wait.updates(10) end").unwrap();
    let count = app.world().entities().len();
    let mut program =
        Program::behavior("params", "params.luau", "error('must not load')", game_api);
    program.params = json!({"config":{"speed":11}});
    program.validate_params = validate;
    let error =
        kedra_script::replace(app.world_mut(), old, program.clone(), Value::Null).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidInput);
    assert_eq!(error.phase.as_deref(), Some("admission"));
    assert_eq!(app.world().entities().len(), count);
    assert_eq!(
        kedra_script::status(app.world(), old),
        Some(&kedra_script::Status::Running)
    );
    program.validate_params = |_| Ok(());
    program.params = json!([]);
    assert_eq!(
        kedra_script::start(app.world_mut(), program, Value::Null)
            .unwrap_err()
            .kind,
        ErrorKind::InvalidInput
    );
    assert_eq!(app.world().resource::<Counter>().0, 0);
}

#[test]
fn nested_parameters_are_readonly_and_separate_from_sequence_arguments() {
    let mut app = app();
    let mut program = Program::sequence(
        "params",
        "params.luau",
        r#"
        function run(ctx, args)
            assert(not pcall(function() ctx.params.config.speed = 99 end))
            assert(not pcall(function() ctx.params.config = {} end))
            assert(not pcall(function() ctx.params = {} end))
            return {speed = ctx.params.config.speed, argument = args.speed}
        end
    "#,
        game_api,
    );
    program.params = json!({"config":{"speed":3}});
    program.validate_params = validate;
    let run = kedra_script::start(app.world_mut(), program, json!({"speed":7})).unwrap();
    assert_eq!(
        finish(&mut app, run).unwrap(),
        json!({"speed":3,"argument":7})
    );
}
