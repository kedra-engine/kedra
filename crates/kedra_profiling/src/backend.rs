//! Perfetto owns recording/encoding. This worker owns only finite lifecycle and publication.
pub(crate) mod bevy_layer;
pub(crate) mod markers;

perfetto_sdk::track_event_categories! {
    pub(crate) mod perfetto_te_ns {
        ("Application", "Application-defined work", []),
        ("Runtime", "Engine runtime", []),
        ("Scheduling", "Scheduler and executor details", []),
        ("UI", "Retained UI", []),
        ("Rendering CPU", "CPU render work, not GPU timing", []),
        ("Assets", "Asset work", []),
        ("Scripts", "Script work and logical operations", []),
        ("Editor", "Editor tools", []),
        ("Metadata", "Recording identity and frame correlation", []),
    }
}

pub(super) fn initialize() {
    use perfetto_sdk::{
        producer::{Backends, Producer, ProducerInitArgsBuilder},
        track_event::TrackEvent,
    };
    Producer::init(
        ProducerInitArgsBuilder::new()
            .backends(Backends::IN_PROCESS)
            .build(),
    );
    TrackEvent::init();
    // Static category registration is idempotent at process initialization.
    if let Err(error) = perfetto_te_ns::register() {
        let _ = INITIALIZATION_ERROR.set(error.to_string());
    }
}
use crate::{CaptureConfig, CaptureState, Error, Recording, Target};
use perfetto_sdk::{
    heap_buffer::HeapBuffer,
    pb_msg::{PbMsg, PbMsgWriter},
    protos::config::{
        data_source_config::DataSourceConfig,
        trace_config::{TraceConfig, TraceConfigBufferConfig, TraceConfigDataSource},
        track_event::track_event_config::TrackEventConfig,
    },
    tracing_session::TracingSession,
};
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use uuid::Uuid;

pub(super) static INITIALIZED: std::sync::Once = std::sync::Once::new();
static INITIALIZATION_ERROR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static PROCESS_RECORDING: Mutex<()> = Mutex::new(());

struct ActiveGeneration;
impl Drop for ActiveGeneration {
    fn drop(&mut self) {
        if let Ok(mut generation) = markers::GENERATION.write() {
            *generation = 0;
        }
    }
}

enum Command {
    Stop,
    Cancel,
}
pub(super) struct Worker {
    commands: mpsc::SyncSender<Command>,
    events: Mutex<mpsc::Receiver<CaptureState>>,
    publication: Arc<AtomicU8>,
}

impl Worker {
    pub(super) fn spawn(
        id: Uuid,
        config: CaptureConfig,
        target: Target,
        directory: PathBuf,
        label: Option<String>,
    ) -> Result<Self, Error> {
        if !INITIALIZED.is_completed() {
            return Err(Error::Worker(
                "Install the profiling layer through Bevy LogPlugin before recording".into(),
            ));
        }
        if let Some(error) = INITIALIZATION_ERROR.get() {
            return Err(Error::Worker(error.clone()));
        }
        let (commands, receiver) = mpsc::sync_channel(2);
        let (events, results) = mpsc::sync_channel(2);
        let publication = Arc::new(AtomicU8::new(0));
        let permit = publication.clone();
        thread::Builder::new()
            .name("kedra-profile-recording".into())
            .spawn(move || {
                let result = record(
                    id,
                    config,
                    (target, label),
                    directory,
                    receiver,
                    &events,
                    &permit,
                );
                let state = match result {
                    Ok(Some((recording, directory))) => CaptureState::Completed {
                        recording,
                        directory,
                    },
                    Ok(None) => CaptureState::Cancelled { id },
                    Err(error) => CaptureState::Failed { id, error },
                };
                let _ = events.send(state);
            })
            .map_err(|error| Error::Worker(error.to_string()))?;
        Ok(Self {
            commands,
            events: Mutex::new(results),
            publication,
        })
    }

    pub(super) fn stop(&self) -> Result<(), Error> {
        self.commands
            .try_send(Command::Stop)
            .map_err(|error| Error::Worker(error.to_string()))
    }
    pub(super) fn cancel(&self) -> Result<(), Error> {
        match self
            .publication
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(1) => {
                // The atomic permit also covers cancellation during readback/publication.
                let _ = self.commands.try_send(Command::Cancel);
                Ok(())
            }
            Err(_) => Err(Error::Inactive),
        }
    }
    pub(super) fn poll(&self, current: &CaptureState) -> Option<CaptureState> {
        let Ok(receiver) = self.events.lock() else {
            return Some(CaptureState::Failed {
                id: current.id()?,
                error: "Profiling result mailbox was poisoned".into(),
            });
        };
        let mut latest = None;
        loop {
            match receiver.try_recv() {
                Ok(CaptureState::Recording { .. })
                    if matches!(current, CaptureState::Stopping { .. }) => {}
                Ok(state) => {
                    latest = Some(state);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if latest.is_none() && current.active() {
                        latest = Some(CaptureState::Failed {
                            id: current.id()?,
                            error: "Profiling worker exited without a terminal result".into(),
                        });
                    }
                    break;
                }
            }
        }
        latest
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Dropping the sender disconnects an idle wait even if the bounded queue
        // was full. SDK destruction remains on its owner thread, never the UI.
        let _ = self.cancel();
    }
}

fn configuration(config: CaptureConfig) -> Result<Vec<u8>, String> {
    let writer = PbMsgWriter::new();
    let heap = HeapBuffer::new(writer.stream_writer());
    let mut message = PbMsg::new(&writer).map_err(|error| error.to_string())?;
    let mut trace = TraceConfig { msg: &mut message };
    trace.set_buffers(|buffer: &mut TraceConfigBufferConfig| {
        buffer.set_size_kb(config.buffer_kib);
    });
    trace.set_duration_ms(config.duration_ms);
    trace.set_incremental_state_config(|state: &mut perfetto_sdk::protos::config::trace_config::TraceConfigIncrementalStateConfig| {
        state.set_clear_period_ms(500);
    });
    trace.set_data_sources(|source: &mut TraceConfigDataSource| {
        source.set_config(|config: &mut DataSourceConfig| {
            config.set_name("track_event");
            config.set_track_event_config(|events: &mut TrackEventConfig| {
                events.set_disabled_categories("*");
                for category in [
                    "Application",
                    "Runtime",
                    "Scheduling",
                    "UI",
                    "Rendering CPU",
                    "Assets",
                    "Scripts",
                    "Editor",
                    "Metadata",
                ] {
                    events.set_enabled_categories(category);
                }
            });
        });
    });
    message.finalize();
    let mut bytes = vec![0; writer.stream_writer().get_written_size()];
    heap.copy_into(&mut bytes);
    Ok(bytes)
}

fn record(
    id: Uuid,
    config: CaptureConfig,
    context: (Target, Option<String>),
    directory: PathBuf,
    commands: mpsc::Receiver<Command>,
    events: &mpsc::SyncSender<CaptureState>,
    publication: &AtomicU8,
) -> Result<Option<(Recording, PathBuf)>, String> {
    let (target, label) = context;
    let _process_owner = PROCESS_RECORDING
        .try_lock()
        .map_err(|_| "Another profiling owner is recording this process")?;
    // Reserve a private, unique directory before starting. Failure cannot erase
    // or truncate an existing recording, even when the caller reuses the root.
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let owned = tempfile::Builder::new()
        .prefix(&format!("capture-{id}-"))
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    let mut session = TracingSession::in_process().map_err(|error| error.to_string())?;
    session.setup(&configuration(config)?);
    let dropped_before = bevy_layer::DROPPED.load(Ordering::Relaxed);
    session.start_blocking();
    *markers::GENERATION
        .write()
        .map_err(|_| "Profiling generation lock was poisoned")? =
        markers::NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    let generation = ActiveGeneration;
    markers::recording_metadata(id, &target, label.as_deref());
    let _ = events.send(CaptureState::Recording { id });
    let stopped = match commands.recv_timeout(Duration::from_millis(u64::from(config.duration_ms)))
    {
        Ok(Command::Stop) => true,
        Ok(Command::Cancel) | Err(mpsc::RecvTimeoutError::Disconnected) => {
            session.stop_blocking();
            return Ok(None);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => false,
    };
    drop(generation);
    session.stop_blocking();
    // A cancellation queued behind Stop still revokes publication.
    if matches!(
        commands.try_recv(),
        Ok(Command::Cancel) | Err(mpsc::TryRecvError::Disconnected)
    ) {
        return Ok(None);
    }
    let file = tempfile::NamedTempFile::new_in(owned.path()).map_err(|error| error.to_string())?;
    let output = Arc::new(Mutex::new((file, 0u64, None::<String>)));
    let callback = output.clone();
    // Trace buffers are bounded by the SDK; additionally bound persisted bytes
    // rather than trusting a backend or callback to respect our admission config.
    let limit = u64::from(config.buffer_kib) * 2048 + 8 * 1024 * 1024;
    session.read_trace_blocking(move |bytes, _| {
        // Never panic through the native callback: the SDK aborts on unwinding.
        if let Ok(mut output) = callback.lock() {
            if output.2.is_some() {
                return;
            }
            if output.1.saturating_add(bytes.len() as u64) > limit {
                output.2 = Some("Perfetto output exceeded the recording byte limit".into());
                return;
            }
            match output.0.write_all(bytes) {
                Ok(()) => output.1 += bytes.len() as u64,
                Err(error) => output.2 = Some(error.to_string()),
            }
        }
    });
    let output = Arc::try_unwrap(output)
        .map_err(|_| "Perfetto retained a readback callback")?
        .into_inner()
        .map_err(|_| "Profiling output lock was poisoned")?;
    let (mut file, trace_bytes, error) = output;
    if let Some(error) = error {
        return Err(error);
    }
    if trace_bytes == 0 {
        return Err("Perfetto produced an empty trace".into());
    }
    file.flush().map_err(|error| error.to_string())?;
    file.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    file.persist_noclobber(owned.path().join("trace.pftrace"))
        .map_err(|error| error.to_string())?;
    let recording = Recording {
        schema: 1,
        id,
        target,
        process: std::process::id(),
        config,
        backend: "Perfetto SDK 1.2.1 / Kedra Bevy scope adapter".into(),
        stopped,
        trace_bytes,
        label,
        adapter_dropped_events: bevy_layer::DROPPED
            .load(Ordering::Relaxed)
            .saturating_sub(dropped_before),
    };
    let mut metadata =
        tempfile::NamedTempFile::new_in(owned.path()).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut metadata, &recording).map_err(|error| error.to_string())?;
    metadata.flush().map_err(|error| error.to_string())?;
    metadata
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    metadata
        .persist_noclobber(owned.path().join("recording.json"))
        .map_err(|error| error.to_string())?;
    if publication
        .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(None);
    }
    Ok(Some((recording, owned.keep())))
}
