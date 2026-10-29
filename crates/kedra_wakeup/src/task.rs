//! Owned readiness registration, without another executor or task registry.
use crate::Wakeup;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

/// A Bevy task with automatic host readiness registration and one owned result.
///
/// Created with [`Wakeup::spawn`] or [`Wakeup::track`]. No initial main-thread
/// poll is needed for completion to wake the host. Drop cancels the underlying
/// Bevy task (not a rollback of work already performed); [`Self::cancel`] awaits
/// its cancellation. Results never apply themselves or bypass caller admission.
/// Headless handles can be polled manually or awaited. Worker panics retain
/// Bevy's propagation semantics. Repeated nonblocking polls never repeat output;
/// awaiting again after output consumption violates the Future contract and panics.
#[must_use = "Dropping the handle cancels its Bevy task"]
pub struct Task<T> {
    pending: Option<bevy::tasks::Task<T>>,
    ready: Option<T>,
    wakeup: Wakeup,
}

// The Bevy Task is Unpin. A retained output is never pinned or exposed by pin.
impl<T> Unpin for Task<T> {}

impl<T> Task<T> {
    pub(crate) fn new(task: bevy::tasks::Task<T>, wakeup: Wakeup) -> Self {
        let mut this = Self {
            pending: Some(task),
            ready: None,
            wakeup,
        };
        // Register on the actual Task Future: a worker-side notify before return
        // could race ahead of publication. Retain completion if it already won.
        this.ready = this.poll();
        if this.ready.is_some() {
            this.wakeup.notify();
        }
        this
    }

    /// Poll once without blocking, retrying or applying the result.
    ///
    /// Returns `None` while pending and transfers output exactly once. The host
    /// waker remains registered. Remove the handle after receiving `Some`.
    /// A consumed handle returns `None`; it never replays the result or work.
    pub fn poll(&mut self) -> Option<T> {
        let waker = self.wakeup.waker.clone();
        self.poll_with(&waker)
    }

    fn poll_with(&mut self, waker: &Waker) -> Option<T> {
        if let Some(value) = self.ready.take() {
            return Some(value);
        }
        let task = self.pending.as_mut()?;
        match Pin::new(task).poll(&mut Context::from_waker(waker)) {
            Poll::Ready(value) => {
                self.pending = None;
                Some(value)
            }
            Poll::Pending => None,
        }
    }

    /// Observe readiness without consuming output or driving execution.
    /// Returns true after consumption too; it does not mean output is still owned.
    pub fn is_finished(&self) -> bool {
        self.pending
            .as_ref()
            .is_none_or(bevy::tasks::Task::is_finished)
    }

    /// Cancel and await the existing Bevy task, returning output if it already won.
    ///
    /// Consumes this handle. Already-performed external effects are not undone.
    /// Dropping this cancellation future retains Bevy's cancellation-on-drop.
    pub async fn cancel(mut self) -> Option<T> {
        match self.pending.take() {
            Some(task) => task.cancel().await,
            None => self.ready.take(),
        }
    }
}

impl<T> Future for Task<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let this = self.get_mut();
        assert!(
            this.pending.is_some() || this.ready.is_some(),
            "Task output already consumed"
        );
        // An awaiting executor must wake as well as the host. It may replace the
        // original registration; never silently strand block_on/async callers.
        let waker = Waker::from(Arc::new(Awaiting {
            host: this.wakeup.waker.clone(),
            caller: cx.waker().clone(),
        }));
        this.poll_with(&waker).map_or(Poll::Pending, Poll::Ready)
    }
}

struct Awaiting {
    host: Waker,
    caller: Waker,
}
impl Wake for Awaiting {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.host.wake_by_ref();
        if !self.host.will_wake(&self.caller) {
            self.caller.wake_by_ref();
        }
    }
}
