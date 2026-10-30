//! Backend-neutral instrumentation contracts. No backend types escape this module.
use crate::{Category, Error};

/// A reusable CPU-scope definition. Names are static diagnostic labels, not data.
/// Instrumented scope time is elapsed time on the executing thread, including waits.
/// Do not retain a guard across asynchronous suspension; use [`Operation`] instead.
pub struct Marker {
    name: std::borrow::Cow<'static, str>,
    category: Category,
}

impl Marker {
    /// Define an allocation-free static marker. Do not include private user values.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name: std::borrow::Cow::Borrowed(name),
            category: Category::Application,
        }
    }

    /// Define a bounded dynamic label for an admitted script/tool instance.
    /// Reuse definitions and bound their cardinality in dynamically supplied code.
    pub fn named(name: impl Into<String>) -> Result<Self, Error> {
        let name = name.into();
        validate_name(&name)?;
        Ok(Self {
            name: std::borrow::Cow::Owned(name),
            category: Category::Application,
        })
    }

    /// Enter on the current thread and exit when the returned guard drops.
    /// Disabled builds perform no recording or allocation. Backend types are private.
    pub fn enter(&self) -> MarkerGuard {
        MarkerGuard {
            _thread: std::marker::PhantomData,
            #[cfg(feature = "capture")]
            _inner: crate::backend::markers::enter_in(&self.name, self.category),
        }
    }

    /// Stable diagnostic label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Assign a semantic category without exposing backend types or a registry.
    pub const fn with_category(mut self, category: Category) -> Self {
        self.category = category;
        self
    }
    /// Semantic category, independent of backend identifiers.
    pub const fn category(&self) -> Category {
        self.category
    }
}

/// Thread-bound CPU scope. Never hold this across a coroutine yield or task await.
/// Drop ends the scope, including ordinary Rust error/panic unwinding.
#[must_use = "Dropping the guard ends the CPU scope"]
pub struct MarkerGuard {
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
    #[cfg(feature = "capture")]
    _inner: crate::backend::markers::Scope,
}

/// Explicit unit for absolute counter samples; units never change after construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// Number of objects/events; not a byte count or duration.
    Count,
    /// Bytes, not KiB or resident-memory attribution.
    Bytes,
    /// Milliseconds.
    Milliseconds,
    /// Percentage points, not a normalized 0..1 ratio.
    Percent,
}
impl Unit {
    /// Canonical unit label, shared with scoped Luau instrumentation.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Bytes => "bytes",
            Self::Milliseconds => "ms",
            Self::Percent => "percent",
        }
    }
}

/// Integer samples retain signed 64-bit precision; floating samples must be finite.
/// These are absolute gauge values, never implicit increments or rates.
#[derive(Clone, Copy, Debug)]
pub enum CounterValue {
    /// Exact signed integer.
    Integer(i64),
    /// Finite floating-point sample.
    Float(f64),
}
impl From<i64> for CounterValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}
impl From<u32> for CounterValue {
    fn from(value: u32) -> Self {
        Self::Integer(i64::from(value))
    }
}
impl From<i32> for CounterValue {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}
impl From<f64> for CounterValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

/// A named absolute counter, qualified by name and unit within the current process.
/// Use application/subsystem-qualified names when applications share a process.
/// Own/reuse this handle rather than constructing one for every sample.
pub struct Counter {
    name: String,
    unit: Unit,
    category: Category,
}
impl Counter {
    /// Define a bounded Unicode diagnostic name. Rejects empty/control/NUL names
    /// and names larger than 256 UTF-8 bytes without backend/file effects.
    pub fn new(name: impl Into<String>, unit: Unit) -> Result<Self, Error> {
        let name = name.into();
        validate_name(&name)?;
        Ok(Self {
            name,
            unit,
            category: Category::Application,
        })
    }

    /// Record one absolute sample. Invalid numbers have no side effects, including
    /// in disabled builds. Does not retain a separate application-side history.
    pub fn sample(&self, value: impl Into<CounterValue>) -> Result<(), Error> {
        let value = value.into();
        if matches!(value, CounterValue::Float(number) if !number.is_finite()) {
            return Err(Error::Invalid(
                "Profiling counters require finite values".into(),
            ));
        }
        #[cfg(feature = "capture")]
        crate::backend::markers::counter(&self.name, self.unit, value, self.category);
        Ok(())
    }

    /// Stable diagnostic name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Immutable sample unit.
    pub const fn unit(&self) -> Unit {
        self.unit
    }
    /// Set a semantic category before sharing/sampling this handle.
    pub const fn with_category(mut self, category: Category) -> Self {
        self.category = category;
        self
    }
    /// Semantic category, independent of backend identifiers.
    pub const fn category(&self) -> Category {
        self.category
    }
}

/// Logical operation duration on its own track, not CPU execution time.
/// May cross worker threads and script yields. Each in-flight operation has its own
/// identity, so overlapping/non-LIFO operations cannot corrupt a thread scope stack.
/// [`Operation::finish`] records completion; Drop without Finish records cancellation.
/// Operations crossing recording boundaries remain incomplete in the old recording;
/// they never emit a stray end into a later session.
#[must_use = "Retain the operation until completion or cancellation"]
pub struct Operation {
    #[cfg(feature = "capture")]
    inner: Option<crate::backend::markers::Operation>,
}
impl Operation {
    /// Begin a bounded named operation. An inactive/disabled recorder is a no-op,
    /// not authority to start recording. Never pass sensitive payloads as names.
    pub fn begin(name: &str) -> Result<Self, Error> {
        Self::begin_in(name, Category::Application)
    }

    /// Begin a logical operation in an explicit semantic category.
    pub fn begin_in(name: &str, category: Category) -> Result<Self, Error> {
        validate_name(name)?;
        #[cfg(not(feature = "capture"))]
        let _ = category;
        Ok(Self {
            #[cfg(feature = "capture")]
            inner: crate::backend::markers::operation(name, category)?,
        })
    }
    /// Finish a live logical operation. Consumes ownership exactly once.
    pub fn finish(mut self) {
        #[cfg(feature = "capture")]
        if let Some(inner) = self.inner.take() {
            inner.finish();
        }
        #[cfg(not(feature = "capture"))]
        let _ = &mut self;
    }
}

fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(Error::Invalid(
            "Profiling names require 1..=256 UTF-8 bytes without control characters".into(),
        ));
    }
    Ok(())
}
