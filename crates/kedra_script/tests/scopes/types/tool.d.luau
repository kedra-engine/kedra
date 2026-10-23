-- Test-owned tool stand-in. Do NOT combine with production editor types.
export type KedraTestToolContext = KedraRuntimeContext & {
    game: {count: () -> number, increment: () -> ()},
    editor: {selection: () -> string},
}
