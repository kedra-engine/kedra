# Application automation

Use finite debug Sequences to observe UI and drive native operations. For game-object
changes, use the [game live-object API](bevy.md). Shared waits and
values are in the [runtime reference](runtime.md).

Declarations:
[application.d.luau](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_app/types/application.d.luau).
Public helper implementation:
[native.luau](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_app/src/scripting/native.luau).
Bindings: `crates/kedra_app/src/scripting/{bindings.rs,native.luau}`.
Catalog/lifetime: `crates/kedra_app/src/scripting.rs`.

`ctx.app` is installed only by a trusted host calling `install_app_api`. The module and
input/capture services exist only with `debug_assertions`. Shared gameplay scripting
remains available in release, but this automation API does not. Debug services are inert
until configured; a network listener requires explicit remote enablement.
See [connection setup](../../manual/automation/connecting.md), not a script-side installer.

## Public functions

```luau
ctx.app.window: () -> WindowId
ctx.app.performance: (options: {include_changed_nodes: boolean}?) -> Performance
ctx.app.node: (id: string) -> Node?
ctx.app.wait_node: (id: string) -> Node
ctx.app.input: (window: WindowId, input: Input) -> Delivered
ctx.app.resize: (window: WindowId, width: number, height: number,
    dpr: number?) -> Delivered
ctx.app.capture: (window: WindowId) -> Frame
ctx.app.click: (id: string) -> Delivered
ctx.app.field: (id: string, value: string) -> ()
```

`window`, `performance` and `node` are synchronous observations; other functions can yield
and belong in Sequences, not Behavior lifecycle/event callbacks.

| Function | Result and failure behavior |
| --- | --- |
| `window()` | Returns the primary window's decimal ID string; errors if no usable primary exists |
| `performance()` | Read-only rolling update intervals and UI-change diagnostics; not GPU or input-to-photon timings |
| `node(id)` | First visible matching UI identity, or nil; incomplete/ambiguous observation fails |
| `wait_node(id)` | Polls for a visible node for up to 40000 ms; timeout fails |
| `input(window, input)` | Awaits native delivery receipt, not application-specific acceptance |
| `resize(window,w,h,dpr?)` | Awaits delivery; input sizes are logical pixels, not an exact native-size guarantee |
| `capture(window)` | Awaits a renderer-produced frame, not a synthetic node screenshot |
| `click(id)` | Waits for a visible enabled target and its window's native focus, then submits an element click |
| `field(id,value)` | Clicks, waits for focus, selects all, inserts text, waits for exact value |

`window()` requires exactly one live `Window` with `PrimaryWindow`. This check does not
serialize or depend on the UI inventory.

`field` uses separate 40000 ms focus/value waits. Native operations poll receipts with a
12000 ms timeout and raise on failed receipts. The Sequence's total budget still applies
and may expire before a helper's own timeout. Stale windows, invalid input and native
feature failures are errors, not successful delivery. Cancellation revokes future script
effects but does not undo input, resize, captures or already queued native work.

`click` and the initial `field` click wait up to 40000 ms for native window focus without
requesting activation or retrying input. Window retirement fails the wait. This readiness
boundary avoids pressing while a newly shown or surviving window is still regaining focus.

When another live window legitimately owns focus, the caller must explicitly submit
`ctx.app.input(window, {kind = "focus_window"})` before the next interaction. This requests
activation once and waits up to eight seconds for backend/Bevy acknowledgment, without
replaying input. Injected holds, retirement and unacknowledged/unsupported activation fail.
After resize, await the feature's usable state or request a renderer fence separately.
Native rounding can change size; exact backend/Bevy/camera equality is not a generic readiness gate.

Native validation (`crates/kedra_app/src/input.rs`) requires resize width/height in
`320..=3840`, and optional DPR in `1..=4`, all finite. Pointer positions must be finite
and inside `[0,width) × [0,height)`. Wheel components must be finite with absolute value
at most `10000`; text input is at most 16 KiB UTF-8. Click requires exactly one of
`element` or `position`. Held-input/window conflicts and occupied queues can reject work.

## Identities, geometry and nodes

`WindowId`, `EntityId` and `OperationId` are strings. Native numeric
`id`, `entity`, `parent`, `window` fields are adapted to decimal strings to avoid integer
rounding; do not parse them into Luau numbers or use them as `EntityHandle`.
`Point` is two logical window coordinates; `Rect` is
`{left,top,right,bottom}` in logical pixels. Array lengths are runtime contracts, not
enforced by `{number}` annotations.

`Node` fields:

| Fields | Meaning |
| --- | --- |
| `id`, `entity`, `parent`, `window` | Stable UI label, observed entity, nullable parent/window |
| `rect`, `visible`, `enabled`, `center_unclipped` | Measured geometry and interaction observations |
| `focused` | Current native focus observation |
| `text`, `value` | Nullable display text / editable value |
| `font` | Nullable `{source, size, weight}`; source is nullable, size/weight are strings |
| `background`, `color` | Nullable arrays of four one-based sRGBA bytes in `0..=255` |

Serialized absent properties use `json.null`. `node(id)` instead returns nil for no visible
match. Its binding rejects truncated inventories and duplicate visible identities within
one window, but does not demand global cross-window uniqueness: it selects the first
visible matching ID. Use host-owned unique labels, especially with floating windows.
Snapshots are owned data; table edits do not update UI or prove a frame was presented.

## Input variants

`Input` is a tagged union. Use the exact `kind` and fields:

| `kind` | Additional fields |
| --- | --- |
| `focus_window` | None; explicitly requests and acknowledges native activation |
| `close_window` | None |
| `modifier` | `modifier: "shift" \| "control" \| "alt" \| "super"`, `pressed: boolean` |
| `button` | `pressed: boolean`, optional `button: "left" \| "right" \| "middle"` (defaults to left) |
| `click` | Either `element: string` or `position: Point` |
| `move` | `position: Point` |
| `drag` | `from: Point`, `to: Point` |
| `wheel` | `delta: Point` (two-component pixel delta, not an in-window position) |
| `key` | `key: Key` |
| `text` | `text: string` |

`Key` values are `enter`, `escape`, `tab`, `shift_tab`, `space`, `backspace`,
`delete`, `left`, `right`, `up`, `down`, `home`, `end`, `f1`, `f2`, `f3`, `f4`, `f5`,
`f6`, `f7`, `f8`, `f12`, `select_all`, `copy`, `paste`, `cut`, `undo`, `redo`.
This is not arbitrary keyboard/OS automation. Native input validation and receipt
settlement remain authoritative; see
[input/captures](../../manual/automation/input-and-captures.md).

`Delivered` is `{delivered: boolean}`. Verify feature state separately after delivery.
`Frame` contains `path`, physical pixel `width`/`height`, `uniform`, `window`, `frame`.
The path names host evidence, not an authored asset or script filesystem capability.
A nonuniform frame alone does not prove expected content.

## Registered programs

The native catalog supplies `app.input`, `app.resize`, `app.capture`, `app.field`,
`app.click`, each a finite host-owned Sequence. Hosts register further Sequences explicitly.
Only one application scenario is active at a time. Registration is not execution;
`app.scenario` selects a registered name with owned arguments, not remote source code.
Session/idempotency and cancellation belong to the
[automation protocol](../../manual/automation/programs-and-diagnostics.md).

Public `ctx.app` has no `submit` or `receipt`. Those declaration types belong to the
[private bootstrap](integration.md#application-private-bootstrap), not script-side dispatch.

## Checked-in example and tests

This excerpt from `crates/kedra_editor/src/catalog/prepare.luau` requires a host-mounted
component specimen. It does not mount that fixture or expose a catalog API:

```luau
function run(ctx: AppContext, args)
    local app = ctx.app
    local window = args.window
    app.resize(window, 640, 360, 1)
    wait.until_true("measured specimen", function()
        local target = app.node("specimen.target")
        return target ~= nil and target.rect[3] > target.rect[1] + 1
    end, 12000)
    return {window = window, delivered = true}
end
```

The excerpt omits the program's later hover/press/focus preparation.
`crates/kedra_app/src/scripting/tests.rs` tests the queue/VM route, opaque IDs,
unknown/source/stale request rejection, single-active/idempotent admission, failure
recovery and cancellation without rollback. Native input and rendered frames require
separate native tests.
