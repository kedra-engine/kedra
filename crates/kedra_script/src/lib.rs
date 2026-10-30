//! Shared, explicitly authorized Luau execution over ordinary Bevy applications.
//!
//! Hosts install narrow bindings; runtime owns independent VMs, suspension,
//! deadlines and revocation. No project scanning, generic World binding, remote
//! eval, authoring writer or editor dependency is installed. See the crate README.
mod diagnostics;
pub mod events;
mod execution;
mod jobs;
mod modules;
mod parameters;
mod profiling;
pub mod values;
mod vm;
pub use diagnostics::{Diagnostics, ErrorKind, Level, Record, ScriptError};
pub use modules::Sources;
pub use parameters::ParameterValidator;

/// The version-aligned embedding API for trusted Rust host binding functions.
pub use bevy_mod_scripting::lua::mlua as lua;

use bevy::{ecs::world::WorldId, prelude::*};
use bevy_mod_scripting::{
    bindings::{ThreadWorldContainer, WorldExtensions},
    core::{BMSScriptingInfrastructurePlugin, ScriptingSystemSet},
    prelude::{LuaScriptingPlugin, ScriptAsset},
};
use parking_lot::Mutex;
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Trusted host API registration. It must not execute host effects while loading.
/// Add narrowly scoped tables to the supplied `ctx`; do not publish host globals.
/// Capture Scope, not World/component borrows or strong Lua handles in Lua callbacks.
pub type Bindings = fn(&lua::Lua, Scope, &lua::Table) -> lua::Result<()>;

/// Explicit entrypoint contract; no source-name or callback-shape guessing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One finite, yielding `run(ctx, args)` invocation.
    Sequence,
    /// Persistent non-yielding `start/update/fixed_update/stop` callbacks.
    Behavior,
}

/// Execution budgets; host/native callbacks must separately bound their own work.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Per-VM Lua allocator limit, not a native-memory or process sandbox.
    pub memory_bytes: usize,
    /// Cooperative VM deadline for each load/resume, not an exact instruction count.
    pub callback_time: Duration,
    /// Whole-run real-time deadline. Required for sequences; None permits persistent
    /// behaviors. Checked before dispatch, including updates while a sequence waits.
    pub total_time: Option<Duration>,
    /// Maximum dispatches, including zero-work resumes. Required for sequences;
    /// None permits persistent behaviors with bounded per-callback work.
    pub resumes: Option<u64>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 16 * 1024 * 1024,
            callback_time: Duration::from_millis(100),
            total_time: Some(Duration::from_secs(45)),
            resumes: Some(100_000),
        }
    }
}

/// One host-authorized program. Registration is not execution or project permission.
#[derive(Clone)]
pub struct Program {
    /// Diagnostic identity, at most 128 UTF-8 bytes, without control characters.
    pub name: String,
    /// Owned Luau source, at most 64 KiB. No file/module lookup is implicit.
    pub source: String,
    /// Canonical project-relative entry file, also used by LSP import resolution.
    pub path: String,
    /// Explicit sequence or behavior entrypoint contract.
    pub kind: Kind,
    /// Admitted module/config snapshot. The entry file is supplied separately.
    pub sources: Sources,
    /// Host-selected capability initializer, different for game/editor/test contexts.
    pub bindings: Bindings,
    /// Explicit execution limits.
    pub limits: Limits,
    /// Immutable per-instance parameters, validated before allocation. Must be an object.
    pub params: Value,
    /// Host-selected typed/domain validator. The default accepts only an empty object.
    pub validate_params: ParameterValidator,
    /// Host-approved finite children. Only behaviors can start these entries; child
    /// programs cannot themselves declare children. No filesystem discovery occurs.
    pub sequences: Vec<Program>,
    /// Subsystem event names this behavior may subscribe to on its own target.
    /// The subsystem still owns producing native notifications.
    pub events: &'static [&'static str],
}
impl Program {
    /// Construct a program with ordinary bounded defaults. Source is not executed.
    pub fn sequence(
        name: impl Into<String>,
        path: impl Into<String>,
        source: impl Into<String>,
        bindings: Bindings,
    ) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            path: path.into(),
            kind: Kind::Sequence,
            sources: Sources::default(),
            bindings,
            limits: Limits::default(),
            params: serde_json::json!({}),
            validate_params: parameters::empty,
            sequences: Vec::new(),
            events: &[],
        }
    }

    /// Construct a persistent behavior. Lifetime is owner-controlled; memory and
    /// per-callback budgets remain bounded, without a finite sequence deadline.
    pub fn behavior(
        name: impl Into<String>,
        path: impl Into<String>,
        source: impl Into<String>,
        bindings: Bindings,
    ) -> Self {
        let mut program = Self::sequence(name, path, source, bindings);
        program.kind = Kind::Behavior;
        program.limits.total_time = None;
        program.limits.resumes = None;
        program
    }
}

/// Host-owned admission policy, independent from individual VM budgets.
/// Insert before LuauPlugin to override the default capacities.
#[derive(Resource)]
pub struct Capacity {
    /// Concurrent finite sequences (default 32).
    pub sequences: usize,
    /// Concurrent persistent behaviors (default 256, not a performance guarantee).
    pub behaviors: usize,
}
impl Default for Capacity {
    fn default() -> Self {
        Self {
            sequences: 32,
            behaviors: 256,
        }
    }
}

/// Opaque runtime entity reference exposed as `ctx.entity` for attached programs.
/// It never keeps an entity alive or serves as persistent source identity.
#[derive(Clone, Copy, Debug)]
pub struct EntityHandle {
    world: WorldId,
    entity: Entity,
}
impl EntityHandle {
    /// Resolve against the current authorized World. Foreign/stale handles fail;
    /// host bindings must still use Scope and enforce their own domain permissions.
    pub fn resolve(&self, world: &World) -> lua::Result<Entity> {
        if world.id() != self.world || world.get_entity(self.entity).is_err() {
            return Err(diagnostics::fault(
                ErrorKind::Stale,
                "Stale or foreign entity handle",
            ));
        }
        Ok(self.entity)
    }
}
impl lua::UserData for EntityHandle {}

/// Terminal outcomes never imply input acceptance, GPU readiness or rollback.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// The program is eligible for later sequence/behavior dispatch.
    Running,
    /// The entry function returned bounded JSON-compatible owned data.
    Completed(Value),
    /// The host revoked execution; already applied effects remain applied.
    Cancelled,
    /// Loading, a binding, a budget or an entrypoint failed permanently.
    Failed(ScriptError),
}

/// Process-local, generational run identity. It is not persistent project identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    world: WorldId,
    owner: Entity,
}

/// Rust-only capability to reacquire live ECS data for a synchronous native call.
#[derive(Clone)]
pub struct Scope {
    run: Run,
    asset: Handle<ScriptAsset>,
}
impl Scope {
    /// Access one live World only during this run's resume. Returns owned data.
    ///
    /// Rejects wrong worlds, revoked/despawned owners and source-evaluation calls.
    /// No borrow can escape this closure or cross a coroutine yield. Mutations
    /// happen synchronously; this is not an authoring transaction or rollback API.
    pub fn with_world<T>(&self, f: impl FnOnce(&mut World) -> lua::Result<T>) -> lua::Result<T> {
        let guard = ThreadWorldContainer
            .try_get_context()
            .map_err(vm::world_error)?
            .world;
        let current = guard.current_attachment();
        if current.0.as_ref().is_none_or(|attachment| {
            attachment.entity() != Some(self.run.owner) || attachment.script() != self.asset
        }) {
            return Err(diagnostics::fault(
                ErrorKind::Permission,
                "Script scope belongs to another execution context",
            ));
        }
        guard
            .with_world_mut_access(|world| {
                let allowed = world.id() == self.run.world
                    && world
                        .get::<Instance>(self.run.owner)
                        .is_some_and(|instance| {
                            instance.active
                                && instance.asset == self.asset
                                && instance.status == Status::Running
                        });
                if !allowed {
                    return Err(diagnostics::fault(
                        ErrorKind::Stale,
                        "Script scope is not active",
                    ));
                }
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(world))).unwrap_or_else(
                    |_| {
                        Err(lua::Error::runtime(
                            "Native script binding panicked; prior effects are not rolled back",
                        ))
                    },
                )
            })
            .map_err(vm::world_error)?
    }

    /// Resolve only this run's attached object. Invoke within with_world; foreign,
    /// stale and unowned targets fail before a subsystem operation can mutate data.
    pub fn resolve_entity(&self, world: &World, handle: &EntityHandle) -> lua::Result<Entity> {
        let entity = handle.resolve(world)?;
        if world.id() != self.run.world
            || world
                .get::<Instance>(self.run.owner)
                .is_none_or(|instance| instance.target != Some(entity))
        {
            return Err(diagnostics::fault(
                ErrorKind::Permission,
                "Entity is outside this script's owned target",
            ));
        }
        Ok(entity)
    }
}

#[derive(Component)]
#[component(on_remove = execution::removed)]
struct Instance {
    program: Program,
    asset: Handle<ScriptAsset>,
    arguments: Value,
    execution: Arc<Mutex<vm::Execution>>,
    target: Option<Entity>,
    status: Status,
    active: bool,
    started: Instant,
    resumes: u64,
    released: bool,
    jobs: Vec<Arc<Mutex<jobs::Job>>>,
    events: Arc<Mutex<events::Mailbox>>,
    error_kind: Arc<Mutex<Option<ErrorKind>>>,
    profiling: profiling::State,
}

/// Host ordering points in ordinary Bevy schedules; no order between instances is promised.
#[derive(SystemSet, Debug, Clone, Hash, PartialEq, Eq)]
pub enum ScriptSystems {
    /// Bounded event delivery, deferred child admission and one resume per eligible
    /// sequence in Last; no separate ECS/scheduler.
    Run,
    /// Non-yielding behavior callbacks in Update.
    Update,
    /// Non-yielding behavior callbacks in FixedUpdate, once per fixed step.
    FixedUpdate,
}

#[derive(Resource, Default, Clone, Copy)]
struct Clock {
    updates: u64,
    fixed: u64,
}

#[derive(Resource, Default)]
struct Retiring {
    pending: Vec<execution::Retirement>,
    dispatching: bool,
}

/// Installs one interpreted Luau backend. Requires Bevy AssetPlugin; creates no window.
/// Installing it never executes project code, discovers files or opens a listener.
pub struct LuauPlugin;
impl Plugin for LuauPlugin {
    fn build(&self, app: &mut App) {
        let mut backend = LuaScriptingPlugin::default();
        backend.scripting_plugin.context_initializers = vec![vm::initialize];
        backend
            .scripting_plugin
            .context_pre_handling_initializers
            .clear();
        app.add_plugins((BMSScriptingInfrastructurePlugin::default(), backend))
            .init_resource::<Clock>()
            .init_resource::<Retiring>()
            .init_resource::<Capacity>()
            .init_resource::<Diagnostics>()
            .init_resource::<kedra_wakeup::WakeDemand>()
            .add_systems(First, |mut clock: ResMut<Clock>| {
                clock.updates = clock.updates.saturating_add(1)
            })
            .add_systems(FixedUpdate, |mut clock: ResMut<Clock>| {
                clock.fixed = clock.fixed.saturating_add(1)
            })
            .add_systems(Update, execution::update.in_set(ScriptSystems::Update))
            .add_systems(
                FixedUpdate,
                execution::fixed_update.in_set(ScriptSystems::FixedUpdate),
            )
            .add_systems(
                Last,
                execution::step
                    .in_set(ScriptSystems::Run)
                    .before(kedra_wakeup::WakeSystems::Apply)
                    .after(ScriptingSystemSet::RunQueuedCallbacks),
            );
    }
}

/// Load/evaluate a fresh independent context, then schedule its entrypoints.
///
/// Explicit trusted-host authorization is required. Loading cannot use Scope
/// effects. Invalid source/API/budgets fail without publishing a runnable owner;
/// startup does not mutate an existing run. Capacity is host-owned.
pub fn start(world: &mut World, program: Program, arguments: Value) -> Result<Run, ScriptError> {
    execution::start(world, None, None, program, arguments, None)
}

/// Start a program owned by a live game/tool entity. Bevy's ordinary linked
/// hierarchy retires the run when its parent despawns. Stale parents allocate nothing.
pub fn attach(
    world: &mut World,
    parent: Entity,
    program: Program,
    arguments: Value,
) -> Result<Run, ScriptError> {
    if !world.entities().contains(parent) {
        return Err(ScriptError::new(ErrorKind::Stale, "Stale script parent"));
    }
    execution::start(world, Some(parent), Some(parent), program, arguments, None)
}

/// Validate a fresh VM before retiring the old generation. Failed candidates
/// preserve the old run; success resets all globals/tasks and returns a new handle.
/// Applied effects are not rolled back and arbitrary VM stacks are not migrated.
/// Replacing one active program does not consume an extra slot in its capacity pool.
pub fn replace(
    world: &mut World,
    old: Run,
    program: Program,
    arguments: Value,
) -> Result<Run, ScriptError> {
    if status(world, old).is_none() {
        return Err(ScriptError::new(
            ErrorKind::Stale,
            "Stale or foreign script run",
        ));
    }
    let parent = world.get::<ChildOf>(old.owner).map(ChildOf::parent);
    let target = world
        .get::<Instance>(old.owner)
        .and_then(|instance| instance.target);
    let new = execution::start(world, target, parent, program, arguments, Some(old))?;
    execution::finish(world, old, Status::Cancelled, "replaced")?;
    world.despawn(old.owner);
    execution::flush(world);
    Ok(new)
}

/// Read an owned run's outcome. Wrong worlds and removed owners return None.
pub fn status(world: &World, run: Run) -> Option<&Status> {
    (world.id() == run.world)
        .then(|| world.get::<Instance>(run.owner))
        .flatten()
        .map(|i| &i.status)
}

/// Revoke and release VM/tasks immediately. No already applied effect is undone.
/// Terminal outcomes remain terminal; a missing/foreign owner is an error.
pub fn cancel(world: &mut World, run: Run) -> Result<(), ScriptError> {
    execution::cancel(world, run)
}

/// Release terminal evidence/owner. Running runs are revoked before retirement.
/// Hosts own when terminal outcomes have been consumed; stale handles cannot revive.
pub fn retire(world: &mut World, run: Run) -> Result<(), ScriptError> {
    if world.id() != run.world || world.get::<Instance>(run.owner).is_none() {
        return Err(ScriptError::new(
            ErrorKind::Stale,
            "Stale or foreign script run",
        ));
    }
    execution::finish(world, run, Status::Cancelled, "retired")?;
    world.despawn(run.owner);
    execution::flush(world);
    Ok(())
}
