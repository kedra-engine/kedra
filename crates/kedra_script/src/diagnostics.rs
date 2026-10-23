//! Structured execution failures and bounded, owner-attributed script diagnostics.
use crate::{EntityHandle, Program, Run, lua, values};
use bevy::prelude::Resource;
use parking_lot::Mutex;
use serde_json::Value;
use std::{collections::VecDeque, fmt, sync::Arc};

/// Stable failure category; callers need not parse display text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Invalid source, parameters, arguments or operation shape.
    InvalidInput,
    /// A bounded execution, child or subscription pool is full.
    Capacity,
    /// Entry compilation/evaluation or binding initialization failed.
    Load,
    /// A running script or native binding failed.
    Runtime,
    /// A memory, time, resume or event-queue budget was exhausted.
    Budget,
    /// The current program is not authorized for this operation or target.
    Permission,
    /// A run, target or owner no longer exists in the expected World.
    Stale,
}
impl ErrorKind {
    /// Stable lowercase code used by script-side outcomes and host diagnostics.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::Capacity => "capacity",
            Self::Load => "load",
            Self::Runtime => "runtime",
            Self::Budget => "budget",
            Self::Permission => "permission",
            Self::Stale => "stale",
        }
    }
}

/// Owned, bounded failure evidence. The message retains available Lua traceback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    /// Machine-readable failure class.
    pub kind: ErrorKind,
    /// Human-readable message, at most 8 KiB, truncated on a UTF-8 boundary.
    pub message: String,
    /// Diagnostic program name, if known.
    pub program: Option<String>,
    /// Project-relative entry source, if known.
    pub path: Option<String>,
    /// Loading or callback phase, if known.
    pub phase: Option<String>,
}
impl ScriptError {
    /// Construct a categorized error without assuming an execution context.
    pub fn new(kind: ErrorKind, message: impl ToString) -> Self {
        Self {
            kind,
            message: bounded(message.to_string()),
            program: None,
            path: None,
            phase: None,
        }
    }
    pub(crate) fn at(
        program: &Program,
        kind: ErrorKind,
        phase: &str,
        message: impl ToString,
    ) -> Self {
        Self {
            program: Some(program.name.clone()),
            path: Some(program.path.clone()),
            phase: Some(phase.into()),
            ..Self::new(kind, message)
        }
    }
}
impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{path}: ")?;
        }
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for ScriptError {}

/// Diagnostic severity, independent of failure category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Ordinary script observation.
    Info,
    /// Recoverable script warning.
    Warn,
    /// Explicit log error or terminal execution failure.
    Error,
}

/// One script log/failure, attributed to its native owner and source.
#[derive(Clone, Debug)]
pub struct Record {
    /// Process-local execution identity.
    pub run: Run,
    /// Attached object; never persistent source identity.
    pub target: Option<EntityHandle>,
    /// Diagnostic program name.
    pub program: String,
    /// Project-relative entry path.
    pub path: String,
    /// Actual Lua caller source, when available.
    pub source: Option<String>,
    /// Actual Lua caller line, when available.
    pub line: Option<usize>,
    /// Loading or callback phase.
    pub phase: String,
    /// Record severity.
    pub level: Level,
    /// Stable class for execution failures; None for explicit log calls.
    pub error_kind: Option<ErrorKind>,
    /// Bounded message or traceback.
    pub message: String,
    /// Bounded owned JSON fields; no borrowed ECS or Lua objects.
    pub fields: Value,
}

#[derive(Default)]
struct Buffer {
    entries: VecDeque<Record>,
    dropped: u64,
}

/// Shared bounded diagnostic sink. Hosts can drain records for their console/UI.
/// Retains the newest 256 records and counts overwritten records; writes also emit
/// source-attributed stderr diagnostics. No editor dependency or mandatory UI.
#[derive(Resource, Default, Clone)]
pub struct Diagnostics(Arc<Mutex<Buffer>>);
impl Diagnostics {
    /// Consume retained records in insertion order. Does not reset dropped count.
    pub fn drain(&self) -> Vec<Record> {
        self.0.lock().entries.drain(..).collect()
    }
    /// Cumulative number of overwritten records, saturating at u64::MAX.
    pub fn dropped(&self) -> u64 {
        self.0.lock().dropped
    }
    pub(crate) fn push(&self, record: Record) {
        eprintln!(
            "[script {:?}] {} [{}]: {}",
            record.level, record.path, record.phase, record.message
        );
        let mut buffer = self.0.lock();
        if buffer.entries.len() == 256 {
            buffer.entries.pop_front();
            buffer.dropped = buffer.dropped.saturating_add(1);
        }
        buffer.entries.push_back(record);
    }
}

pub(crate) struct Phase(pub &'static str);

pub(crate) fn install(
    lua: &lua::Lua,
    ctx: &lua::Table,
    sink: Diagnostics,
    template: Record,
) -> lua::Result<()> {
    let log = lua.create_table()?;
    for (name, level) in [
        ("info", Level::Info),
        ("warn", Level::Warn),
        ("error", Level::Error),
    ] {
        let sink = sink.clone();
        let template = template.clone();
        log.set(
            name,
            lua.create_function(
                move |lua, (message, fields): (String, Option<lua::Value>)| {
                    if message.len() > 8192 {
                        return Err(fault(ErrorKind::InvalidInput, "Log message exceeds 8 KiB"));
                    }
                    let fields = fields
                        .map(|value| values::from_lua(lua, value))
                        .transpose()?
                        .unwrap_or(Value::Null);
                    let phase = lua
                        .app_data_ref::<Phase>()
                        .map_or("unknown", |phase| phase.0);
                    let location = lua.inspect_stack(1, |frame| {
                        (
                            frame.source().source.as_deref().map(str::to_owned),
                            frame.current_line(),
                        )
                    });
                    let (source, line) = location.unwrap_or((None, None));
                    sink.push(Record {
                        level,
                        message,
                        fields,
                        phase: phase.into(),
                        source,
                        line,
                        ..template.clone()
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    log.set_readonly(true);
    ctx.set("log", log)
}

pub(crate) fn fault(kind: ErrorKind, message: impl ToString) -> lua::Error {
    lua::Error::external(ScriptError::new(kind, message))
}

pub(crate) fn classify(error: &lua::Error) -> ErrorKind {
    match error {
        lua::Error::MemoryError(_) => ErrorKind::Budget,
        lua::Error::CallbackError { cause, .. } | lua::Error::BadArgument { cause, .. } => {
            classify(cause)
        }
        lua::Error::ExternalError(error) => error
            .downcast_ref::<ScriptError>()
            .map_or(ErrorKind::Runtime, |error| error.kind),
        _ => ErrorKind::Runtime,
    }
}

fn bounded(mut value: String) -> String {
    if value.len() > 8192 {
        let mut end = 8192;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
    }
    value
}
