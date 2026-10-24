# Implementation limitations

Check attachment, geometry and resource behavior alongside the general
[context conventions](conventions.md) when using host-injected APIs.

## Behavior attachment annotation

`BehaviorContext.entity` is optional: Rust `start` permits unattached Behaviors.
Only `attach` supplies a target. Narrow the optional value before using an owned-object
operation. `TransformContext` and `UiContext` describe hosts that guarantee an
attached target; a type annotation alone cannot provide one. Sources:
`crates/kedra_script/types/runtime.d.luau`, `src/lib.rs` and `src/vm.rs` in that crate.

## Fixture geometry

`tests/fixtures/test_project/scripts/Native/remote.luau` checks height with
`rect[4] - rect[2]` and width with `rect[3] - rect[1]`, expecting 64 and 120 logical pixels
respectively. The rectangle is one-based `{left,top,right,bottom}`; index 0 is absent.
Those fixture-specific dimensions are not general UI API guarantees. Check them in a native
run, not just by reading the expressions.
See the [application reference](application.md#identities-geometry-and-nodes) for geometry rules.

## Contract qualifications

The [application function contract](application.md#public-functions) describes first
`PrimaryWindow` selection, not validation of unique native primary markers. The
[runtime resource contract](runtime.md#lifetime-and-resource-limits) distinguishes terminal
callback-time interrupts from caught allocation-limit failures. These qualifications
apply independently of type annotations.
