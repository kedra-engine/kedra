//! Wake a native application when an existing task or producer has work ready.
//!
//! Task readiness uses existing Futures, not a new executor or queue. Native
//! hosts may explicitly opt into a reactive wait policy; games inherit none.
//! Task results, cancellation and admission remain with their existing owners.
mod demand;
mod sender;
mod task;
pub use demand::{WakeDemand, WakeSystems};
#[cfg(feature = "native")]
mod reactive;
use bevy::{prelude::*, tasks::TaskPool};
#[cfg(feature = "native")]
pub use reactive::ReactivePlugin;
pub use sender::Sender;
use std::{future::Future, task::Waker};
pub use task::Task;

/// Cloneable, thread-safe readiness notification for an application's event loop.
///
/// Install a value captured with [`Self::for_world`] after the native window plugin
/// to reuse its waker. The default is inert, suitable for manually driven headless
/// worlds. Notifications do not authorize, consume, retry or apply any operation.
#[derive(Resource, Clone, Debug)]
pub struct Wakeup {
    waker: Waker,
    inert: bool,
}

impl Default for Wakeup {
    fn default() -> Self {
        Self {
            waker: Waker::noop().clone(),
            inert: true,
        }
    }
}

impl Wakeup {
    /// Capture the host's installed adapter or its current native event-loop proxy.
    ///
    /// Without `native`, or without a proxy, returns an inert adapter. This does
    /// not install a plugin or resource. An installed inert default is upgraded
    /// when a proxy becomes available; already-held clones do not change. An
    /// explicitly supplied host waker is preserved rather than replaced.
    pub fn for_world(world: &World) -> Self {
        if let Some(installed) = world.get_resource::<Self>().filter(|value| !value.inert) {
            return installed.clone();
        }
        #[cfg(feature = "native")]
        if let Some(proxy) = world.get_resource::<bevy::winit::EventLoopProxyWrapper>() {
            return Self::from_waker(Waker::from(std::sync::Arc::new(NativeWake(
                (**proxy).clone(),
            ))));
        }
        Self::default()
    }

    /// Use an existing host waker without changing its notification semantics.
    ///
    /// Useful for non-Winit hosts. The adapter owns a cloneable standard waker,
    /// never the host's work or executor. Waking follows the supplied waker's
    /// behavior, including any panic behavior defined by that host.
    pub fn from_waker(waker: Waker) -> Self {
        Self {
            waker,
            inert: false,
        }
    }

    /// Notify after publishing producer work into its existing owner/channel.
    ///
    /// Notifications may coalesce and do not carry results. For the native
    /// adapter, a closed event loop discards the notification without retrying
    /// work. A worker returning a Task result should use [`Self::spawn`] instead
    /// of notifying before that Task has published completion.
    pub fn notify(&self) {
        self.waker.wake_by_ref();
    }

    /// Spawn on the supplied Bevy pool and automatically register native readiness.
    ///
    /// No initial owner poll or separate notification is required. The returned
    /// handle retains Bevy cancellation-on-drop and exclusive result ownership.
    /// No executor or thread is created; the pool and future keep Bevy's semantics.
    pub fn spawn<T: Send + 'static>(
        &self,
        pool: &TaskPool,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        self.track(pool.spawn(future))
    }

    /// Attach readiness to an already spawned Bevy task, including local tasks.
    ///
    /// Registers immediately; an already-ready output is retained and notifies
    /// the host too. Never consumes or applies the owner's result. A consumed or
    /// cancelled input task can panic, following Bevy's Future contract.
    /// This does not drive its executor: local/main-thread tasks still need their
    /// existing execution route and must not be assumed safe for indefinite sleep.
    pub fn track<T>(&self, task: bevy::tasks::Task<T>) -> Task<T> {
        Task::new(task, self.clone())
    }

    /// Attach post-publication notifications to an existing bounded std channel.
    ///
    /// Keeps the channel's capacity, backpressure and errors. All producers must
    /// use the returned sender (or its clones) to obtain automatic notification.
    pub fn sender<T>(&self, sender: std::sync::mpsc::SyncSender<T>) -> Sender<T> {
        Sender::new(sender, self.clone())
    }
}

#[cfg(feature = "native")]
struct NativeWake(bevy::winit::EventLoopProxy<bevy::winit::WinitUserEvent>);

#[cfg(feature = "native")]
impl std::task::Wake for NativeWake {
    fn wake(self: std::sync::Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &std::sync::Arc<Self>) {
        // Notification only: closed-loop failure never replays the owned work.
        let _ = self.0.send_event(bevy::winit::WinitUserEvent::WakeUp);
    }
}
