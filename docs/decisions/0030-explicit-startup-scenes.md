# ADR 0030: Explicit entry scenes and host-owned native capabilities

Status: implemented; complete native composition acceptance remains pending.

## Decision

Applications may select a different startup scene by Bevy asset path. Runtime-layer
`kedra_scene::startup` owns loading and one-shot instantiation over the existing
`PrefabPlugin` catalog. It does not introduce another scene format, discovery path,
reflection registry, scheduler, project writer or automatic project execution.

Only Scene documents explicitly included in the host's source catalog are admitted.
Missing, invalid, out-of-catalog and Prefab entries fail without choosing a fallback.
Instantiation waits for the complete published source revision and its dependencies.
Mounted state is not GPU readiness. Removing the root retires startup without an
implicit reload. Automatic transitions and Play are outside this decision.

## Application launch and release boundary

The application's normal executable is the entry point. Common launch arguments
belong to runtime-layer `kedra_app::launch`, not the editor, test runner or a copied
game template. Games flatten `LaunchOptions` into their own clap Parser and apply
it to the same Bevy App after installing native plugins and registering their APIs.
The editor uses the same `LaunchOptions` rather than owning another transport config.
Its explicit debug scene entry excludes the normal project-first startup; combining
it with a project root or game delivery is rejected before asset loading.
Both editor lifecycles install the same `EditorPlugin` and native capabilities;
explicit-scene startup rejects project operations rather than selecting another
editor capability mode.

`--scene` overrides the host's normal entry path only in builds with
`cfg(debug_assertions)`. Remote port, credential and artifact flags have the same
compile-time boundary. Release parsers reject them; environment variables and Cargo
features do not restore them. The application transport, injected-input/capture
services and named test-program API are excluded from release modules, not merely
hidden in help or disabled by a runtime boolean. Dependency resolution still includes
the declared packages; this is not a claim of an HTTP-free dependency graph.

Ordinary source startup and the game-facing `kedra_script` runtime remain available
in release. A shipped game still instantiates its configured entry scene and may
explicitly execute authorized game code. Only diagnostic overrides/services are removed.
Custom profiles must treat enabling debug assertions as opting into diagnostic builds.

Reuse the already maintained clap 4.x parser/derive stack, under its MIT/Apache
licenses, for the shared argument schema. It adds parser code to native application
consumers but avoids hand-written parsing and divergent credential/flag rules.

The architecture has no separate Cargo targets solely for selecting a scene,
enabling remote or defining a test layout. The normal editor/game executable supplies
its compiled capabilities; scenes and Luau programs supply compositions and sequences.
Concrete external native verifiers retain process supervision through shared
`kedra_test` mechanisms.
Bindings activate on authored native components, not scene-path dispatch or hidden
per-fixture modes. The catalog retains scoped text measurement and calibration.

## Host capabilities

The host selects native plugins and a mount function in Rust. Ordinary instantiation
is the default; `kedra_ui::authored::view::UiView::mount_scene` supplies existing live
UI hydration. This single function boundary is necessary because scene ownership
must not depend on UI or editor crates. It is not a plugin registry or DI container.
Adapters must publish a live owned root or fail without leaving a partial instance.
The editor uses desktop's `replace_mounted` mount/rollback callbacks. A returned
mount error or later native root/camera/visibility failure retires candidate
bindings before candidate resources, preserving the prior primary surface and focus.

Scenes describe composition, source identities and initial component values.
Native systems implement controls, docking gestures, operating-system windows and
rendering. Initial docking topology must use the existing workspace model/snapshot,
not a second tree reconstructed from UI geometry. Scenes cannot choose arbitrary
Rust plugins or grant editor authority to an independent runtime host.

Luau scripts drive the running composition through explicitly scoped bindings.
Loading a scene does not discover, load or execute scripts. Hosts explicitly register
trusted programs and an external caller starts a named scenario through the existing
authenticated protocol. Arbitrary remote source/eval remains unavailable. Rust retains
process supervision, wall-clock deadlines and independent artifact validation.

## Consumer and verification boundaries

The independent application's ordinary executable selects remote-panel, switch and
delivered projection scenes. It has one Rust-selected capability set and one fixed
application-owned catalogue; there are no capability/profile/per-fixture selectors.
Required resource arguments never select source admission, native plugins or startup.
The same two verified deliveries are required for every scene, including the default
switch: `--assets` (editor) and `--project-assets` (game). Missing/invalid resources fail
before constructing the app. Scene paths select composition, never capabilities.
The single canonical `tests/fixtures/test_project` owns `assets/Native` scene data,
`scripts/Native` programs and `catalogs/native.prefabs.ron` host admission. The
same project's ordinary `kedra-test-project` Cargo executable has runtime-only
dependencies, without a separate native consumer project or per-scenario target.
It embeds its canonical host catalogue as a native Bevy memory asset, outside either
processed delivery.
This avoids cross-delivery importer dependencies without duplicate content projects
or Ignore exceptions. That catalogue combines its remote/switch scenes with
delivered projection sources directly; no redundant editor-only projection subset
catalogue is needed. Editor delivery verification never depends on application-local
files. Projection scene/panel identities, provenance and sidecars are preserved.
Local application sources use the explicit `game://` namespace, including the
default `game://Native/switch.scene.ron` entry; the host catalogue is
`application://catalogs/native.prefabs.ron`. Native docking
prefab resolution checks loaded-catalogue membership and equality with the published
source revision before retrieving compiled content. Asset paths are source-qualified
logical locators, not hardcoded document UUID lookup or absolute filesystem paths.

The common runner's explicitly concrete `native-switch` and `native-projection`
scenarios supervise that executable. Their
`--manifest-path` locates a complete per-scenario project copy and its
`kedra-test-project` ordinary package/bin, not an
arbitrary application contract. Generic Cargo discovery, authenticated named-program
transport, process supervision and evidence validation remain in `kedra_test`.
Other consumers supply ordinary Rust verification, not a universal scenario DSL.
The switch scenario executes ordinary Luau, while Rust validates its observations,
renderer receipts and two independent process lifecycles. Neither host imports editor,
authoring or importer code; the native control activation remains an application system.
Independent docking action choices live in named, scoped Luau programs.
Rust retains scoped native model operations/projection and independent
topology, retained-draft, live observation, renderer/readback and two-process checks.
Native close uses the normal host policy and both successful child exits are required.
The inert `ui.docking` schema is registered in every build, so the fixed catalogue
can publish in release without compiling debug projection operations/programs.
Release retains ordinary switch startup and the shared game-facing Luau runtime.
Read-only projection readiness distinguishes unmounted state from source/startup/UI
failure; scoped Luau declarations live with the ordinary game, not global editor
or unrelated game definitions. Unique headless Luau API-isolation checks live in the
`kedra_script` product tests rather than a separate consumer workspace.

The component catalog uses an authored canvas with button, field and icon slots
instead of constructing native layout nodes. Existing production prefabs remain
separate instances inside those slots so text-source addresses do not change.
Its 22 states use a registered Luau input program, not 22 copied layouts. Independent
Figma inputs, fixed regions, accepted SVG pairs and reviewed policy are not rewritten.

Docking initial topology is authored in `ui.docking`, an inert native component in
`kedra_ui::docking::scene`. It embeds the existing versioned JSON Snapshot, decoded
with bounded, duplicate-key and ownership validation, not a second layout schema or
parser. Snapshot view restore keys name catalog-loaded panel sources; native adapters
bind retained content. This responsibility belongs with docking presentation in
`kedra_ui`, with both editor and independent projection consumers, not a new host
framework. The editor retains production gestures, reset, draft retention, native
hosts and bounded traces. Diagnostic bindings remain debug-only. Initial assignment
does not replace tested gestures with remote topology mutation.
The production editor preset is read from `editor://scenes/editor.scene.ron`;
the editor adapter maps its template host/view aliases to live and retained identities.

## Verification and limitations

Runtime source-lifecycle regressions cover explicit path selection, single mounting,
retirement and rejected entry sources. Native consumers still require real input,
frames and clean shutdown on the changed revision. No compilation or static source
inspection grants native acceptance. External supervision remains necessary for a
hung loader, native callback, renderer or process.

This is explicit startup support, not a stable game SDK, automatic scene-attached
scripts or project Play. Other renderer profiles and platforms remain uncertified.
