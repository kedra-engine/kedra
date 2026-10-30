//! Opt-in mapping to Bevy/Winit's existing reactive wait; no timer thread.
use crate::{WakeDemand, WakeSystems};
use bevy::{
    prelude::*,
    winit::{UpdateMode, WinitSettings},
};
use std::time::{Duration, Instant};

/// Explicit native host policy; never installed by runtime libraries on games.
///
/// Uses Winit's event loop and waits, with window/user events remaining immediate.
/// `idle` is a bounded safety poll for integrations without native readiness;
/// `Duration::MAX` is appropriate only for a host proving all of its wake routes.
/// Active hints use the supplied focused/unfocused cadences. Continuous runtime
/// demand takes precedence in both focus states; deadlines can shorten reactive
/// waits. Requires native Winit and Time plugins, and owns WinitSettings;
/// do not install another writer to that resource. This does not cap rendering
/// driven by RequestRedraw, create an executor or spawn a timer thread.
pub struct ReactivePlugin {
    /// Safety polling interval when no subsystem requests active frames.
    pub idle: Duration,
    /// Focused cadence while a subsystem requires active updates.
    pub focused_active: Duration,
    /// Unfocused cadence while a subsystem requires active updates.
    pub unfocused_active: Duration,
}

#[derive(Resource)]
struct Policy {
    idle: Duration,
    focused: Duration,
    unfocused: Duration,
}
#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum WaitKey {
    #[default]
    Idle,
    Active,
    Continuous,
    Deadline(Instant, bool),
    Near(Instant, bool),
    Due(Instant, bool),
}
#[derive(Default)]
struct Applied {
    key: Option<(WaitKey, WaitKey)>,
}

impl Plugin for ReactivePlugin {
    fn build(&self, app: &mut App) {
        assert!(
            !self.idle.is_zero()
                && !self.focused_active.is_zero()
                && !self.unfocused_active.is_zero(),
            "Reactive cadence must be positive"
        );
        app.init_resource::<WakeDemand>()
            .insert_resource(Policy {
                idle: self.idle,
                focused: self.focused_active,
                unfocused: self.unfocused_active,
            })
            .insert_resource(WinitSettings {
                focused_mode: UpdateMode::reactive_low_power(self.idle),
                unfocused_mode: UpdateMode::reactive_low_power(self.idle),
            })
            .add_systems(Last, apply.in_set(WakeSystems::Apply));
    }
}

fn choose(
    demand: &WakeDemand,
    now: Instant,
    idle: Duration,
    active: Duration,
) -> (WaitKey, Duration) {
    let base = if demand.active {
        active.min(idle)
    } else {
        idle
    };
    if let Some(deadline) = demand.deadline {
        let remaining = deadline.saturating_duration_since(now);
        if remaining.is_zero() {
            return (WaitKey::Due(deadline, demand.active), Duration::ZERO);
        }
        if remaining <= active.min(base) {
            // Winit's public API is relative to frame start, not an absolute
            // deadline. A frame may wake just before expiry. Service that final
            // window at the existing active cadence instead of repeating the
            // original long wait, without recalculating a shrinking mode each
            // immediate configuration redraw.
            return (WaitKey::Near(deadline, demand.active), active.min(base));
        }
        if remaining < base {
            return (WaitKey::Deadline(deadline, demand.active), remaining);
        }
    }
    (
        if demand.active {
            WaitKey::Active
        } else {
            WaitKey::Idle
        },
        base,
    )
}

fn apply(
    mut demand: ResMut<WakeDemand>,
    policy: Res<Policy>,
    mut settings: ResMut<WinitSettings>,
    mut applied: Local<Applied>,
) {
    // Recomputing a shrinking relative timeout every immediate redraw changes
    // UpdateMode every frame, which Winit
    // interprets as another redraw and can busy-loop. Cache absolute identity.
    let now = Instant::now();
    let hints = std::mem::take(&mut *demand);
    if hints.continuous {
        let key = (WaitKey::Continuous, WaitKey::Continuous);
        if applied.key != Some(key) {
            settings.focused_mode = UpdateMode::Continuous;
            settings.unfocused_mode = UpdateMode::Continuous;
            applied.key = Some(key);
        }
        return;
    }
    let (focused, focused_wait) = choose(&hints, now, policy.idle, policy.focused);
    let (unfocused, unfocused_wait) = choose(&hints, now, policy.idle, policy.unfocused);
    if applied.key != Some((focused, unfocused)) {
        settings.focused_mode = UpdateMode::reactive_low_power(focused_wait);
        settings.unfocused_mode = UpdateMode::reactive_low_power(unfocused_wait);
        applied.key = Some((focused, unfocused));
    }
}

#[cfg(test)]
mod tests;
