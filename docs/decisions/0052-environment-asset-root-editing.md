# ADR 0052: Environment asset roots in Inspector

Status: implementation; owner review pending.

## Decision

Inspect existing `Environment` assets as reflected source roots. Asset Browser
selection already uses the scoped source catalog: Scene/Prefab assets resolve to
their composed document root; `.environment.ron` assets resolve to their existing
environment schema. Other imported assets do not acquire invented editable roots.
No new light-profile format, synthetic prefab or runtime entity is introduced.

Assets exposes **Create → Environment**. Creation uses the existing staged importer
and metadata initializer, selecting the asset after publication/admission; existing
files and sidecars are never overwritten. `kedra_import` depends on the existing
authoring filesystem crate to reuse its portable filename policy. No new publisher,
filename parser or metadata format is introduced. Import's per-file publication
limits also apply to creation. A missing profile appears as an **Environment** picker
on the scene root. Choosing a path adds `SceneEnvironment` in one ordinary source
transaction, with Undo/Redo and separate scene Save.

`kedra_authoring::environment::Edits` owns shared per-path working values, monotonic
revisions, bounded per-asset history (64 edits) and pinned Save requests. This belongs
in the existing authoring crate: it is independent of widgets and native files, but
shares the source inventory and commit protocol. The editor owns scoped selection,
field settlement, visibility and project-retirement authorization. `kedra_inspector`
continues to supply the ordinary reflected numeric/color controls. Schema version
and value validation remain owned by `kedra_scene::environment::Environment`.

The admitted source snapshot retains bounded environment text beside its existing
asset identity/hash/length record. Inspector reads this snapshot, never filesystem
bytes on the UI thread. Native text acceptance validates an isolated candidate
before acknowledgment and publication. A dirty asset remains distinct from both the
active scene and its published file. Save targets the selected environment asset;
source Undo/Redo follows that selection. Discard and project-close settlement include
environment edits. Runtime deliveries and game state never own Save.

`kedra_authoring_fs::commit_environment` reuses the existing source lease,
inventory checks, sidecar pinning, journal, exact retry and explicit recovery. It
only replaces one existing environment, checks its admitted identity under the
writer lease and preserves the sidecar. It cannot write a catalog or create/delete
an asset. Creation/import and binary processing remain separate responsibilities.

## Preview boundary

Environment data is a validated, bounded authored root. Editor preview admission
accepts new and changed environments from the admitted source catalog with exact
validated source bytes and persistent identities. The authoring-only
`kedra_authoring_assets::environments` adapter wraps the existing native source reader
before AssetPlugin construction. It serves bounded environment bytes and loader
metadata from the published source snapshot at the original source-qualified path.
Other requests retain the verified reader; no parallel handle/identity registry or
filesystem watcher is introduced. Hosts publish admission and clear it on project
retirement. Full runtime delivery matching remains strict.
`kedra_authoring_assets::refresh_environment`
applies accepted environment values to already loaded preview assets before scene
materialization, including after native loading completes. It resolves the existing
source-qualified AssetServer path rather than adding an identity registry. Scene
cameras refresh their cached environment; editor inspection lighting remains
independent. This does not rebuild or modify the published runtime delivery.

## Verification boundaries

Authoring/filesystem contracts cover invalid values, stale field revisions,
unchanged files before Save, Undo/Redo, exact receipts, external edits and sidecar
identity conflicts. Resource admission tests cover valid source revisions and
refusal of mismatched bytes/identities. The `environment-editing` native scenario
selects the asset, edits numeric/color fields, refuses invalid brightness, saves
without changing identity and exercises retained Scene View lighting and Undo.
Native evidence is generated under `target/test`; it is not established by compilation.
