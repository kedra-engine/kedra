# ADR 0040: Shared authoring and passive preview boundaries

Status: implemented extraction; owner review pending.

## Decision

Authoring tools consume the same `kedra_authoring` working authority, typed commands,
property/history preparation and immutable inspection. `editing::SourceNode` is a
native source type, not a Luau-owned type. `source` compiles against the admitted
symbolic resource inventory without filesystem access or runtime asset loading.
`changes` returns matched working/compiled candidates; the editor owns native-input
acknowledgment, source-session lifetime authorization and synchronous publication.
No second source journal, ECS, scheduler or reflection system is introduced.

`kedra_authoring_fs::source` owns bounded filesystem snapshots/rechecks under the
existing Save admission lease. It never writes during Open. The host selects the
namespace. The implementation retains WalkDir, RON, native metadata and existing
content hashes rather than adding a custom walker, parser or source identity index.

`kedra_assets::references` owns Bevy-reflected Handle/Option inspection and exact
native reconstruction. It knows no Inspector, scene documents or Save. Picker UI,
icons and image-density insertion remain UI/domain adapters, not asset primitives.

`kedra_authoring_assets` is an authoring-layer native resource-admission profile over
the existing source snapshot and verified delivery. It owns type/loader/identity/hash
matching and source-limited loading, without windows or source publication. Its
consumers are resource assignment and Scene View. Keeping this independently usable
profile separate avoids pulling GLTF/stylesheet loading into basic source editing.
It reuses Bevy loaders and the existing manifest, not another asset registry.

`kedra_preview` is an editor-layer passive static-model mechanism. It owns audited
native materialization/remapping, owned forest retirement, bounded mesh measurement
and native GLTF subasset enumeration. Scene View and Inspector consume it directly.
Native Bevy WorldAsset/DynamicWorld remains the cloning/remapping mechanism; no
custom importer or graph engine is introduced. Viewport navigation, studio lighting,
selection overlays, frame/readback lifecycle and mixed UI/spatial refresh orchestration
remain in Scene View. Passive UI refresh remains in `kedra_ui`.

The editor's settlement module centralizes queries of tool-owned drafts for Save,
Undo/Redo, scripts and Close. It is host coordination, not a generic transaction
manager. Each tool retains its own drafts; accepted source values remain authoring-owned.

## Bounds

Extraction does not add gameplay model activation, skins, animations, managed asset
moves, new-document Save, implicit project execution or automatic source reload.
Static preview remains intentionally narrower than gameplay instantiation. Public
prepared candidates require the host to recheck its scope/revisions before publication.

## Verification ownership

Native source-handle tests belong to authoring; source filesystem tests to
`kedra_authoring_fs::source`; handle reconstruction contracts to `kedra_assets`;
subasset retention tests to `kedra_preview::catalog`. Editor tests retain assembled
source publication, panel lifetime, viewport refresh and native-input/frame contracts.
Native acceptance must be rerun after extraction; earlier passes are not current evidence.
