-- Shared project's native debug scope only. Requires runtime/application definitions.
-- This describes observations and operations, not authority for arbitrary World access.
export type NativeProjectionAction = "reorder" | "split" | "activate"
    | "maximize" | "visibility" | "restore"
export type NativeProjectionView = {
    id: string,
    content: EntityId,
    tab: Nullable<string>,
    group: Nullable<string>,
    hidden: boolean,
}
export type NativeProjectionGroup = {Group: {tabs: {string}, active: string}}
export type NativeProjectionSplit = {Split: {
    axis: "Horizontal" | "Vertical", fraction: number, first: string, second: string,
}}
export type NativeProjectionSnapshot = {
    version: number,
    hosts: {[string]: {root: Nullable<string>, maximized: Nullable<string>}},
    nodes: {[string]: NativeProjectionGroup | NativeProjectionSplit},
    views: {[string]: {kind: string, restore_key: Nullable<string>}},
    hidden: {[string]: {host: string, group: string, index: number}},
}
export type NativeProjectionMounted = {
    mounted: true,
    projected: boolean,
    snapshot: NativeProjectionSnapshot,
    views: {[string]: NativeProjectionView},
}
export type NativeProjectionState = {mounted: false, projected: false}
    | NativeProjectionMounted
export type NativeProjectionApi = {
    -- Loading returns mounted=false; source/startup/UI failures raise an error.
    state: () -> NativeProjectionState,
    -- Names resolve uniquely against the live model. Target must be visible.
    -- Tab placement requires nil edge/fraction; edges require a valid fraction.
    move: (name: string, target: string, edge: ("left" | "right" | "top" | "bottom")?,
        index: number, fraction: number?) -> (),
    -- nil restores the full host tree; named views must be visible.
    maximize: (name: string?) -> (),
    visibility: (name: string, target: string, visible: boolean) -> (),
    -- Restores authored topology while retaining registered content/drafts.
    restore: () -> (),
}
export type NativeProjectionContext = AppContext & {projection: NativeProjectionApi}
