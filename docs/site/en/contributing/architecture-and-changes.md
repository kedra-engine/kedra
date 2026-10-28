# Architecture and changes

The [Rust API guide](../api/index.md) explains how to inspect generated contracts;
the [Luau API overview](../api/luau/index.md) separates runtime and host-owned scopes.

A change should have one clear owner, named consumers and defined behavior on failure.
Use the [codebase map](codebase-map.md) to find the current owner, then read the
[architecture contract][architecture] and relevant ADRs. Check each ADR's status:
proposed editor interactions are not implemented APIs or permission to build them.

## Extend the mechanism that already owns the guarantee

Before adding infrastructure, search the repository and maintained ecosystem tools.
Compare compatibility, maintenance, license/security and dependency cost. Bevy owns ECS,
reflection, scheduling and assets; Kedra uses ordinary crates and plugins rather than a
parallel ECS, DI system, scheduler or proprietary UI language.

For example, a delivery-admission change should use the existing asset/import boundary,
not create an editor-only validator with a second admission policy. A source-selection
change belongs in the editor's source adapter, not in consumer-neutral reflection values.
Trace the current failure path and its callers before deciding which API needs to move.

If no maintained solution fits, document the specific gap and why focused custom code is
necessary. “Kedra policy” alone does not justify rewriting a parser, walker or process
runner whose underlying mechanics already have a maintained owner.

## Decide whether a new crate is necessary

Keep a cohesive responsibility together. Extract a mechanism when it has an independent
purpose, a small useful API, a real consumer and a valid place in the DAG. One consumer
is sufficient; generic future reuse is not required. Conversely, do not create an empty
forwarding crate, speculative framework or numbered fragments just to reduce file size.

The existing split between `kedra_script` and `kedra_script_bevy` is concrete: VM lifetime
and capability scopes remain separate from optional Transform/widget integration. The
editor still owns its host-specific workspace API and execution authorization. Read the
[binding crate contract][bindings] before extending that boundary.

A boundary proposal should answer:

- Which values and resources does the mechanism own, and when do they retire?
- What does the host authorize, schedule or display rather than delegate?
- Which production consumer uses the API now?
- Which layer and dependency edges are necessary, including build/optional/target edges?
- What public errors and unchanged-state guarantees will tests observe?

Keeping an independent mechanism inside a host also needs rationale. Avoid a catch-all
editor crate merely because the editor happens to be the first consumer.

## Check the declared dependency boundary

The allowed destination layers are:

| Source layer | Allowed destination layers |
| --- | --- |
| `core` | `core` |
| `runtime` | `core`, `runtime` |
| `authoring` | `core`, `runtime`, `authoring` |
| `import` | `core`, `runtime`, `authoring`, `import` |
| `editor` | `core`, `runtime`, `authoring`, `import`, `editor` |
| `tooling` | All declared layers |

Every member declares `package.metadata.kedra.layer`, inherits workspace lints and uses
`MIT OR Apache-2.0`. The [architecture gate][gate] reads Cargo metadata and manifests;
optional, renamed and inactive target dependencies do not hide a forbidden edge.
The Inspector-specific restriction is described in the [codebase map](codebase-map.md).

Run `cargo xtask architecture` for that policy. Passing it does not establish runtime
capability isolation: the gate excludes dev dependencies and does not inspect the meaning
of code inside a package. Runtime must still avoid editor, importer and filesystem
authoring-writer dependencies. Do not add compatibility forwarding layers to evade this.

## Design public API and lifecycle together

Document invariants, errors, ownership and lifecycle semantics where the API is defined.
Specify preparation versus publication, cancellation versus completion, and what happens
to stale work. Filesystem commit, authoring transaction and deferred ECS mutation are
different guarantees; avoid promising rollback across all three.

For example, a prepared docking candidate belongs to a particular workspace revision.
An API receiving a stale candidate must reject it without replacing published topology.
For async host work, identify the generation/owner used to reject obsolete completion,
not only the task's successful return value.

Native scene/prefab references use source-qualified logical paths such as
`editor://prefabs/button.prefab.ron`, resolved through the existing loaded catalog with
membership and published-revision checks. Do not introduce hardcoded document UUIDs or a
parallel lookup registry. Persisted IDs, sidecars and provenance retain UUID identity;
the logical lookup path is neither a filesystem locator nor a replacement identity.

Sources own Save. Runtime state, drafts and derived deliveries remain separate. Loading
a scene does not authorize script execution. Source-editing APIs do not imply visual
editing, editor Save or Play. Only the documented Inspector source edits and
single-existing-document Save are implemented; general authoring and Play remain unavailable.

## Review features, safety and compatibility

Justify new public APIs and dependencies. Inspect exact resolved versions and enabled
features, not only upstream latest documentation. Git-patched dependencies require the
resolved source. The root manifest currently pins Bevy and patches Winit; consult the
lockfile and [third-party guide][third-party] before relying on external API behavior.

`--all-features` alone cannot prove default, minimal or target-specific behavior.
Run the relevant feature/consumer routes; mutually exclusive features or new platform
commitments need an ADR and CI matrix change. `cargo xtask features` includes separate
minimal runtime checks and debug-assertions-disabled diagnostic-boundary tests, not an
optimized editor release or native acceptance.

Unsafe code is forbidden in workspace code. A local lint override is not an exception;
changing that policy needs an ADR, reviewed safety argument and appropriate Miri tests.
Preserve Unicode and platform path semantics, and keep absolute filesystem paths out of
in-project asset labels without changing persisted identity or native locators.

Finish the design with a [test/review plan](tests-and-review.md), affected docs and explicit
non-goals. Durable cross-cutting decisions belong in ADRs, not only review discussion.

[architecture]: https://github.com/kedra-engine/kedra/blob/main/docs/architecture.md
[bindings]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script_bevy/README.md
[gate]: https://github.com/kedra-engine/kedra/blob/main/xtask/src/architecture.rs
[third-party]: https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md
