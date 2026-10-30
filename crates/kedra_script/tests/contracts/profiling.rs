//! Standard scoped instrumentation through real Luau execution, without a recorder.
use super::support;
use kedra_script::Status;

#[test]
fn scopes_and_yielding_operations_preserve_return_values_and_errors() -> Result<(), String> {
    let mut app = support::app();
    let run = support::start(
        &mut app,
        r#"
function run(ctx)
    local a, b, c = ctx.profiler.scope("synchronous", function(value)
        ctx.profiler.counter("objects", 12, "count")
        return value, nil, "日本語"
    end, 7)
    assert(a == 7 and b == nil and c == "日本語")
    local result, missing, tail = ctx.profiler.operation("waiting", function()
        wait.updates(2)
        ctx.game.increment()
        return 9, nil, "tail"
    end)
    assert(result == 9 and missing == nil and tail == "tail")
    local ok, err = pcall(function()
        ctx.profiler.operation("failed", function() error("owned failure") end)
    end)
    assert(not ok and string.find(tostring(err), "owned failure"))
    return ctx.game.count()
end
"#,
    )?;
    assert_eq!(support::finish(&mut app, run)?, serde_json::json!(1));
    Ok(())
}

#[test]
fn invalid_samples_units_and_cpu_scope_yields_cannot_apply_later_effects() -> Result<(), String> {
    let mut app = support::app();
    let run = support::start(
        &mut app,
        r#"
function run(ctx)
    ctx.profiler.counter("objects", 12, "count")
    assert(not pcall(function() ctx.profiler.counter("objects", 3, "bytes") end))
    assert(not pcall(function() ctx.profiler.counter("objects", math.huge, "count") end))
    assert(not pcall(function() ctx.profiler.counter("", 3, "count") end))
    assert(not pcall(function() ctx.profiler.counter("objects", 3, "unknown") end))
    ctx.profiler.counter("objects", 13, "count")
    local ok = pcall(function()
        ctx.profiler.scope("cannot suspend", function()
            wait.updates(2)
            ctx.game.increment()
        end)
    end)
    assert(not ok)
    assert(not pcall(function() ctx.profiler.counter = function() end end))
    return ctx.game.count()
end
"#,
    )?;
    assert_eq!(support::finish(&mut app, run)?, serde_json::json!(0));
    Ok(())
}

#[test]
fn instrumentation_capacity_and_cancelled_operations_do_not_poison_another_run()
-> Result<(), String> {
    let mut app = support::app();
    let run = support::start(
        &mut app,
        r#"
function run(ctx)
    for index = 1, 64 do ctx.profiler.counter("counter." .. index, index) end
    assert(not pcall(function() ctx.profiler.counter("one too many", 1) end))
    ctx.profiler.counter("counter.1", 42)
    ctx.profiler.operation("cancelled", function()
        wait.updates(100)
        ctx.game.increment()
    end)
end
"#,
    )?;
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
    kedra_script::cancel(app.world_mut(), run).map_err(|error| error.to_string())?;
    let other = support::start(
        &mut app,
        r#"
function run(ctx)
    ctx.profiler.counter("one too many", 1)
    return ctx.profiler.operation("completed", function() return ctx.game.count() end)
end
"#,
    )?;
    assert_eq!(support::finish(&mut app, other)?, serde_json::json!(0));
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Cancelled)
    );
    Ok(())
}
