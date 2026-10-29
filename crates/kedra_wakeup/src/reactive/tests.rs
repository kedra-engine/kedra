//! Adapter policy: earliest deadline, retired hints and stable absolute identity.
use super::*;

#[test]
fn live_runtime_overrides_idle_and_deadlines_until_its_owner_stops_renewing() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ReactivePlugin {
        idle: Duration::from_millis(100),
        focused_active: Duration::from_millis(16),
        unfocused_active: Duration::from_millis(100),
    });
    let mut hints = app.world_mut().resource_mut::<WakeDemand>();
    hints.at(Instant::now() + Duration::from_secs(10));
    hints.continuous();
    app.world_mut().run_schedule(Last);
    let settings = app.world().resource::<WinitSettings>();
    assert_eq!(settings.focused_mode, UpdateMode::Continuous);
    assert_eq!(settings.unfocused_mode, UpdateMode::Continuous);
    // The runtime remains live even without changed entities or pending tasks.
    app.world_mut().resource_mut::<WakeDemand>().continuous();
    app.world_mut().run_schedule(Last);
    assert_eq!(
        app.world().resource::<WinitSettings>().focused_mode,
        UpdateMode::Continuous
    );
    // Hiding/retirement releases demand, rather than leaving a sticky redraw loop.
    app.world_mut().run_schedule(Last);
    let settings = app.world().resource::<WinitSettings>();
    assert_eq!(
        settings.focused_mode,
        UpdateMode::reactive_low_power(Duration::from_millis(100))
    );
    assert_eq!(settings.unfocused_mode, settings.focused_mode);
}

#[test]
fn deadlines_only_shorten_wait_and_same_target_keeps_its_identity() {
    let now = Instant::now();
    let idle = Duration::from_millis(100);
    let active = Duration::from_millis(16);
    let mut demand = WakeDemand::default();
    demand.at(now + Duration::from_millis(350));
    assert_eq!(choose(&demand, now, idle, active).1, idle);
    let near = now + Duration::from_millis(300);
    let (key, remaining) = choose(&demand, near, idle, active);
    assert_eq!(remaining, Duration::from_millis(50));
    let (same, _) = choose(&demand, near + Duration::from_millis(1), idle, active);
    assert!(key == same);
    demand.active();
    assert_eq!(choose(&demand, near, idle, active).1, active);
    demand.at(near);
    assert_eq!(choose(&demand, near, idle, active).1, Duration::ZERO);
}

#[test]
fn earliest_hint_wins_and_unrenewed_work_returns_to_idle_without_stale_deadline() {
    let now = Instant::now();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ReactivePlugin {
        idle: Duration::MAX,
        focused_active: Duration::from_millis(16),
        unfocused_active: Duration::from_millis(100),
    });
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .update_with_instant(now);
    let mut hints = app.world_mut().resource_mut::<WakeDemand>();
    hints.at(now + Duration::from_secs(10));
    hints.at(now + Duration::from_secs(20));
    app.world_mut().run_schedule(Last);
    let first = app.world().resource::<WinitSettings>().focused_mode;
    assert!(
        matches!(first, UpdateMode::Reactive { wait, .. } if wait > Duration::ZERO && wait <= Duration::from_secs(10))
    );
    // Renewing the same absolute target on the next frame must not keep changing
    // WinitSettings and asking Winit for another immediate configuration redraw.
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .update_with_instant(now + Duration::from_millis(1));
    app.world_mut()
        .resource_mut::<WakeDemand>()
        .at(now + Duration::from_secs(10));
    app.world_mut().run_schedule(Last);
    assert_eq!(app.world().resource::<WinitSettings>().focused_mode, first);
    app.world_mut().run_schedule(Last);
    let settings = app.world().resource::<WinitSettings>();
    assert_eq!(
        settings.focused_mode,
        UpdateMode::reactive_low_power(Duration::MAX)
    );
    assert_eq!(settings.unfocused_mode, settings.focused_mode);
}
