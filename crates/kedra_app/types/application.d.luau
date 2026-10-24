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
    | "f1" | "f2" | "f3" | "f4" | "f5" | "f6" | "f7" | "f8" | "f12"
    | "select_all" | "copy" | "paste" | "cut"
export type Input = {kind: "close_window"}
    | {kind: "modifier", modifier: "shift" | "control" | "alt" | "super", pressed: boolean}
    | {kind: "button", pressed: boolean}
    | {kind: "click", element: string}
    | {kind: "click", position: Point}
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

-- Bootstrap-only functions captured by native.luau, not public after installation.
export type NativeApp = {
    receipt: (params: {id: OperationId}) -> Receipt,
    submit: ((method: "app.input", params: InputRequest) -> Receipt)
        & ((method: "app.resize", params: ResizeRequest) -> Receipt)
        & ((method: "app.capture", params: CaptureRequest) -> Receipt),
    window: () -> WindowId,
    node: (id: string) -> Node?,
}
export type AppApi = {
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
