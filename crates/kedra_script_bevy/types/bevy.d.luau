-- Opt-in native capabilities; host authorization still governs each context.
export type KedraPosition = {x: number, y: number, z: number}
export type KedraTransformContext = {
    entity: KedraEntityHandle, params: {[string]: KedraJsonValue}, log: KedraLog,
    transform: {
        get_position: (target: KedraEntityHandle) -> KedraPosition,
        set_position: (target: KedraEntityHandle, position: KedraPosition) -> (),
    },
}
export type KedraUiContext = {
    entity: KedraEntityHandle, params: {[string]: KedraJsonValue}, log: KedraLog,
    ui: {set_background: (target: KedraEntityHandle, rgba: {number}) -> ()},
}
