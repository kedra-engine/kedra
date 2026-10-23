# ADR 0005: Authored scenes and prefabs

Status: runtime and source-editing contract in use. A filesystem authoring writer,
editable scene feature and complete native platform acceptance are not implemented.

## Decision

Use ordinary Bevy assets, reflection, DynamicWorld instantiation and native UI.
Stable document/node/instance UUIDs and scoped addresses are separate from live
generational entities and asset handles. There is one canonical versioned RON
format for scenes/prefabs, with base variants, nested instances, ordered children,
field/component overrides and symbolic local/port references.

| Package | Responsibility |
| --- | --- |
| `kedra_scene` | Validated composition, component policy, compilation, immutable snapshots, instantiation and native loaders |
| `kedra_ui` | Hydration into native Bevy layout/widgets, retained bindings and Flair styling |
| `kedra_authoring` | Mutable sources, guarded transactions/history, Apply/Revert, scoped Unpack and Save request/receipt protocol |
| `kedra_import` | Identity metadata, processing and installed-delivery verification |
| Desktop/editor host | Window/camera ownership, source selection, feature models and native lifecycle |

Runtime dependencies exclude editor, importer and filesystem authoring writers.
Live gameplay state, working authoring sources and published compiled revisions
are different state owners. Source Save never serializes a gameplay World.

## Invariants

- Schema/component registration uses Bevy's TypeRegistry with exact keys,
  versions and validators. Side-effectful lifecycle hooks and unregistered
  requirements are rejected in the source world.
- Composition and decoding occur in isolation. Publication is atomic; invalid
  candidates leave the prior immutable revision unchanged and report failure.
- Instances retain their exact compilation and own their node mapping. Updating
  published sources does not mutate existing instances.
- Parents and external ports are preflighted. A failed partial DynamicWorld write
  reclaims only owned allocations; external bindings and other instances survive.
- Source loaded, compiled, dependency-ready, hydrated and renderer-frame-ready
  are distinct milestones. None silently implies the next.
- Documents/catalogs/graphs are bounded. Duplicate/nil identities, unknown fields
  or versions, cycles, dangling references and invalid patches fail explicitly.
- The explicit catalog contains qualified asset references, not a runtime directory
  scanner. Readable paths and UUID addresses use the same manifest-verified source.
  Document UUID equals sidecar UUID. Paths require explicit rename updates.
- UI structure is authored data. Features bind content, input and typed actions;
  source strings cannot execute code, arbitrary RPC or ECS mutation.
- Failed assets do not select a manual layout, older delivery, embedded resource
  or synthetic default. Optional input is an explicit source contract, not a
  recovery shortcut.
- Working-set edits carry revision preconditions and candidate validation. Failed
  edits do not replace sources or consume history. Undo/redo and complete scoped
  Unpack preserve unrelated links and expose selection mappings.
- Save requests retain exact source baselines/read conditions and write bytes.
  Only a matching receipt advances the saved baseline, never a newer draft.
  Host writers own filesystem compare-and-swap, durability and publication.

## Reuse and alternatives

Bevy owns reflection, layout, schedules, asset loading and entity remapping. RON
RawValue/parser/serializer owns typed values; `serde_with` rejects duplicate maps;
`serde_ignored` rejects ignored native fields while retaining Bevy serde defaults.
`futures-lite` bounds async reads. These are maintained, existing ecosystem tools.

A proprietary layout language, custom formatter/parser, compatibility forwarding
crates, parallel catalog scanner or runtime-to-writer feature boundary would
duplicate those mechanisms or weaken ownership. They are not part of this contract.

## Consumers and verification

Startup forms, rows, captions and dock chrome use the shared source path.
Independent game-style tests load installed catalogs and instantiate components
without source-tree access. Tests cover composition, source revision pinning,
references, source transactions, partial failures and retained native UI ownership.
Required commands and exact limits are in [authored scenes](../scenes.md).

Native input/frame checks, source-free installed consumers, dependency policy and
[third-party license obligations](../third-party.md) are separate requirements.
Owner-authored prototype provenance and project-license authorization are recorded
in [the source provenance statement](../third-party.md#source-provenance-and-publication).
