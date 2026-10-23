-- Test-owned capability, not a production game SDK or Roblox API.
export type KedraTestGameContext = KedraRuntimeContext & {
    game: {count: () -> number, increment: () -> ()},
    editor: nil,
}
declare game_scope_only: boolean?
declare tool_scope_only: boolean?
