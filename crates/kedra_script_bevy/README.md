# kedra_script_bevy

This runtime-layer crate provides opt-in Bevy adapters for `kedra_script`, with no
editor, importer, source writer or generic reflected World API. Hosts install only
the bindings they authorize.
This separate crate keeps component/widget integration out of the VM execution owner.

- `install_transform` adds `ctx.transform.get_position/set_position(target, …)` for
  the run's attached live entity. Position is a local `{x, y, z}` value; writes require
  finite f32 coordinates and an existing `Transform`, preserving rotation and scale.
  It does not implement world-space conversion, physics synchronization or parenting.
- `install_ui` adds `ctx.ui.set_background(target, rgba)` for an existing
  `BackgroundColor`, using exactly four integral sRGB bytes. It does not create widgets.
- `ScriptBevyPlugin`, installed with `LuauPlugin`, forwards real Bevy `Activate` into
  the runtime's bounded `activate` notifications through native deferred commands.
  Programs must explicitly authorize that event and subscribe on their own target.

Both binding installers use `Scope::with_world` and `Scope::resolve_entity`; foreign,
stale, revoked and missing-component targets fail before mutation. Sources, persisted
identities and other entities remain unchanged. Already-applied effects are not rolled
back if a later operation fails. See the [runtime contract](../kedra_script/README.md).

The canonical test game's switch embeds a Behavior and child Sequence with independent
bindings. Its Behavior receives immutable color parameters and listens for Activate;
the child receives only owned arguments and the UI adapter. The debug native verifier
drives real input and observes the resulting control/frame without implementing the
switch in Rust. Release uses the same game-facing scripts, without remote transport.

`tests/contracts/owned_objects.rs` checks component ownership, invalid inputs and
Activate → Behavior → child integration in headless Bevy. Real input/frame acceptance
uses `native-switch` and remains a separate run. `types/bevy.d.luau` describes the
opt-in context fields for luau-lsp; declarations do not grant runtime permissions.
