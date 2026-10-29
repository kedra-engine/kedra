//! The adapter registers host readiness without consuming or applying owned work.
use bevy::tasks::{TaskPoolBuilder, block_on};
use kedra_wakeup::Wakeup;
use std::{
    sync::{Arc, mpsc},
    task::{Wake, Waker},
    time::{Duration, Instant},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Notification(mpsc::Sender<()>);
impl Wake for Notification {
    fn wake(self: Arc<Self>) {
        let _ = self.0.send(());
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.0.send(());
    }
}

fn host() -> (Wakeup, mpsc::Receiver<()>) {
    let (send, receive) = mpsc::channel();
    (
        Wakeup::from_waker(Waker::from(Arc::new(Notification(send)))),
        receive,
    )
}

fn finished<T>(task: &bevy::tasks::Task<T>) -> Result {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !task.is_finished() {
        if Instant::now() >= deadline {
            return Err("Owned task did not finish".into());
        }
        std::thread::yield_now();
    }
    Ok(())
}

#[test]
fn task_notifies_without_an_initial_owner_poll_and_keeps_its_result() -> Result {
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let (release, gate) = mpsc::channel();
    let (wakeup, notifications) = host();
    let mut task = wakeup.spawn(
        &pool,
        async move { gate.recv_timeout(Duration::from_secs(5)) },
    );
    release.send("owned result")?;
    notifications.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(task.poll().transpose()?, Some("owned result"));
    assert!(
        task.poll().is_none(),
        "Consumed output must never be replayed"
    );
    Ok(())
}

#[test]
fn completion_before_registration_is_returned_without_waiting_for_another_wake() -> Result {
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let task = pool.spawn(async { 17 });
    finished(&task)?;
    let (wakeup, notifications) = host();
    let mut task = wakeup.track(task);
    notifications.recv_timeout(Duration::from_secs(5))?;
    assert!(task.is_finished());
    assert_eq!(task.poll(), Some(17));
    Ok(())
}

#[test]
fn worker_failure_is_not_retried_or_reinterpreted() -> Result {
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let (release, gate) = mpsc::channel();
    let (wakeup, notifications) = host();
    let mut task = wakeup.spawn(&pool, async move {
        gate.recv_timeout(Duration::from_secs(5))
            .map(|()| Err::<(), _>("worker refused"))
    });
    release.send(())?;
    notifications.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(task.poll().transpose()?, Some(Err("worker refused")));
    Ok(())
}

#[test]
fn producer_notification_does_not_drain_the_owners_channel() -> Result {
    let (wakeup, notifications) = host();
    let (producer, consumer) = mpsc::sync_channel(1);
    let mut world = bevy::prelude::World::new();
    world.insert_resource(wakeup);
    let producer = Wakeup::for_world(&world).sender(producer);
    producer.send("published progress")?;
    notifications.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(consumer.try_recv()?, "published progress");
    Ok(())
}

#[test]
fn headless_world_retains_manual_task_polling_without_a_native_proxy() -> Result {
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let wakeup = Wakeup::for_world(&bevy::prelude::World::new());
    let task = wakeup.spawn(&pool, async { "headless" });
    assert_eq!(block_on(task), "headless");
    Ok(())
}

#[test]
fn full_or_disconnected_progress_is_preserved_without_notification_or_retry() -> Result {
    let (wakeup, notifications) = host();
    let (producer, consumer) = mpsc::sync_channel(1);
    let producer = wakeup.sender(producer);
    producer.clone().try_send("published")?;
    notifications.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(
        producer.try_send("full"),
        Err(mpsc::TrySendError::Full("full"))
    );
    assert_eq!(notifications.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(consumer.try_recv()?, "published");
    assert_eq!(consumer.try_recv(), Err(mpsc::TryRecvError::Empty));
    drop(consumer);
    assert_eq!(
        producer.try_send("disconnected"),
        Err(mpsc::TrySendError::Disconnected("disconnected"))
    );
    assert_eq!(producer.send("closed"), Err(mpsc::SendError("closed")));
    assert_eq!(notifications.try_recv(), Err(mpsc::TryRecvError::Empty));
    Ok(())
}

#[test]
fn awaiting_keeps_both_host_and_executor_readiness_registered() -> Result {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let (wakeup, notifications) = host();
    let (caller, caller_notifications) = mpsc::channel();
    let caller = Waker::from(Arc::new(Notification(caller)));
    let (release, gate) = mpsc::channel();
    let mut task = wakeup.spawn(
        &pool,
        async move { gate.recv_timeout(Duration::from_secs(5)) },
    );
    assert!(matches!(
        Pin::new(&mut task).poll(&mut Context::from_waker(&caller)),
        Poll::Pending
    ));
    release.send(73)?;
    caller_notifications.recv_timeout(Duration::from_secs(5))?;
    notifications.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(block_on(task)?, 73);
    Ok(())
}

#[test]
fn cancellation_keeps_completed_output_but_never_replays_pending_work() -> Result {
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let (wakeup, _) = host();
    let completed = pool.spawn(async { "completed" });
    finished(&completed)?;
    let completed = wakeup.track(completed);
    assert_eq!(block_on(completed.cancel()), Some("completed"));
    let pending = wakeup.spawn(&pool, bevy::tasks::futures_lite::future::pending::<()>());
    assert_eq!(block_on(pending.cancel()), None);
    Ok(())
}
