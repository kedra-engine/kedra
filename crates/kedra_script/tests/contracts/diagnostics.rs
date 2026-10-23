//! Runtime failures carry stable classes and logs retain bounded source attribution.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated diagnostic contract assertions"
)]
use crate::support::*;
use kedra_script::{Diagnostics, ErrorKind, Level, Program, Status};
use serde_json::Value;

#[test]
fn failures_and_logs_retain_program_phase_and_module_location() {
    let mut app = app();
    let mut program = Program::sequence(
        "diagnostic",
        "scripts/main.luau",
        r#"
        local report = require("./report")
        function run(ctx)
            report(ctx)
            error("Кедра failure")
        end
    "#,
        game_api,
    );
    program
        .sources
        .insert(
            "scripts/report.luau",
            "return function(ctx) ctx.log.warn('Кедра', {answer = 42}) end",
        )
        .unwrap();
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    app.update();
    let Some(Status::Failed(error)) = kedra_script::status(app.world(), run) else {
        panic!("expected failure")
    };
    assert_eq!(error.kind, ErrorKind::Runtime);
    assert_eq!(error.path.as_deref(), Some("scripts/main.luau"));
    assert_eq!(error.phase.as_deref(), Some("sequence"));
    assert!(error.message.contains("Кедра failure"));
    let records = app.world().resource::<Diagnostics>().drain();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].level, Level::Warn);
    assert_eq!(records[0].program, "diagnostic");
    assert_eq!(records[0].phase, "sequence");
    assert_eq!(records[0].source.as_deref(), Some("@scripts/report.luau"));
    assert_eq!(records[0].line, Some(1));
    assert_eq!(records[0].fields["answer"], 42);
    assert_eq!(records[1].error_kind, Some(ErrorKind::Runtime));
}

#[test]
fn diagnostic_retention_is_bounded_and_invalid_fields_have_no_effect() {
    let mut app = app();
    let run = start(
        &mut app,
        r#"
        function run(ctx)
            for i = 1, 260 do ctx.log.info(tostring(i)) end
            assert(not pcall(function() ctx.log.info("bad", function() end) end))
            assert(not pcall(function() ctx.log.info(string.rep("x", 8193)) end))
            return true
        end
    "#,
    )
    .unwrap();
    finish(&mut app, run).unwrap();
    let sink = app.world().resource::<Diagnostics>();
    let records = sink.drain();
    assert_eq!(sink.dropped(), 4);
    assert_eq!(records.len(), 256);
    assert_eq!(records.first().unwrap().message, "5");
    assert_eq!(records.last().unwrap().message, "260");
    assert_eq!(app.world().resource::<Counter>().0, 0);
}

#[test]
fn load_and_resume_exhaustion_have_distinct_machine_readable_categories() {
    let mut app = app();
    let error = kedra_script::start(
        app.world_mut(),
        Program::sequence("bad", "bad.luau", "syntax ?", game_api),
        Value::Null,
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Load);
    let mut program = Program::sequence(
        "budget",
        "budget.luau",
        "function run(ctx) wait.updates(10) end",
        game_api,
    );
    program.limits.resumes = Some(1);
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    app.update();
    app.update();
    assert!(
        matches!(kedra_script::status(app.world(), run), Some(Status::Failed(error)) if error.kind == ErrorKind::Budget)
    );
}
