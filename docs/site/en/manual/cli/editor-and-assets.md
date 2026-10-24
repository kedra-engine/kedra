# Editor startup and asset commands

The `kedra` application in `apps/kedra` opens the native editor when no subcommand
is supplied. Its `assets` subcommands run headlessly. From the repository root,
`cargo run -p kedra --` selects that application; tokens after the separator belong
to the application, not Cargo.

## Startup arguments

```sh
cargo run -p kedra -- --assets target/editor-assets
cargo run -p kedra -- /PATH/TO/PROJECT --assets target/editor-assets \
  --project-assets target/game-assets --state-directory target/cli-state
```

| Argument | Meaning and default |
| --- | --- |
| `[PROJECT]` | Optional existing project root, validated through normal opening |
| `--assets PATH` | Verified editor delivery; also available to asset subcommands |
| `--project-assets PATH` | Explicit game-namespace delivery for that project's Scene View; requires `PROJECT` |
| `--state-directory PATH` | Override local preference storage; omitted uses normal editor storage |

Without `PROJECT`, startup resolves the normal project-manager/history flow. This
is not a command for creating an empty running game. `--project-assets` supplies
preview resources; it does not process sources or authorize project execution.
Use [project opening](../projects/index.md) for authoring-project requirements.

Editor delivery selection is, in order: explicit `--assets`, `KEDRA_ASSETS`, then
the platform's executable-relative bundled location. The default does not search
the current working directory or source checkout. macOS bundles and installed
Linux layouts have platform-specific resource locations. Passing a relative
explicit path still resolves it from the caller's working directory.

## Debug-only shared launch options

Follow [Connecting to an application](../automation/connecting.md) for credential handling
and [scene lifecycle](../scenes/lifecycle.md) for startup-scene integration.

Debug-assertion builds flatten the shared application launch options into the editor:

| Option | Contract |
| --- | --- |
| `--scene PATH` | Override the host's startup scene; no default override |
| `--remote-port PORT` | Enable authenticated loopback diagnostics; unsigned 16-bit port, `0` chooses an available port |
| `--remote-credentials PATH` | New private credential file; required with `--remote-port` and vice versa |
| `--remote-artifacts PATH` | Renderer artifact directory; default `target/test/captures` |

Release schemas omit all four options; Cargo features do not restore them. A scene
override is not a plugin loader or script-execution permission. The shared option
validator rejects empty, over-4096-byte or syntactically invalid scene paths; the
startup lifecycle additionally checks source-catalog admission and scene membership.
Use full source-qualified logical paths such as
`editor://fixtures/catalog.scene.ron`, not filesystem paths or hardcoded UUIDs in
native host code. Persisted document and asset identities remain UUID-based.

Diagnostics are opt-in. The token is not passed on the command line; credentials
and captures must stay separate from project sources and public evidence. Configuring
remote access requires the host's application service to be installed and rejects
an empty artifact directory. Do not interpret publication of credentials as proof
that the renderer or UI is ready.

## Initialize source identities

```sh
cargo run -p kedra -- assets init /PATH/TO/PROJECT/assets
```

`assets init SOURCE` writes missing native `.meta` sidecars and preserves existing
identities/settings. It is an explicit authoring write, unlike verification. Review
the source tree first and keep sources and sidecars together when moving assets.
Initialization does not make unsupported source formats renderable or generate a
scene, game template or delivery. The builder also exposes this operation as
`cargo kedra assets init SOURCE`.

## Build a new delivery

The first example builds editor resources; the second uses `game-desktop` for a standalone
host that expects that label, not for the editor's project preview.

```sh
cargo run -p kedra -- assets build assets/editor target/editor-assets --namespace editor
cargo run -p kedra -- assets build /PATH/TO/PROJECT/assets target/game-assets \
  --profile game-desktop --namespace game
```

Syntax: `assets build SOURCE OUTPUT --namespace NAME [--profile PROFILE]`.
Both positional paths and `--namespace` are required. The profile defaults to
`editor-desktop`. Current editor project previews require that profile even with namespace
`game`; use `game-desktop` only when your standalone host expects it. Namespace and profile
are different inputs: selecting a namespace does not change the default profile.
Profile is a nonempty string of at most 64 bytes, not a Cargo feature name.
Namespace is 1–64 ASCII letters, digits, `_` or `-`.

Build reads authored sources/sidecars and publishes a new immutable processed
delivery containing manifest, loader metadata and hashes. It does not implicitly
initialize identities. `OUTPUT` must be a new directory; existing destinations are
refused rather than replaced. Source and delivery roots must be disjoint: neither
may contain the other. A failed publication leaves no usable delivery;
inspect the diagnostic before
choosing a fresh output.

## Verify without opening a window

```sh
cargo run -p kedra -- assets verify target/editor-assets
cargo run -p kedra -- assets verify target/game-assets --profile game-desktop
```

Syntax: `assets verify [ROOT] [--profile PROFILE]`. Verification checks manifest
members, hashes and loader metadata without writes. Its default profile is also
`editor-desktop`. With no positional `ROOT`, lookup uses `--assets`, then
`KEDRA_ASSETS`, then the executable-relative default. An explicit `ROOT` takes
precedence even when `--assets` is also supplied.

The parser requires an actual asset operation: bare `assets` fails. A flag such as
`--profile` requires a value. Processing, verification or startup errors produce
failure diagnostics and a nonzero exit, not a silent fallback delivery. Headless
asset commands report progress and success to stderr without prompting. Cargo may
still compile code before dispatching them.

Verification establishes integrity and format compatibility, not publisher trust,
freshness against edited sources, GPU readiness or correct pixels. A matching
project delivery is a separate requirement for Scene View; see
[asset processing](../assets/index.md) and [editor previews](../editor/index.md).
