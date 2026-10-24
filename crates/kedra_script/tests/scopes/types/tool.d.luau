-- Test-owned tool stand-in. Do NOT combine with production editor types.
export type TestToolContext = RuntimeContext & {
    game: {count: () -> number, increment: () -> ()},
    editor: {selection: () -> string},
}
