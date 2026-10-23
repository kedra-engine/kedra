-- Requires runtime/application definitions. Editor observations never grant Save.
export type KedraEntry = {mode: "create" | "open", name: string, directory: string, error: string}
export type KedraProject = {name: string, root: string, format: number}
export type KedraEditorState = {
    phase: "boot" | "projects" | "loading" | "editor",
    generation: number, preferences: KedraNullable<string>, busy: boolean, ready: boolean,
    error: KedraNullable<string>, notice: string,
    project: KedraNullable<KedraProject>, entry: KedraNullable<KedraEntry>,
}
export type KedraDockGroup = {Group: {tabs: {string}, active: string}}
export type KedraDockSplit = {
    Split: {axis: "Horizontal" | "Vertical", fraction: number, first: string, second: string},
}
export type KedraDockLayout = {
    version: number,
    hosts: {[string]: {root: KedraNullable<string>, maximized: KedraNullable<string>}},
    nodes: {[string]: KedraDockGroup | KedraDockSplit},
    views: {[string]: {kind: string, restore_key: KedraNullable<string>}},
    hidden: {[string]: {host: string, group: string, index: number}},
}
export type KedraNodeAddress = {instances: {string}, node: string}
export type KedraDocuments = {
    active: KedraNullable<string>, pending: KedraNullable<string>, read_only: boolean,
    documents: {{document: string, path: string, view: string, selection: KedraNullable<KedraNodeAddress>}},
}
export type KedraSceneView = {ready: false}
    | {
        ready: boolean, spatial: boolean,
        translation: KedraNullable<{number}>, rotation: KedraNullable<{number}>,
        pivot: KedraNullable<{number}>, speed: KedraNullable<number>,
        orthographic: boolean, overlays: boolean, editor_lighting: boolean, aligning: boolean,
    }
export type KedraNavigationInput = {
    gesture: boolean, pending_host: boolean, menu_nodes: number, measuring: boolean,
    rendered: KedraNullable<number>, revision: number,
    capture: KedraNullable<{view: string, window: KedraWindowId}>,
    viewports: {{
        view: string, rect: KedraNullable<KedraRect>,
        -- These arrays retain JSON numeric identities in the current adapter.
        -- Out-of-range values are rejected by the lossless value boundary.
        eligible_windows: {number}, pointer_windows: {number},
    }},
}
export type KedraWorkspace = {bound: false}
    | {
        bound: true, native_alive: boolean, main_host: string, revision: number,
        layout: KedraDockLayout, documents: KedraDocuments,
        scene_views: {[string]: KedraSceneView}, navigation_input: KedraNavigationInput,
    }
export type KedraEditorApi = {
    state: () -> KedraEditorState,
    workspace: () -> KedraWorkspace,
}
export type KedraEditorContext = KedraAppContext & {editor: KedraEditorApi}

-- Present only in named docking diagnostic programs, not the editor/project API.
export type KedraDockDiagnostic = {ready: false} | {ready: true, generation: number}
export type KedraDockApi = {
    state: () -> KedraDockDiagnostic,
    reset: () -> (),
    drafts: () -> (),
    overflow: () -> (),
}
export type KedraDockContext = KedraAppContext & {dock: KedraDockApi}
