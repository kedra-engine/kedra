//! Frame-scoped subsystem hints, not a task/timer registry.
use bevy::prelude::*;
use std::time::{Duration, Instant};

/// Ordering seam for the opt-in host adapter in Last.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WakeSystems {
    /// Consume the frame's hints after all Last producers have run.
    Apply,
}

/// Frame-scoped demand supplied by subsystem integrations, not individual controls.
///
/// Owners request active updates or their earliest absolute monotonic deadline
/// during Update/PostUpdate (Last producers must run before [`WakeSystems::Apply`]).
/// The optional native `ReactivePlugin` drains hints in Last. Renew demand each
/// frame while relevant; cancellation/retirement simply stops renewing it. This
/// owns no timers, callbacks, tasks or entities and is inert without a host adapter.
/// Deadlines do not execute work: they only arrange another ordinary Bevy frame.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct WakeDemand {
    pub(crate) active: bool,
    pub(crate) continuous: bool,
    pub(crate) deadline: Option<Instant>,
}

impl WakeDemand {
    /// Keep the host's configured active cadence for work without a deadline seam.
    pub fn active(&mut self) {
        self.active = true;
    }

    /// Keep ordinary continuous Bevy updates while a live runtime/view requires
    /// them, even without changed properties, input or pending resource work.
    ///
    /// Unlike `active()`, this is not a polling cadence: the native adapter must
    /// not sleep between simulation/render frames. Renew at the owning runtime
    /// boundary, not in each effect. Retirement/hiding stops renewal. With a
    /// shared App this necessarily keeps that entire App running; it does not
    /// isolate worlds, establish a fixed simulation tickrate or cap rendering.
    pub fn continuous(&mut self) {
        self.continuous = true;
    }

    /// Request a frame by the earliest supplied deadline. Past deadlines are due
    /// immediately; hosts can wake late under OS/load constraints. Owners must
    /// service or retire an expired deadline to avoid demanding repeated frames.
    pub fn at(&mut self, deadline: Instant) {
        self.deadline = Some(self.deadline.map_or(deadline, |old| old.min(deadline)));
    }

    /// Convert a Bevy real-clock elapsed deadline using its actual clock origin.
    ///
    /// Bevy elapsed time starts at the first update, not resource construction or
    /// `startup()`. This shared conversion avoids early deadlines after slow app
    /// initialization. An uninitialized clock retains active demand until it is
    /// initialized. An overflowing/unrepresentable deadline has no finite wake.
    /// Paused/scaled game clocks must use their owning subsystem's active fallback.
    pub fn at_real(&mut self, time: &Time<Real>, elapsed: Duration) {
        let Some(origin) = time.first_update() else {
            self.active();
            return;
        };
        if let Some(deadline) = origin.checked_add(elapsed) {
            self.at(deadline);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_deadline_uses_first_update_not_app_construction_and_keeps_uninitialized_clock_active() {
        let startup = Instant::now();
        let mut time = Time::<Real>::new(startup);
        let mut demand = WakeDemand::default();
        demand.at_real(&time, Duration::from_secs(2));
        assert!(demand.active);
        time.update_with_instant(startup + Duration::from_secs(5));
        time.update_with_instant(startup + Duration::from_secs(6));
        let mut demand = WakeDemand::default();
        demand.at_real(&time, Duration::from_secs(2));
        assert!(!demand.active);
        assert_eq!(demand.deadline, Some(startup + Duration::from_secs(7)));
        time.update_with_instant(startup + Duration::from_millis(6500));
        let mut same = WakeDemand::default();
        same.at_real(&time, Duration::from_secs(2));
        assert_eq!(
            same, demand,
            "Renewal must retain one absolute deadline identity"
        );
    }
}
