//! Preserve std channel publication and errors; notification follows success.
use crate::Wakeup;
use std::sync::mpsc::{SendError, SyncSender, TrySendError};

/// An existing bounded std-channel producer with automatic host notification.
///
/// Use [`Wakeup::sender`] once at the channel boundary, then `send`/`try_send`
/// normally. This creates no queue or consumer. Capacity, ordering, disconnection
/// and backpressure remain std-owned. Full/disconnected sends do not notify or
/// retry. Drop retains ordinary sender disconnection semantics; it is not a
/// cancellation/result event. Notifications can coalesce; receivers must drain
/// their existing channel rather than assume one notification per frame.
pub struct Sender<T> {
    sender: SyncSender<T>,
    wakeup: Wakeup,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            wakeup: self.wakeup.clone(),
        }
    }
}

impl<T> Sender<T> {
    pub(crate) fn new(sender: SyncSender<T>, wakeup: Wakeup) -> Self {
        Self { sender, wakeup }
    }

    /// Publish, then notify; blocks on a full channel exactly like std `send`.
    /// A disconnected receiver returns the original value without notification.
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.sender.send(value)?;
        self.wakeup.notify();
        Ok(())
    }

    /// Publish without blocking, then notify only on success.
    /// Full/disconnected errors retain their original value, with no replay.
    pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        self.sender.try_send(value)?;
        self.wakeup.notify();
        Ok(())
    }
}
