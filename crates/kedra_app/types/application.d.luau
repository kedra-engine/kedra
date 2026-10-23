-- Requires kedra_script/types/runtime.d.luau. No editor or reflected World API.
-- Entity/window identities are decimal STRINGS, never full-width Luau numbers.
export type KedraWindowId = string
export type KedraEntityId = string
export type KedraOperationId = string
export type KedraPoint = {number}
-- Four logical window coordinates: left, top, right, bottom.
export type KedraRect = {number}
export type KedraRgba = {number}
export type KedraFont = {
    source: KedraNullable<string>, size: string, weight: string,
}
export type KedraNode = {
    id: string,
    entity: KedraEntityId,
    parent: KedraNullable<KedraEntityId>,
    window: KedraNullable<KedraWindowId>,
    rect: KedraRect,
    visible: boolean,
    enabled: boolean,
    center_unclipped: boolean,
    focused: boolean,
    text: KedraNullable<string>,
    value: KedraNullable<string>,
    font: KedraNullable<KedraFont>,
    background: KedraNullable<KedraRgba>,
    color: KedraNullable<KedraRgba>,
}
export type KedraKey = "enter" | "escape" | "tab" | "shift_tab" | "space"
    | "backspace" | "delete" | "left" | "right" | "up" | "down" | "home" | "end"
    | "f1" | "f2" | "f3" | "f4" | "f5" | "f6" | "f7" | "f8" | "f12"
    | "select_all" | "copy" | "paste" | "cut"
export type KedraInput = {kind: "close_window"}
    | {kind: "modifier", modifier: "shift" | "control" | "alt" | "super", pressed: boolean}
    | {kind: "button", pressed: boolean}
    | {kind: "click", element: string}
    | {kind: "click", position: KedraPoint}
    | {kind: "move", position: KedraPoint}
    | {kind: "drag", from: KedraPoint, to: KedraPoint}
    | {kind: "wheel", delta: KedraPoint}
    | {kind: "key", key: KedraKey}
    | {kind: "text", text: string}
export type KedraDelivered = {delivered: boolean}
export type KedraFrame = {
    path: string, width: number, height: number, uniform: boolean,
    window: KedraWindowId, frame: number,
}
-- submit() initially returns only id/state. Polling adds hash/detail fields.
export type KedraReceipt = {id: KedraOperationId, state: "pending"}
    | {id: KedraOperationId, state: "pending", request_hash: string, detail: KedraJsonNull}
    | {id: KedraOperationId, state: "succeeded", request_hash: string, detail: KedraJsonValue}
    | {id: KedraOperationId, state: "failed", request_hash: string, detail: string}
export type KedraInputRequest = {window: KedraWindowId, input: KedraInput}
export type KedraResizeRequest = {window: KedraWindowId, width: number, height: number, dpr: number?}
export type KedraCaptureRequest = {window: KedraWindowId}

-- Bootstrap-only functions captured by native.luau, not public after installation.
export type KedraNativeApp = {
    receipt: (params: {id: KedraOperationId}) -> KedraReceipt,
    submit: ((method: "app.input", params: KedraInputRequest) -> KedraReceipt)
        & ((method: "app.resize", params: KedraResizeRequest) -> KedraReceipt)
        & ((method: "app.capture", params: KedraCaptureRequest) -> KedraReceipt),
    window: () -> KedraWindowId,
    node: (id: string) -> KedraNode?,
}
export type KedraAppApi = {
    window: () -> KedraWindowId,
    -- Missing visible targets return nil; serialized null properties use json.null.
    node: (id: string) -> KedraNode?,
    wait_node: (id: string) -> KedraNode,
    input: (window: KedraWindowId, input: KedraInput) -> KedraDelivered,
    resize: (window: KedraWindowId, width: number, height: number, dpr: number?) -> KedraDelivered,
    capture: (window: KedraWindowId) -> KedraFrame,
    click: (id: string) -> KedraDelivered,
    field: (id: string, value: string) -> (),
}
export type KedraAppContext = KedraRuntimeContext & {app: KedraAppApi}
