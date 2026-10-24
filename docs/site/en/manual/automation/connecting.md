# Connecting to a debug application

The diagnostic endpoint belongs to one running application process. There is no
unauthenticated discovery endpoint, public-interface binding or release remote-control
mode. Use a trusted local client that can keep a bearer credential private.

## Prerequisites and explicit opt-in

The host must install its ordinary Bevy asset, input, window and rendering plugins as
needed, then `kedra_app::KedraAppPlugin` and its native feature APIs. The shared plugin
creates neither a window nor a renderer. Only explicit remote configuration opens a
listener; merely linking the crate or installing the plugin is insufficient.

Hosts compose `kedra_app::launch::LaunchOptions` or `RemoteOptions` into their CLI.
The remote fields exist only with debug assertions. `--remote-port` and
`--remote-credentials` require each other; port `0` selects an available loopback port.
`--remote-artifacts` selects the capture directory, defaulting to
`target/test/captures`. It is distinct from project sources and credentials.

Prepare a matching editor delivery at `target/editor-assets`, then launch with new,
private diagnostic paths:

```sh
cargo run -p kedra -- --assets target/editor-assets \
  --remote-port 0 --remote-credentials target/test/local-session.json \
  --remote-artifacts target/test/local-captures \
  --state-directory target/test/isolated-state
```

`--assets` and `--state-directory` are editor arguments, not universal remote fields.
Use [Getting started](../getting-started/index.md) for normal resource preparation.
Do not add `--remote-port`, `--remote-credentials` or `--remote-artifacts` to a release
executable: its parser does not expose these debug-only options.

## Credential publication is a readiness boundary

The credential destination must not exist, including as a symlink. Configuration
binds `127.0.0.1` synchronously and prepares a private temporary credential file.
The returned endpoint and the launch log are still **pending**.

Publication happens without overwriting only after the first main-world BRP
request/cleanup cycle and acknowledged HTTP runtime/listener initialization. If
initialization or publication fails, the application requests an error exit rather
than treating incomplete setup as success. A path created during startup also causes
publication to fail closed.

Have the external supervisor bound the wait for publication and notice process exit.
File publication is not evidence of a mounted form, loaded project, native window
acknowledgment or renderer readiness. Those require later observations and frames.

The file is a JSON `Endpoint` with these exact fields:

| Field | Meaning |
| --- | --- |
| `url` | Current `http://127.0.0.1:PORT/` endpoint |
| `token` | Random bearer capability for this process |
| `session` | Process identity used to reject stale effect targets |

No token value is shown here. Read the actual file locally; do not paste it into
chat, commit it, print it in a command transcript or attach it to a report.
The token is not a CLI argument and the launcher does not print it.

Unix credential files are owner-only. On Windows, the containing directory must
already have an appropriate ACL; the service does not establish that directory ACL.
Protect the parent directory and any private client logs as well as the file itself.

## Authenticate every request

Send HTTP `POST` requests to the discovered URL with `Content-Type: application/json`
and `Authorization: Bearer TOKEN`, substituting the private token in the client.
This describes the header format, not a recommendation to expose the token through
shell arguments or public logs.

Start with an `app.describe` request:

```json
{
  "jsonrpc": "2.0",
  "id": "describe-1",
  "method": "app.describe"
}
```

The response's `result` advertises `protocol: 1`, `session`, `methods`,
`operation_capacity`, coordinate/input descriptions and `scripting`. Use the returned
inventory, not an assumption that editor-specific methods exist on a game host.
See [Observation and operations](observation-and-operations.md) for effects and polling.

Any `Origin` header is rejected, including same-machine browser origins. There is no
wildcard CORS or browser discovery route. Missing/wrong authentication yields HTTP
401; an Origin header yields 403. Neither is an operation failure receipt.

## Session end and stale credentials

Normal resource teardown stops and joins the HTTP server thread. It removes only this
instance's successfully published regular credential file whose session and token
still match. Replacement files and replacement symlinks are not blindly deleted.

A killed process may leave a stale file. Do not reuse its session or window IDs in
another process, and do not automatically overwrite the path. Give the next session
a fresh credential destination; inspect and clean up stale private material under
the supervisor's ownership.

Successful termination is a process-lifecycle fact, not an HTTP receipt. A close
request may retire its own target before its input receipt completes. The host may
also refuse a close request. See [Input and captures](input-and-captures.md).

## If connection fails

For launch syntax, use [editor CLI options](../cli/editor-and-assets.md). For supervised
process readiness and cleanup, continue to [Native scenarios](../testing/native-scenarios.md).

- No file yet: inspect private startup diagnostics and process status under a deadline.
- Existing destination: select a new path; do not remove unfamiliar user content.
- HTTP 401/403: check the credential source and headers, not application UI readiness.
- Endpoint responds but UI is absent: inspect windows and feature state separately.
- Timeout after an effect: query its operation before deciding whether to retry.

The [transport source][transport] owns credential lifecycle and transport limits;
the [shared launch source][launch] owns the debug CLI fields.

[transport]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_app/src/transport.rs
[launch]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_app/src/launch.rs
