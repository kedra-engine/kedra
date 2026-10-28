# Authoring asset admission

Source-matched native loading for authoring tools, independently of Inspector and
Scene View. `Delivery` compares an admitted SourceSnapshot with a verified manifest;
`Loader` admits exact native asset types and published identities before requesting
Bevy loading. It neither builds deliveries nor publishes source edits or Save.

The current profile supports byte-preserving images, static GLTF scenes, environments
and stylesheets. Imported model cameras/lights/animations stay disabled. A caller
must check the loader's retained error before accepting compilation. Processed-resource
provenance, arbitrary asset types and hostile-filesystem isolation are not implied.

The editor owns project-root lifetime and explicit delivery authorization. The core
authoring crate remains independent of these loaded GLTF/stylesheet dependencies.
See [ADR 0040](../../docs/decisions/0040-shared-authoring-and-preview-boundaries.md).
