-- Kedra runtime globals. Editor/application capabilities are separate definitions.
-- JSON null is a light-userdata sentinel, NOT Lua nil or an absent table field.
declare extern type KedraJsonNull with end
export type KedraJsonValue = boolean | number | string | KedraJsonNull
    | {KedraJsonValue} | {[string]: KedraJsonValue}
export type KedraNullable<T> = T | KedraJsonNull
declare extern type KedraEntityHandle with end
export type KedraErrorKind = "invalid_input" | "capacity" | "load" | "runtime"
    | "budget" | "permission" | "stale"
export type KedraScriptError = {kind: KedraErrorKind, message: string,
    path: KedraNullable<string>, phase: KedraNullable<string>}
export type KedraLog = {
    info: (message: string, fields: KedraJsonValue?) -> (),
    warn: (message: string, fields: KedraJsonValue?) -> (),
    error: (message: string, fields: KedraJsonValue?) -> (),
}
export type KedraRuntimeContext = {
    entity: KedraEntityHandle?, params: {[string]: KedraJsonValue}, log: KedraLog,
}
declare extern type KedraSequenceHandle with end
export type KedraSequenceOutcome = {state: "queued" | "running" | "cancelled"}
    | {state: "completed", result: KedraJsonValue}
    | {state: "failed", error: KedraScriptError}
export type KedraConnection = {disconnect: (self: KedraConnection) -> ()}
export type KedraBehaviorContext = {
    entity: KedraEntityHandle, params: {[string]: KedraJsonValue}, log: KedraLog,
    sequences: {
        start: (path: string, args: KedraJsonValue?) -> KedraSequenceHandle,
        status: (handle: KedraSequenceHandle) -> KedraSequenceOutcome,
        cancel: (handle: KedraSequenceHandle) -> (),
    },
    events: {
        on: (target: KedraEntityHandle, name: string,
            callback: (payload: KedraJsonValue) -> ()) -> KedraConnection,
    },
}
export type KedraStopReason = "cancelled" | "retired" | "replaced"
    | "despawned" | "failed" | "budget_exhausted"

declare runtime: {
    -- Monotonic milliseconds since this run was admitted, including suspension.
    elapsed_ms: () -> number,
}
declare json: {null: KedraJsonNull}
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
