-- Requires kedra_script/types/runtime.d.luau. No editor or reflected World API.
-- Entity/window identities are decimal STRINGS, never full-width Luau numbers.
export type WindowId = string
export type EntityId = string
export type OperationId = string
export type Point = {number}
-- Four logical window coordinates: left, top, right, bottom.
export type Rect = {number}
export type Rgba = {number}
export type Font = {
    source: Nullable<string>, size: string, weight: string,
}
export type Node = {
    id: string,
    entity: EntityId,
    parent: Nullable<EntityId>,
    window: Nullable<WindowId>,
    rect: Rect,
    visible: boolean,
    enabled: boolean,
    center_unclipped: boolean,
    focused: boolean,
    text: Nullable<string>,
    value: Nullable<string>,
    font: Nullable<Font>,
    background: Nullable<Rgba>,
    color: Nullable<Rgba>,
}
export type Key = "enter" | "escape" | "tab" | "shift_tab" | "space"
    | "backspace" | "delete" | "left" | "right" | "up" | "down" | "home" | "end"
    | "f1" | "f2" | "f3" | "f4" | "f5" | "f6" | "f7" | "f8" | "f9" | "f12"
    | "select_all" | "copy" | "paste" | "cut"
export type Input = {kind: "close_window"}
    | {kind: "focus_window"}
    | {kind: "modifier", modifier: "shift" | "control" | "alt" | "super", pressed: boolean}
    | {kind: "button", pressed: boolean, button: ("left" | "right" | "middle")?}
    | {kind: "click", element: string, button: ("left" | "right" | "middle")?}
    | {kind: "click", position: Point, button: ("left" | "right" | "middle")?}
    | {kind: "move", position: Point}
    | {kind: "drag", from: Point, to: Point}
    | {kind: "wheel", delta: Point}
    | {kind: "key", key: Key}
    | {kind: "text", text: string}
export type Delivered = {delivered: boolean}
export type Frame = {
    path: string, width: number, height: number, uniform: boolean,
    window: WindowId, frame: number,
}
-- submit() initially returns only id/state. Polling adds hash/detail fields.
export type Receipt = {id: OperationId, state: "pending"}
    | {id: OperationId, state: "pending", request_hash: string, detail: JsonNull}
    | {id: OperationId, state: "succeeded", request_hash: string, detail: JsonValue}
    | {id: OperationId, state: "failed", request_hash: string, detail: string}
export type InputRequest = {window: WindowId, input: Input}
export type ResizeRequest = {window: WindowId, width: number, height: number, dpr: number?}
export type CaptureRequest = {window: WindowId}
-- Rolling main-world measurements. These are not GPU or input-to-photon timings.
export type Performance = {
    enabled: boolean,
    focus_events: number,
    frame: Nullable<number>,
    update_intervals: {
        samples: number, capacity: number,
        mean_ms: Nullable<number>, min_ms: Nullable<number>, max_ms: Nullable<number>,
        p50_ms: Nullable<number>, p95_ms: Nullable<number>, p99_ms: Nullable<number>,
    },
    ui_changes: {[string]: {latest: Nullable<number>, mean_per_update: Nullable<number>}},
    inventory: {ui_nodes: number, text_nodes: number, editable_nodes: number, windows: number},
    changed_ui_nodes: Nullable<{{entity: EntityId, id: Nullable<string>, parent: Nullable<EntityId>,
        parent_id: Nullable<string>, node: Nullable<string>}}>,
}

-- Bootstrap-only functions captured by native.luau, not public after installation.
export type NativeApp = {
    performance: (options: {include_changed_nodes: boolean}?) -> Performance,
    receipt: (params: {id: OperationId}) -> Receipt,
    submit: ((method: "app.input", params: InputRequest) -> Receipt)
        & ((method: "app.resize", params: ResizeRequest) -> Receipt)
        & ((method: "app.capture", params: CaptureRequest) -> Receipt),
    window: () -> WindowId,
    window_focused: (window: WindowId) -> boolean,
    node: (id: string) -> Node?,
}
export type AppApi = {
    performance: (options: {include_changed_nodes: boolean}?) -> Performance,
    window: () -> WindowId,
    -- Missing visible targets return nil; serialized null properties use json.null.
    node: (id: string) -> Node?,
    wait_node: (id: string) -> Node,
    input: (window: WindowId, input: Input) -> Delivered,
    resize: (window: WindowId, width: number, height: number, dpr: number?) -> Delivered,
    capture: (window: WindowId) -> Frame,
    click: (id: string) -> Delivered,
    field: (id: string, value: string) -> (),
}
export type AppContext = RuntimeContext & {app: AppApi}
