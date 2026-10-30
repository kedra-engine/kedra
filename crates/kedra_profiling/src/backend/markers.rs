//! The only custom instrumentation adapter: facade semantics to standard backend events.
use super::perfetto_te_ns;
use crate::{Category, CounterValue, Error, Unit};
use perfetto_sdk::track_event::{EventContext, TrackEventCounter, TrackEventTrack, TrackEventType};
use std::{
    ffi::CString,
    sync::{
        RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

pub(super) static GENERATION: RwLock<u64> = RwLock::new(0);
pub(super) static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

pub(crate) struct Scope {
    generation: u64,
    category: Category,
}
pub(crate) fn enter_in(name: &str, category: Category) -> Scope {
    let Ok(current) = GENERATION.read() else {
        return Scope {
            generation: 0,
            category,
        };
    };
    if *current == 0 || !active() {
        return Scope {
            generation: 0,
            category,
        };
    }
    let Ok(label) = CString::new(name) else {
        return Scope {
            generation: 0,
            category,
        };
    };
    event(category, TrackEventType::SliceBegin(label.as_ptr()), |_| {});
    Scope {
        generation: *current,
        category,
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let Ok(current) = GENERATION.read() else {
            return;
        };
        if self.generation == 0 || *current != self.generation || !active() {
            return;
        }
        event(self.category, TrackEventType::SliceEnd, |_| {});
    }
}

fn active() -> bool {
    super::INITIALIZED.is_completed() && perfetto_sdk::track_event_category_enabled!("Metadata")
}

pub(super) fn recording_metadata(id: uuid::Uuid, target: &crate::Target, label: Option<&str>) {
    use perfetto_sdk::track_event::TrackEventDebugArg::String as Arg;
    let id = id.to_string();
    let instance = target.instance.to_string();
    event(
        Category::Metadata,
        TrackEventType::Instant(c"kedra.recording".as_ptr()),
        |context| {
            context.add_debug_arg("recording", Arg(&id));
            context.add_debug_arg("application", Arg(&target.name));
            context.add_debug_arg("instance", Arg(&instance));
            context.add_debug_arg("build", Arg(&target.build));
            if let Some(label) = label {
                context.add_debug_arg("label", Arg(label));
            }
        },
    );
}

pub(crate) fn frame(name: &str, update: u64) {
    if !active() {
        return;
    }
    let Ok(name) = CString::new(name) else {
        return;
    };
    event(
        Category::Metadata,
        TrackEventType::Instant(name.as_ptr()),
        |context| {
            context.add_debug_arg(
                "update",
                perfetto_sdk::track_event::TrackEventDebugArg::Uint64(update),
            );
        },
    );
}

pub(super) fn event(
    category: Category,
    kind: TrackEventType,
    fields: impl FnOnce(&mut EventContext),
) {
    macro_rules! emit {
        ($name:literal) => {
            perfetto_sdk::track_event!($name, kind, fields)
        };
    }
    match category {
        Category::Application => emit!("Application"),
        Category::Runtime => emit!("Runtime"),
        Category::Scheduling => emit!("Scheduling"),
        Category::Ui => emit!("UI"),
        Category::Rendering => emit!("Rendering CPU"),
        Category::Assets => emit!("Assets"),
        Category::Scripting => emit!("Scripts"),
        Category::Editor => emit!("Editor"),
        Category::Metadata => emit!("Metadata"),
    }
}

pub(crate) fn counter(name: &str, unit: Unit, value: CounterValue, category: Category) {
    if !active() {
        return;
    }
    use perfetto_sdk::protos::trace::track_event::{
        counter_descriptor::CounterDescriptorFieldNumber as Counter,
        track_descriptor::TrackDescriptorFieldNumber as Track,
    };
    use perfetto_sdk::track_event::{TrackEventProtoField as Field, TrackEventProtoTrack};
    let label = format!("{name} [{}]", unit.name());
    let parent = TrackEventTrack::process_track_uuid();
    let uuid = TrackEventTrack::counter_track_uuid(&label, parent);
    event(
        category,
        TrackEventType::Counter,
        |context: &mut EventContext| {
            context.set_proto_track(&TrackEventProtoTrack {
                uuid,
                fields: &[
                    Field::VarInt(Track::Uuid as u32, uuid),
                    Field::VarInt(Track::ParentUuid as u32, parent),
                    Field::Cstr(Track::Name as u32, &label),
                    Field::Nested(
                        Track::Counter as u32,
                        &[
                            Field::Cstr(Counter::UnitName as u32, unit.name()),
                            Field::Cstr(Counter::Categories as u32, category.name()),
                            Field::VarInt(Counter::IsIncremental as u32, 0),
                        ],
                    ),
                ],
            });
            context.set_counter(match value {
                CounterValue::Integer(value) => TrackEventCounter::Int64(value),
                CounterValue::Float(value) => TrackEventCounter::Double(value),
            });
        },
    );
}

pub(crate) struct Operation {
    track: TrackEventTrack,
    generation: u64,
    finished: bool,
    category: Category,
}
pub(crate) fn operation(name: &str, category: Category) -> Result<Option<Operation>, Error> {
    if !active() {
        return Ok(None);
    }
    let current = GENERATION
        .read()
        .map_err(|_| Error::Worker("Profiling generation lock was poisoned".into()))?;
    let generation = *current;
    if generation == 0 {
        return Ok(None);
    }
    static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_OPERATION.fetch_add(1, Ordering::Relaxed);
    let track = TrackEventTrack::register_named_track_with_dynamic_name(
        &format!("Operation: {name}"),
        id,
        TrackEventTrack::process_track_uuid(),
    )
    .map_err(|error| Error::Worker(error.to_string()))?;
    let label = CString::new(name).map_err(|error| Error::Invalid(error.to_string()))?;
    event(
        category,
        TrackEventType::SliceBegin(label.as_ptr()),
        |context: &mut EventContext| {
            context.set_track(&track);
        },
    );
    Ok(Some(Operation {
        track,
        generation,
        finished: false,
        category,
    }))
}
impl Operation {
    pub(crate) fn finish(mut self) {
        self.finished = true;
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        let Ok(current) = GENERATION.read() else {
            return;
        };
        if !active() || *current != self.generation {
            return;
        }
        event(
            self.category,
            TrackEventType::SliceEnd,
            |context: &mut EventContext| {
                context.set_track(&self.track);
                context.add_debug_arg(
                    "outcome",
                    perfetto_sdk::track_event::TrackEventDebugArg::String(if self.finished {
                        "completed"
                    } else {
                        "cancelled"
                    }),
                );
            },
        );
    }
}
