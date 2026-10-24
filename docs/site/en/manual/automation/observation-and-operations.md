# Observation and operations

Discovery and observation are reads. Input, resize, capture and scenario requests
admit effects that may finish on later application updates. Keep the JSON-RPC
exchange separate from the operation it starts.

## Inspect the current application

After [connecting](connecting.md), call `app.describe` and `app.observe` with ordinary
JSON-RPC request IDs. Observation needs no effect parameters:

```json
{
  "jsonrpc": "2.0",
  "id": "observe-1",
  "method": "app.observe"
}
```

The observation returns `session`, `frame`, `windows`, `native`, `cameras`, `screens`,
`elements`, `input_busy` and `truncated`. Window entries include logical/physical
dimensions, DPR, focus and primary-window status. Native entries report available
window-system state; cameras report their targets and viewport sizes.

An element can report stable `id`, runtime `entity`/`parent`, owning `window`, `rect`,
`visible`, `enabled`, `center_unclipped`, `focused`, `text`, editable `value`, font
metadata and colors. These are actual retained-tree observations, not an alternate
test layout or a complete accessibility tree. Text values and native metadata may
contain private information; do not publish unrestricted snapshots.

## Stable UI names and geometry

Hosts put `ElementId` and `ScreenId` on their real retained UI. Authored `ui.id` and
`ui.screen` supply those identities for live views; passive projections strip them.
Use stable UI names such as `projects.new`, freshly observed in their target window,
rather than hardcoding transient ECS entity handles.

The `app.observe` RPC window/entity identities are full-width integers. Preserve their exact
value in your client: a number type that rounds large integers can target the wrong identity.
They belong to the current process, not persisted scene/node identities. Re-observe
after window replacement, project transitions, resize or other layout changes.

Editor diagnostics use decimal strings directly for viewport `eligible_windows` and
`pointer_windows`, and for docking-fixture `host` and `fields`, including raw RPC results.
See the separate [editor](../../api/luau/editor.md) and
[test-fixture](../../api/luau/testing.md) references for those schemas.

`rect` is `[left, top, right, bottom]` in **logical window pixels**. The adapter accounts
for UI scale and camera viewport origin; do not multiply it by DPR again for input.
Capture image dimensions, in contrast, are physical pixels.

Duplicate visible IDs within a window make the snapshot fail. The element list caps
at 4096 entries and marks `truncated` when incomplete. Click-by-ID rejects a truncated
snapshot even if its prefix contains one apparent match: uniqueness beyond the prefix
has not been established.

An ID click freshly resolves one visible target in the requested window, rejecting
disabled targets, disabled ancestors and clipped centers. These checks do not prove
that no other widget covers the center. Visibility and bounds alone are not a full
hit-test or a guarantee that the feature will accept the action.

## Two different IDs

| Location | Purpose |
| --- | --- |
| Top-level JSON-RPC `id` | Correlates this HTTP request and response |
| Effect `params.id` | Non-nil UUID identifying one session-local operation and its arguments |
| Effect `params.session` | Current `app.describe.session`, rejecting stale process requests |
| Effect `params.window` where required | Live window ID from the current observation |

The examples below use placeholders for session/window data. Substitute a real
session and exact observed numeric window identity before sending. The UUID is an
illustrative operation key, not a reserved identity.

To resize through the normal input adapter, send:

```json
{
  "jsonrpc": "2.0",
  "id": "resize-1",
  "method": "app.resize",
  "params": {
    "session": "SESSION",
    "id": "123e4567-e89b-42d3-a456-426614174001",
    "window": 123,
    "width": 800,
    "height": 600
  }
}
```

An admission can return `state: "pending"`. It does not mean the resize is complete.
Poll the same operation:

```json
{
  "jsonrpc": "2.0",
  "id": "poll-1",
  "method": "app.operation",
  "params": {"id": "123e4567-e89b-42d3-a456-426614174001"}
}
```

Full receipts contain `id`, `request_hash`, `state` and `detail`. States are `pending`,
`succeeded` and `failed`. Completion is feature-specific: input/resize delivery returns
`{"delivered":true}`, while capture completion reports its published frame artifact.
After delivery, observe again to check whether the requested change actually occurred.

## Retry and failure rules

Use [Testing](../testing/index.md) to separate observed state and delivered effects from
renderer, pixel and physical-input evidence.

Keep the same operation ID and arguments for a matching retry. Reusing that ID with
different arguments is rejected. Completed results are terminal; late completions
cannot replace them. Prefer querying a receipt over resubmitting an uncertain effect.

The collection holds at most 256 receipts. Completed receipts expire oldest-first;
active ones are not evicted. `Unknown or expired operation` is an explicit error,
not a successful no-op. Once a receipt expires, this is not durable replay protection.
Have an external supervisor track operations and deadlines; this is not a transaction journal.

Validation can reject a request before admission; other failures appear in an admitted
receipt. Check both JSON-RPC `error` and receipt `state`/`detail`. A transport timeout
can occur after enqueueing: it is not rollback, and a fresh UUID may duplicate effects.

See [Programs and diagnostics](programs-and-diagnostics.md) for transport limits and
failure layers, and the [observation source][observe] and [receipt source][operations]
for exact process-local contracts.

[observe]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_app/src/observe.rs
[operations]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_app/src/operations.rs
