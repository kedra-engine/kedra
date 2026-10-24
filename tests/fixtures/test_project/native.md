# Ordinary native game and external verification

This project's ordinary Bevy game executable proves the no-editor/importer/test-tool
runtime dependency boundary. `Cargo.toml`, `src/`, scenes, finite programs and scoped
API declarations belong to this one project, not a separate consumer workspace.
Unique headless Luau API-isolation regressions belong to
[`kedra_script` product tests](../../../crates/kedra_script/README.md).

## One canonical content project

Native scene/scenario sources live only in this [test_project](README.md):

- `assets/Native/{remote,switch}.scene.ron`: native UI data, preserving document and
  node identities. Import sidecars use those same existing document IDs.
- `scripts/Native/{remote,switch}.luau`: finite application-owned sequences.
- `scripts/Native/switch_behavior.luau` and `switch_activate.luau`: the game-owned
  parameterized Behavior and its independently scoped child Sequence; real Bevy
  Activate notifications drive UI mutation, with no Rust activation implementation.
- `scripts/Native/projection.luau`: shared module used by named projection sequences;
  `.luaurc` maps `@game` to `scripts` in both LSP and the explicit runtime snapshot.
- `catalogs/native.prefabs.ron`: the host's fixed combined source admission catalogue.

The catalogue explicitly combines `game://Native/…` and `editor://…` sources. It is
outside imported assets, embedded by the game as
`application://catalogs/native.prefabs.ron` through Bevy's native memory asset source.
Each delivery remains independently verifiable; neither delivery's catalogue needs
the other delivery registered during import verification. No Ignore exception hides
cross-delivery dependencies. The editor delivery owns the projection scene/prefab
and their sidecars; the game consumes them through its explicit source catalogue.

Both `--assets VERIFIED_EDITOR_DELIVERY` and `--project-assets VERIFIED_GAME_DELIVERY`
are required resources for every composition, including normal switch startup.
They never select admission, native plugins or script permissions. There is one
Rust-selected capability set and no mode/profile/capability selectors. The game
does not read a `CARGO_MANIFEST_DIR` asset tree at runtime. Scenes/scripts are not
generated during startup or duplicated in another authored project.

The inert `ui.docking` schema is registered in every build so the complete catalogue
can publish in release. Release retains `game://Native/switch.scene.ron` startup,
native switch behavior and the shared game-facing Luau runtime. Scene overrides,
remote transport and diagnostic projection operations/programs are debug-only.
Loading a scene never discovers or executes a program. Native prefab lookup uses
source-qualified paths, loaded-catalogue membership and published-source equality,
not hardcoded document UUID lookup. Snapshots persist document/view identities,
not process-local entity handles.

## Concrete external verification

`native-switch` and `native-projection` are concrete `kedra-test-project`
package/bin contracts, not arbitrary-manifest runners. The manifest locates the
ordinary game executable in a complete project copy; other applications use reusable
`kedra_test` Cargo/process, authenticated named-program transport and evidence APIs
with their own Rust verifier.
There is no universal scenario DSL or automatic trust in script-reported success.

Each verifier owns exactly two independent processes and independently requires
successful native-close exits. The switch check exercises the real click, fixed
colors/geometry, live state and retained-control identity checks plus renderer
identity and both readback fences. The projection check drives reorder D, split D
right of A, real-input activation of B, maximize/restore, hide/show and topology
restore. It independently checks every topology, retained identities, Unicode draft,
live observations, renderer identity and all readback fences.

Named `projection.reorder`, `.split`, `.activate`, `.maximize`, `.visibility` and
`.restore` programs invoke the corresponding host-owned projection operations.
Scoped Rust operations implement native projection capability, not a scripting DSL
or replacement for the editor's separately tested production gestures.
`ctx.projection.state()` returns explicit unmounted/unprojected readiness while loading;
actual source/startup/UI failures propagate as errors. Panel IDs use the real scope
separator (`lab.D/draft`). The restore-step snapshot is checked against the original
active-C topology before clicking D again to inspect the retained draft.

GPU synchronization uses private temporary PNG readbacks that are validated and
discarded, not image references or screenshot goldens. Only the 22 production kit
button/field cases have screenshot coverage. No font-free/resource-free claim is
made for this game: delivered projection styles, icons and fonts are required.

## Run explicitly

Materialize the complete project once per named scenario under `target`, preserving
its three-level relative dependency paths. Build verified deliveries outside the
runtime process through existing tooling. Use fresh project/output directories;
generating a lockfile affects only the complete working copy, never canonical data:

```sh
mkdir -p target/native-switch target/native-projection
cp -R tests/fixtures/test_project target/native-switch/project
cp -R tests/fixtures/test_project target/native-projection/project
cargo generate-lockfile --manifest-path target/native-switch/project/Cargo.toml
cargo generate-lockfile --manifest-path target/native-projection/project/Cargo.toml
cargo run -p kedra -- assets build assets/editor target/test/editor-delivery --namespace editor
cargo run -p kedra -- assets verify target/test/editor-delivery
cargo run -p kedra -- assets build target/native-switch/project/assets target/native-switch/game-delivery --namespace game
cargo run -p kedra -- assets verify target/native-switch/game-delivery
cargo run -p kedra -- assets build target/native-projection/project/assets target/native-projection/game-delivery --namespace game
cargo run -p kedra -- assets verify target/native-projection/game-delivery
CARGO_TARGET_DIR="$PWD/target" cargo xtask test -- native-switch \
  --manifest-path "$PWD/target/native-switch/project/Cargo.toml" \
  --assets "$PWD/target/test/editor-delivery" \
  --project-assets "$PWD/target/native-switch/game-delivery" \
  --output "$PWD/target/native-switch/result"
CARGO_TARGET_DIR="$PWD/target" cargo xtask test -- native-projection \
  --manifest-path "$PWD/target/native-projection/project/Cargo.toml" \
  --assets "$PWD/target/test/editor-delivery" \
  --project-assets "$PWD/target/native-projection/game-delivery" \
  --output "$PWD/target/native-projection/result"
```

The switch check uses configured startup, without a scene override. The projection
check selects `--scene editor://fixtures/projection.scene.ron` on the same executable.
Manual remote composition uses `--scene game://Native/remote.scene.ron`; supply both
resource arguments and fresh shared `--remote-port`, `--remote-credentials` and
`--remote-artifacts` options. Keep credentials and native logs private. Physical OS
input and other platforms require separate acceptance; no headless/static check
establishes native interaction quality.

## Host readiness regressions

Readiness contracts live beside the host's `ctx.projection.state()` adapter in
`src/docking/scripting.rs`: unmounted state is explicitly unready, and source/UI
failures propagate before mounting. Shared schema/parser/startup contracts
belong to their product crates, not fixture CLI or catalogue-convention selftests.

This Cargo project is excluded from the root workspace. Both `native-switch` and
`native-projection` run these headless host contracts before building/launching the
ordinary game; a failure stops acceptance. `cargo xtask check` does not execute this
excluded workspace. After the complete-copy and lockfile preparation above, the
focused entrypoint is:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test --locked \
  --manifest-path target/native-switch/project/Cargo.toml \
  --bin kedra-test-project docking::scripting::tests
```

This is a separate host regression check, not input/frame acceptance or a replacement
for the externally supervised native scenarios. Report its execution and result
separately from those scenarios.

Open `tests/fixtures/test_project/native.code-workspace` for canonical Luau sources
with shared runtime/application definitions and this project's
`types/projection.d.luau`. Its projection context does not enter unrelated game/editor
scopes. See [Luau editing](../../../tools/luau/README.md).
