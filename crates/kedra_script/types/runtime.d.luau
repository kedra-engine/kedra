-- Kedra runtime globals. Editor/application capabilities are separate definitions.
-- JSON null is a light-userdata sentinel, NOT Lua nil or an absent table field.
declare extern type JsonNull with end
export type JsonValue = boolean | number | string | JsonNull
    | {JsonValue} | {[string]: JsonValue}
export type Nullable<T> = T | JsonNull
declare extern type EntityHandle with end
export type ErrorKind = "invalid_input" | "capacity" | "load" | "runtime"
    | "budget" | "permission" | "stale"
export type ScriptError = {kind: ErrorKind, message: string,
    path: Nullable<string>, phase: Nullable<string>}
export type Log = {
    info: (message: string, fields: JsonValue?) -> (),
    warn: (message: string, fields: JsonValue?) -> (),
    error: (message: string, fields: JsonValue?) -> (),
}
export type RuntimeContext = {
    entity: EntityHandle?, params: {[string]: JsonValue}, log: Log, profiler: Profiler,
}
export type Profiler = {
    -- Synchronous CPU scope. Callback cannot yield; results/errors are preserved.
    scope: <T...>(name: string, callback: (...any) -> T..., ...any) -> T...,
    -- Logical elapsed duration, including yields; not CPU time. Owner cancellation closes it.
    operation: <T...>(name: string, callback: (...any) -> T..., ...any) -> T...,
    -- Absolute finite sample. Unit is immutable for a name within this run.
    counter: (name: string, value: number, unit: ("count" | "bytes" | "ms" | "percent")?) -> (),
}
declare extern type SequenceHandle with end
export type SequenceOutcome = {state: "queued" | "running" | "cancelled"}
    | {state: "completed", result: JsonValue}
    | {state: "failed", error: ScriptError}
export type Connection = {disconnect: (self: Connection) -> ()}
export type BehaviorContext = {
    -- Only attach supplies an entity; start also supports unattached Behaviors.
    entity: EntityHandle?, params: {[string]: JsonValue}, log: Log, profiler: Profiler,
    sequences: {
        start: (path: string, args: JsonValue?) -> SequenceHandle,
        status: (handle: SequenceHandle) -> SequenceOutcome,
        cancel: (handle: SequenceHandle) -> (),
    },
    events: {
        on: (target: EntityHandle, name: string,
            callback: (payload: JsonValue) -> ()) -> Connection,
    },
}
export type StopReason = "cancelled" | "retired" | "replaced"
    | "despawned" | "failed" | "budget_exhausted"

declare runtime: {
    -- Monotonic milliseconds since this run was admitted, including suspension.
    elapsed_ms: () -> number,
}
declare json: {null: JsonNull}
declare wait: {
    -- Positive integral future application updates, not rendered frames.
    updates: (count: number) -> (),
    -- Positive integral future FixedUpdate schedules; independent of display rate.
    fixed: (count: number) -> (),
    -- Positive seconds on Bevy virtual time; honors pause/time scaling.
    game_seconds: (seconds: number) -> (),
    -- Positive monotonic seconds; still needs an application update to resume.
    real_seconds: (seconds: number) -> (),
    -- Predicate is checked immediately, then once per future update until true.
    until_true: (label: string, predicate: () -> boolean, timeout_ms: number) -> (),
}
