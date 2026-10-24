-- Opt-in native capabilities; host authorization still governs each context.
export type Position = {x: number, y: number, z: number}
export type TransformContext = {
    entity: EntityHandle, params: {[string]: JsonValue}, log: Log,
    transform: {
        get_position: (target: EntityHandle) -> Position,
        set_position: (target: EntityHandle, position: Position) -> (),
    },
}
export type UiContext = {
    entity: EntityHandle, params: {[string]: JsonValue}, log: Log,
    ui: {set_background: (target: EntityHandle, rgba: {number}) -> ()},
}
