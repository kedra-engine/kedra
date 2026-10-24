# Native editor startup contract

Project startup coordinates asynchronous preparation and native surface lifetime.
For commands and controls, use [Launch the editor][launch]
and [Create or open a project](../../manual/projects/create-open.md). Startup does not scaffold a
game, build or execute project code, or offer a normal editor session without a validated project.

## Responsibility and authority

| Owner | Responsibility |
| --- | --- |
| `apps/kedra` | CLI parsing, delivery location and plugin composition |
| `kedra_import` / `kedra_assets` | Processing/publication and verified runtime delivery, respectively |
| `kedra_editor` | Startup model, semantic requests, jobs and authored view bindings |
| `kedra_project` | Source manifests, validation/creation and separate recent-project preferences |
| `kedra_ui` / `kedra_scene` | Shared native controls and authored scene/prefab views |
| `kedra_desktop` | Owned window, targeted camera and UI root; native chrome adapter |

`StartupModel` and entry drafts are ordinary Rust values. Bevy resources/states adapt them to
scheduling; entity handles belong to presentation adapters, not domain authority. Requests and
asynchronous completions carry generations. `begin` accepts one request in Projects; repeated
activation cannot replace it while loading. `complete` accepts only a matching, still-working
generation. A stale or duplicate result cannot activate another project. Retry gives a failed
read-only Open a new generation. A Create preparation error stays in Loading until the user
selects Back to Projects. Another creation attempt requires opening and submitting a new form;
the writer is not replayed automatically.

Normal project startup and debug explicit-scene startup are separate authorities. An explicit
scene cannot run concurrently with project/project-delivery arguments; it does not select a
different set of editor plugins or authorize project operations. See the
[editor crate contract (English)][editor-crate] for that diagnostic lifecycle.

Asset processing is not an editor-only service: a host can install `KedraAssetsPlugin` with a
verified source without depending on editor, importer, window or filesystem authoring-writer
crates. The runtime host still owns native loaders and source registration. The
[asset pipeline](../../manual/assets/pipeline.md) and [asset crate contract (English)][asset-crate]
own delivery details; hashes detect changed bytes, not publisher authenticity.

## Windowless boot and readiness

`run` first reads the bounded editor manifest with profile `editor-desktop`. Complete delivery
verification then runs on `IoTaskPool`, concurrently with loading
`editor://scenes/startup.prefabs.ron`. Verification reopens the delivery and rejects a changed
manifest as well as missing/corrupt members. Boot creates no splash window and requests no
startup renderer readback. Catalog readiness and complete delivery verification both gate history
loading. History/CLI-intent resolution precedes the first Projects or project-preparation surface.

Missing configuration, integrity/catalog failure or boot work still unfinished beyond 30 seconds
reports diagnostics and exits without substitute UI. This terminal authored-resource failure is
different from a project validation failure, which retains recovery controls.

Project source readiness, native window creation, layout hydration and OS presentation are
distinct milestones. Bevy/GPU initialization still precedes the first useful window. Neither a
ready source nor a successful headless lifecycle test proves that a native frame was presented.

Fast preparation enters the editor without waiting for an intermediate loading window. Work
still unfinished at 150 ms may show a 640×360 logical-pixel loading surface. A failure bypasses
that display threshold and shows recovery immediately. The threshold delays neither the task nor
successful completion. `finish_loading` advances a ready validated project independently of
loading-window creation. See [ADR 0021 (English)][windowless] for the architectural decision.

## Surface lifetime and authored presentation

Each desktop surface owns its window, targeted camera and root, including pending authored view
requests parented below it. Replacement retires only the former ownership; unrelated surfaces
and project data are not removed. A stale close event for a retired window does not exit its
replacement. Floating editor hosts have independent ownership; the
[docking contract (English)][docking] owns their integration.

Native scene/prefab references use full source-qualified logical paths, not hardcoded UUIDs.
`views::compiled` requires an already loaded native catalog member, its materialized document,
agreement with the published source revision and a published compilation. Persistent source,
node, instance and sidecar UUIDs remain identities; delivery UUID lookup support does not relax
the native-code path rule. Pending views retire with their parent and cannot attach to a stale one.

External scenes/prefabs define loading, Projects, modals, recent rows, captions and workspace
structure. Flair styles those views; native Bevy widgets supply text editing, clipboard and modal
navigation. Panel behavior and current read-only limits belong to the
[editor Manual](../../manual/editor/index.md), not to the startup state machine.

Loading and Projects use the compact owner-supplied T3 mark; startup does not load the large
release artwork. Failed preparation uses the error icon, retains log/back and permits Retry for
Open, not Create. The spinner turns linearly once per second. `KEDRA_REDUCED_MOTION=1` keeps it
static; automatic native motion-preference detection is not implemented. Exact artwork identities,
font notices and derivation remain in [asset provenance (English)][provenance].

The client frame has no painted perimeter stroke. The custom 32px row is the only painted
caption. On macOS the native window retains AppKit's titled/resizable structure with hidden
native title, visible system buttons and full-client content; this avoids Winit frame churn from
truly borderless maximize queries. Platform windowing still owns move, resize, minimize, maximize
and close. The [window-chrome contract (English)][chrome] owns header/input integration, not
startup; [desktop (English)][desktop] owns native surface lifetime.

## Source publication and project failures

`kedra_project::create` reserves `parent/name` with directory creation, creates empty `assets/`
and publishes `kedra.toml` last. Any existing destination, including an empty directory or symlink,
is rejected. Preflight is not a reservation. Manifest bytes are written to a temporary file in
the destination, synchronized and published with `persist_noclobber`; no fallible operation
follows successful manifest publication. This is not a transaction over the whole directory or
a promise of crash-durable directory entries. Failure cleanup removes only owned empty
directories, never unrelated content recursively. Interruption can leave an invalid directory;
it is not silently reused. Cancellation does not promise rollback of published files.

Open is read-only: bounded UTF-8 TOML (at most 64 KiB), schema, non-nil identity, source format and
contained existing asset directory must validate before a `Project` is returned. Only format 1
exists. Source format, not informational `editor_version`, determines compatibility. Open never
executes code, rewrites sources or migrates implicitly. Project manifests and history must be
regular files; symlinks to regular files remain supported. Static special files fail before
opening. Portable preflight is not an OS sandbox against concurrent path replacement.

Recent projects are bounded local preferences, separate from sources: at most 20 entries and
64 KiB of UTF-8 TOML input. Only successful preparation is remembered. Corrupted/unsupported
history is reported and preserved; the editor disables history writes for that session. History
saving uses a synchronized temporary file and atomic replacement, not a multi-writer merge:
concurrent writers currently have last-writer-wins behavior. A history-write failure warns but
does not turn successful project preparation into failure. User recovery and moved/missing rows
are described in [Recent projects](../../manual/projects/create-open.md#recent-projects).

## Verification boundaries and remaining acceptance

The owning source/CPU tests cover generation rejection, preflight versus creation, history
corruption/nonfatal write errors, windowless verification and catalog gates, exact loading
threshold behavior, parent-owned pending views and stale-close rejection. They do not establish
native interaction or pixel acceptance. See [Tests and review](../tests-and-review.md) and the
[testing ownership map (English)][testing] before changing or adding a test.

[Startup geometry (English)][geometry] pins independent visual expectations.
[Automation (English)][automation] owns optional native input/capture scenarios, including DPR
overrides 1/2. Their Bevy input injection and renderer captures do not certify physical OS input
or other platforms. Full native acceptance of editor startup remains unverified;
startup has no active screenshot goldens or pixel-comparison suite.

Remaining acceptance includes deterministic startup fixture data, full control states and
selected/missing recent-row presentation, physical picker/chrome/IME behavior, native DPI/monitor
transitions, capture reliability across environments and pixel equivalence on supported platforms.
Font subsets do not guarantee every Unicode script. Invalid frames fail rather than being
replaced with synthetic pixels. Future product work remains in the [roadmap (English)][roadmap].

[launch]: ../../manual/getting-started/launch-editor.md
[editor-crate]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/README.md
[asset-crate]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_assets/README.md
[desktop]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_desktop/README.md
[windowless]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0021-windowless-startup.md
[docking]: ui/docking.md
[chrome]: ui/window-chrome.md
[provenance]: https://github.com/kedra-engine/kedra/blob/main/assets/README.md
[geometry]: ui/startup-visual.md
[automation]: automation.md
[testing]: testing.md
[roadmap]: https://github.com/kedra-engine/kedra/blob/main/docs/roadmap.md
