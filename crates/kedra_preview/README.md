# Passive static preview

Editor-layer owned model materialization, finite native mesh bounds and real GLTF
subasset enumeration. Scene View and Inspector are consumers, not mechanism owners.
No editor shell, source writes, authoring transactions or gameplay callbacks are installed.

`models::mount` uses Bevy DynamicWorld/WorldAsset cloning and entity remapping, with
an audited visual-component profile and caller-owned node budget. Retire the returned
Instance explicitly. `bounds::measure` has a measurement-local cache, bounded traversal
and finite viewing limits. `catalog::choices` retains metadata on an explicit ECS owner
and reports native scene labels; it never invents Scene0 or grants assignment rights.

Skins, morphs and imported cameras/lights are rejected. This is not a gameplay model
runtime. Navigation, frame readiness, overlays and camera lighting remain host-owned;
passive UI hydration/refresh remains in `kedra_ui`.
See [ADR 0040](../../docs/decisions/0040-shared-authoring-and-preview-boundaries.md).
