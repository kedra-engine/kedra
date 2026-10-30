//! The actual wait bootstrap under a controlled monotonic clock, without sleeps.
use crate::lua::{self, Lua};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

fn host(clock: Arc<AtomicU64>) -> lua::Result<Lua> {
    let lua = Lua::new();
    let runtime = lua.create_table()?;
    runtime.set(
        "elapsed_ms",
        lua.create_function(move |_, ()| Ok(clock.load(Ordering::Relaxed) as f64))?,
    )?;
    lua.globals().set("runtime", runtime)?;
    lua.load(include_str!("../wait.luau")).exec()?;
    Ok(lua)
}

#[test]
fn predicate_success_is_not_accepted_at_or_after_its_deadline() -> lua::Result<()> {
    for completed_at in [4, 5, 6] {
        let clock = Arc::new(AtomicU64::new(0));
        let lua = host(clock.clone())?;
        let predicate = lua.create_function(move |_, ()| {
            clock.store(completed_at, Ordering::Relaxed);
            Ok(true)
        })?;
        let wait: lua::Table = lua.globals().get("wait")?;
        let until: lua::Function = wait.get("until_true")?;
        let result = until.call::<()>(("target", predicate, 5));
        assert_eq!(result.is_ok(), completed_at < 5);
        if let Err(error) = result {
            assert!(error.to_string().contains("Timed out waiting for target"));
        }
    }
    Ok(())
}

#[test]
fn expired_wait_does_not_invoke_its_predicate_again_after_resumption() -> lua::Result<()> {
    let clock = Arc::new(AtomicU64::new(0));
    let calls = Arc::new(AtomicU64::new(0));
    let lua = host(clock.clone())?;
    let observed = calls.clone();
    let predicate = lua.create_function(move |_, ()| {
        observed.fetch_add(1, Ordering::Relaxed);
        Ok(false)
    })?;
    let run: lua::Function = lua
        .load("return function(predicate) wait.until_true('target', predicate, 5) end")
        .eval()?;
    let thread = lua.create_thread(run)?;
    let _: lua::Value = thread.resume(predicate)?;
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    clock.store(5, Ordering::Relaxed);
    let error = thread
        .resume::<lua::Value>(())
        .err()
        .ok_or_else(|| lua::Error::runtime("Expired wait resumed successfully"))?;
    assert!(error.to_string().contains("Timed out waiting for target"));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    Ok(())
}
