# Programs and diagnostics

Registered application programs combine several diagnostic actions inside the running
application. They are finite Luau Sequences selected by name, not remotely supplied
source, arbitrary evaluation or editor Play. The Rust host owns their source, bindings,
admitted modules and execution authorization.

## Discover before starting

`app.describe.scripting` reports `language: "Luau"`, program names and
`source_eval: false`. Shared built-ins are `app.input`, `app.resize`, `app.capture`,
`app.click` and `app.field`. The first three wrap native operations; click resolves a
stable UI ID, while field waits for focus, selection, text delivery and observed value.

Program names and native method names are different selectors even when their spelling
matches. `app.input` as a JSON-RPC method takes an integer window identity and an
effect envelope. The `app.input` program runs through `app.scenario`; its application
API uses decimal-string native identities to avoid Luau number rounding.

Once the editor's `projects.new` target is ready, invoke the shared click program.
Replace the session and use a fresh UUID:

```json
{
  "jsonrpc": "2.0",
  "id": "scenario-1",
  "method": "app.scenario",
  "params": {
    "session": "SESSION",
    "id": "123e4567-e89b-42d3-a456-426614174003",
    "scenario": "app.click",
    "arguments": {"id": "projects.new"}
  }
}
```

Poll the operation UUID through `app.operation`; admission is not completion. Unknown
programs, stale sessions, source/eval fields and overlapping scenario admissions fail
closed. Arguments are owned bounded JSON, not executable code: at most 64 KiB, depth
32 and integers within the exact double range. The HTTP body bound also includes the
request envelope, so it is not an extra 64 KiB allowance for arguments.

Only one application scenario is active at a time. Matching retries reuse the receipt;
conflicting reuse of an operation ID is rejected. Native methods run in-process
through the same registered Bevy systems, without HTTP self-calls. Queued input still
traverses widgets; completing a scenario does not turn injected events into physical
input. See [Input and captures](input-and-captures.md).

## Cancellation is revocation, not rollback

`app.scenario_cancel` takes `session`, a **fresh** cancellation `id`, and `operation`
containing the active scenario's UUID. These are distinct operation identities.
Its receipt can report `{"cancelled":"UUID","rollback":false}` in `detail`.

The cancelled scenario subsequently finishes as failed. Already delivered input,
published artifacts or domain effects are not undone. A stale/nonactive operation
cannot cancel a different run. Keep external process supervision: an in-application
coroutine cannot supervise its own crash or a stopped frame loop.

## Editor-specific observations and programs

The editor registers `editor.state` for startup phase, jobs, readiness, error/notice,
project and actual form drafts. `editor.workspace` reports whether a workspace is
bound, its logical docking/document state and native binding. A `bound` or
`native_alive` value is not rendered-frame or interaction acceptance.

`editor.project` is a semantic adapter to the same validated project lifecycle used
by the UI. It can open forms, update drafts or request project operations, subject to
startup, mounting and busy rules. A semantic draft change is not evidence of keyboard
entry. Explicit-scene startup rejects project commands before operation admission.

The editor-owned `editor.startup.forms` program has a scoped `ctx.editor` containing
read-only `state()` and `workspace()` observations, alongside `ctx.app`. Its source
exercises DPR 1/2 geometry, modal focus, Unicode input, validation retention and
cancellation with renderer fences. Run it on the machine and revision you need to verify.

Other hosts can register different finite programs. Do not assume fixture-specific
`scene.switch`, `projection.exercise` or docking diagnostics are universal game APIs.
For runtime program lifetime and modules, use [Programming](../programming/index.md)
and the [Luau runtime contract][runtime].

## Diagnose the failure layer

| Symptom | Meaning and next check |
| --- | --- |
| HTTP 401 / 403 | Credential/header or Origin rejection; no feature receipt is implied |
| HTTP 413 / 429 | Body over 64 KiB or all 16 concurrent transport slots occupied |
| HTTP 408 | Six-second outer HTTP deadline elapsed; effects may still finish |
| JSON-RPC `-32600` | Invalid request envelope or batch size outside 1..16 |
| JSON-RPC `-32601` | Unknown method or unsupported `+watch` streaming request |
| JSON-RPC `-32602` | Invalid parameters, stale target, capacity/busy refusal or unknown/expired receipt |
| JSON-RPC `-32000` | Five-second mailbox wait failed/timed out; execution remains uncertain |
| Receipt `failed` | Admitted work failed; read private `detail` and inspect relevant unchanged state |
| Receipt stays `pending` | Updates/readback may be stalled; retain the external deadline and supervisor |

The transport accepts bounded JSON-RPC batches, not streaming observations. Notifications
omit the JSON-RPC request ID and return no normal response; use identified requests
when you need errors and effect receipts. An all-notification response is HTTP 204.
Avoid using batches as an atomic transaction: earlier commands can already have effects.

Runtime scripting also has structured `ScriptError` kinds and bounded diagnostics.
Do not assume every native RPC receipt preserves that full structure: the application
scenario adapter converts terminal script errors to receipt failure text. Preserve
private diagnostics locally; share only reviewed portable evidence, without credentials,
raw failure observations or unrestricted logs.

## Integration and evidence owners

For exact in-program calls, use the [application Luau API](../../api/luau/application.md),
[editor diagnostics](../../api/luau/editor.md) and
[test-fixture APIs](../../api/luau/testing.md).

[Programming](../programming/index.md) explains host-owned script integration;
[Native scenarios](../testing/native-scenarios.md) explains the external supervisor's workflow.
Select its command through the [CLI task map](../cli/index.md).

`RemoteAppExt` registers implemented feature systems; duplicate names are programmer
errors, not a remote extension mechanism. The shared app API grants no generic World
or editor authority. The tooling-side `kedra_test` client explicitly chooses programs
for input/resize/capture; its `operation` path invokes exactly the named native method,
without silent fallback. External Rust scenarios prepare the application, check results,
enforce process deadlines and publish reports. See the [automation contract][automation] and
[testing guide][testing] rather than treating an in-app program as a complete verifier.

[runtime]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/README.md
[automation]: ../../contributing/contracts/automation.md
[testing]: ../../contributing/contracts/testing.md
