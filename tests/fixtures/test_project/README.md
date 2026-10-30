# Shared native test project

This is the single checked source project for project-based native editor scenarios.
It is an ordinary Kedra project, usable in the normal editor without a test loader.
Scenes, prefabs, models, textures, styles, fonts and stable asset sidecars are
versioned here. Native test startup does not generate project content or new IDs.

## Automated runs

`cargo xtask test -- features` makes a complete fresh working copy for each project
scenario at `target/test/<run>/private/<scenario>/project`. It builds one matching
`game` delivery from another clean copy, shared by the sessions in that run. Editor
preferences, credentials, frames and reports live under the run directory, never
in this canonical source tree. Docking restart opens the same scenario copy.

Browser/Hierarchy failure checks deliberately modify or move files in their own
copies. A crash can leave a dirty copy for diagnosis, but cannot contaminate the
next scenario or this project. The runner verifies copied bytes and records the
canonical source digest/non-mutation result. Copies preserve document, node,
nested-instance, project and asset IDs; do not initialize metadata on every run.

The startup scenario still executes Create to test project creation itself. That
new project is an explicit tested effect in its private lifecycle directory, not
generated setup data. Its ordinary Open/reopen checks use the shared project copy.
The separate project-independent docking, reflected-value and button/field hosts
remain component-isolation checks; they do not create substitute test projects.

## Contents

- Root scenes, `Cuboid.prefab.ron`, `Hud.prefab.ron`, `checker.png` and `ship.glb`:
  spatial/UI/mixed rendering, environments, scene navigation and selection.
  The Hud image has an authored density list and an initially empty shadow list
  for source-owned compound Inspector editing without changing its initial appearance.
- `marker.glb`: single magenta replacement model reusing the Kedra-authored geometry
  from `ship.glb`; exercises model assignment, retained cameras and source Undo/Redo.
- `Hierarchy/`: a scene with two instances of a 180-leaf prefab, explicit child
  order, Unicode search and a second document. Stable expectations are in
  `cases.json`, not runtime-generated UUIDs.
- `Inspector/`: nested source overrides, repeated names, inherited Unicode
  captions and an independent second source.
- `Prefabs/`: valid Browser admission documents. Failure
  checks corrupt only a working copy, then restore its original bytes.
- `Straße.txt`, `z000.txt`–`z063.txt`, `tree-00/`–`tree-23/`: real filesystem data
  for Unicode search, list/folder virtualization and keyboard reveal.
  Numbered browser-only text sources have native `Ignore` metadata: they remain
  visible as source files without bloating the bounded game-delivery manifest.
  Browser runs at 1120 × 640 logical pixels, requires folder overflow and checks
  that grid/list tails are initially unmounted, End reveals the last source and
  Home returns to the first. Counts and tail names come from this checked tree.
  `hidden.meta` deliberately has no source partner: Browser must never list a
  sidecar as an independent source.
- `Empty/`: a folder with a Git placeholder.

Geometry and textures are generated Kedra-authored data, with no private or external
game assets. The included Inter font retains `Fixture-Inter-OFL.txt`. The checked
GLB is the source for both native scenarios and the model-geometry contract check;
no parallel model/project generator is retained. Updating checked sources/sidecars
is an intentional fixture change for review.

## Open manually

Build deliveries into new output directories and use isolated editor preferences:

```sh
cargo run -p kedra -- assets build assets/editor target/test/editor-delivery --namespace editor
cargo run -p kedra -- assets build tests/fixtures/test_project/assets target/test/game-delivery --namespace game
cargo run -p kedra -- tests/fixtures/test_project --assets target/test/editor-delivery --project-assets target/test/game-delivery --state-directory target/test/manual-state
```

For edits, first copy the complete project into a new directory and open/build that
copy instead. Keep the canonical project unchanged during testing. Source opening
does not authorize game execution or implement Play. Native input/frame acceptance
still requires an unlocked desktop; headless checks cannot establish it.

## Ordinary native game

This is also an ordinary runnable Cargo game project: package/bin
`kedra-test-project` in `Cargo.toml` and `src/` depends only on runtime crates, with
no editor/importer/test-tool dependency.
The native game enables `kedra_ui/render` and uses AuthoredUiPlugin, so it consumes
the same contained-clip render optimization as the editor without importing editor
code or its Winit pacing policy. Headless schema/asset consumers do not need this
feature. Shared implementation does not imply identical performance gains for
different scenes; native game verification remains separate from editor results.

There is no separate native consumer
workspace or per-scenario Cargo target. `assets/Native/` contains its remote/switch
scenes with preserved identities; `scripts/Native/` owns its finite Luau programs.
The game-owned switch Behavior receives validated parameters, subscribes to real
Bevy Activate and starts its UI-only child Sequence through `kedra_script_bevy`.
Rust authorizes these embedded sources explicitly; it does not implement activation
or discover scripts from scene content. Release retains the same game-facing behavior.
`catalogs/native.prefabs.ron` combines separately verified game and editor sources
outside the imported asset tree, so standalone game-delivery verification never
requires an editor source. The game embeds that catalogue and authorized programs.
`native.code-workspace` scopes its API declarations to those canonical scripts.
The deliberate `hidden.meta` negative case, live Hierarchy/Inspector compositions
and representative rendering assets remain. Project inventory changes are recorded
by native runs, not approved through production screenshot-policy hashes or tolerances.
The ordinary executable has one fixed native capability set and requires explicit
verified editor/game deliveries; `--scene` selects composition, not plugins or
authority. Both native scenario validators use complete per-scenario working copies
and independently require two clean process lifecycles, actual state/input and
readback/retention evidence. See [native game usage](native.md) for materialization,
delivery, launch and external verification commands. Luau API-isolation regressions
live in `kedra_script` product tests rather than a separate authored project.

## Explicit editor-script fixture

`Editor/rename-pair.luau` changes two existing prefab names through one explicitly
granted transaction. It never runs on Open and never saves files. The native
`editor-scripts` scenario authorizes it, exercises common Undo/Redo and native Save,
and checks both saved members while preserving other files and sidecars.
