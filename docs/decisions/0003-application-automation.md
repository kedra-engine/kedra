# ADR 0003: Shared application automation

## Decision and scope

Editors and non-editor Bevy hosts use `kedra_app::KedraAppPlugin` for the same
command registry, observations, frame-staged input and renderer capture. The
editor registers its startup adapter; runtime has no dependency on that adapter,
project creation, importer or editor kit. `kedra_ui` owns stable UI identities and
renderer-derived logical geometry, independent of HTTP and editor policy.

Feature-owned typed requests remain authoritative. Semantic draft commands are
not evidence of keyboard input. Pointer/keyboard automation emits ordinary Bevy
input events before `InputSystems`; it does not synthesize widget activation or
pretend to operate native folder pickers. Captures use actual renderer readback.
The application session and generational window handles reject stale targets.
UUID receipts distinguish admission, completion and failure; completed receipts
expire within the advertised 256-entry window. Active receipts are not evicted.

## Reuse and alternatives

Bevy Remote Protocol owns JSON-RPC parsing and ECS system dispatch. Its default
method resource is replaced immediately with an empty allowlist: arbitrary ECS
mutation, asset writes and generic reflection methods are not exposed. Tests use
a non-editor host and the public BRP mailbox to enforce this boundary.

The native BRP HTTP plugin lacks authentication middleware and bounded request
bodies. Rather than implementing an HTTP parser, Axum, Hyper and Tokio provide
HTTP/1, JSON extraction, body bounds, concurrency and timeouts. These are maintained
MIT-licensed ecosystem libraries already used widely with Bevy-compatible Rust.
`async-channel` is the existing BRP mailbox contract; `subtle` supplies
constant-time token comparison. Existing UUID, SHA-256, tempfile and image tooling
provide process identities, request digests, non-clobber publication and PNG output.
This costs additional network/runtime dependencies but avoids a custom protocol,
HTTP parser or scheduler. The transport exposes only explicitly registered methods,
not an unrestricted development/debug API.

## Transport and authority

There is no listener by default. Explicit enablement binds IPv4 loopback, requires
a per-process random bearer capability and rejects browser Origin headers. Tokens
are published to a new credential file, never command arguments or ordinary logs.
Enablement returns a pending endpoint. Non-clobber publication follows the first
main-world BRP request/cleanup cycle and acknowledged HTTP runtime/listener setup;
setup or publication failure requests an application error exit. This is not native
frame or feature readiness. Cleanup requires successful publication by this instance
and matching session/token; a destination that prevented publication is not owned.
Unix temporary files use owner-only permissions. On Windows the host must provide
a directory protected by an appropriate ACL; this has not been natively certified.
The token grants the application's existing user authority, not an OS sandbox.

Bodies are bounded at 64 KiB, concurrent requests at 16 and batches at 16. Mailbox
responses have a five-second timeout and whole HTTP requests six seconds. A timeout
does not imply rollback; read the receipt before retrying a side effect. Shutdown
cancels the server instead of waiting indefinitely for slow HTTP connections.
Only UUID-derived filenames are accepted for captures under a host-owned directory.

## Visual acceptance

The approved Figma remains the independent source of visual expectations. Native
captures are evidence, not automatic new baselines. Layout and image acceptance
must be based on recorded Figma node IDs and explicit expectations. A successful
compile, JSON-RPC response or synthetic-input receipt alone proves no visual fidelity.
OS picker, IME, native chrome interaction and other platforms require real hardware.

The startup window uses custom client chrome. Bevy owns native move, resize,
minimize, maximize and close
requests; the editor kit only renders the title strip and controls.
