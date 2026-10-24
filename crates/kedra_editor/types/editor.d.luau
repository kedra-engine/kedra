-- Requires runtime/application definitions. Editor observations never grant Save.
export type Entry = {mode: "create" | "open", name: string, directory: string, error: string}
export type Project = {name: string, root: string, format: number}
export type EditorState = {
    phase: "boot" | "projects" | "loading" | "editor",
    generation: number, preferences: Nullable<string>, busy: boolean, ready: boolean,
    error: Nullable<string>, notice: string,
    project: Nullable<Project>, entry: Nullable<Entry>,
}
export type DockGroup = {Group: {tabs: {string}, active: string}}
export type DockSplit = {
    Split: {axis: "Horizontal" | "Vertical", fraction: number, first: string, second: string},
}
export type DockLayout = {
    version: number,
    hosts: {[string]: {root: Nullable<string>, maximized: Nullable<string>}},
    nodes: {[string]: DockGroup | DockSplit},
    views: {[string]: {kind: string, restore_key: Nullable<string>}},
    hidden: {[string]: {host: string, group: string, index: number}},
}
export type NodeAddress = {instances: {string}, node: string}
export type Documents = {
    active: Nullable<string>, pending: Nullable<string>, read_only: boolean,
    documents: {{document: string, path: string, view: string, selection: Nullable<NodeAddress>}},
}
export type SceneView = {ready: false}
    | {
        ready: boolean, spatial: boolean,
        translation: Nullable<{number}>, rotation: Nullable<{number}>,
        pivot: Nullable<{number}>, speed: Nullable<number>,
        orthographic: boolean, overlays: boolean, editor_lighting: boolean, aligning: boolean,
    }
export type NavigationInput = {
    gesture: boolean, pending_host: boolean, menu_nodes: number, measuring: boolean,
    rendered: Nullable<number>, revision: number,
    capture: Nullable<{view: string, window: WindowId}>,
    viewports: {{
        view: string, rect: Nullable<Rect>,
        -- Full-width native window identities, not Luau numbers.
        eligible_windows: {WindowId}, pointer_windows: {WindowId},
    }},
}
export type Workspace = {bound: false}
    | {
        bound: true, native_alive: boolean, main_host: string, revision: number,
        layout: DockLayout, documents: Documents,
        scene_views: {[string]: SceneView}, navigation_input: NavigationInput,
    }
export type EditorApi = {
    state: () -> EditorState,
    workspace: () -> Workspace,
}
export type EditorContext = AppContext & {editor: EditorApi}

-- Present only in named docking diagnostic programs, not the editor/project API.
export type DockDiagnostic = {ready: false} | {ready: true, generation: number}
export type DockApi = {
    state: () -> DockDiagnostic,
    reset: () -> (),
    drafts: () -> (),
    overflow: () -> (),
}
export type DockContext = AppContext & {dock: DockApi}
