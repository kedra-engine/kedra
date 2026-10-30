# Authoring asset admission

Source-matched native loading for authoring tools, independently of Inspector and
Scene View. `Delivery` compares an admitted SourceSnapshot with a verified manifest;
`Loader` admits exact native asset types and published identities before requesting
Bevy loading. It neither builds deliveries nor publishes source edits or Save.

Passive authoring previews use `Delivery::matches_resources`: project scope and delivered binary
resources must match, including indirect dependencies. Environment roots may use new or newer
validated source values with catalog identities; hosts apply them with `refresh_environment` before
materialization and after native load completion. The source text must match the admitted
hash/length and schema. Scene/prefab changes do not invalidate unchanged resources: authoring graphs
come from the admitted source workspace. `matches` retains full document matching for consumers
requiring a complete delivery match, including environment bytes. Each requested resource still
requires `accepts` for its type and identity.

`environments::install` wraps the existing project source reader before AssetPlugin.
Hosts publish the admitted source snapshot and clear it on project retirement. The
adapter serves validated environment data at its original logical path; other reads
use the original verified reader. This authoring adapter is not installed by games.

`compile_preview` admits resource-free graphs and source environments without a project delivery,
refusing binary asset requests in that mode. With a delivery it applies scope/resource matching
checks and native loader. It performs no hydration, gameplay execution or renderer acceptance.

The current profile supports byte-preserving images, static GLTF scenes, environments
and stylesheets. Imported model cameras/lights/animations stay disabled. A caller
must check the loader's retained error before accepting compilation. Processed-resource
provenance, arbitrary asset types and hostile-filesystem isolation are not implied.

The editor owns project-root lifetime and explicit delivery authorization. The core
authoring crate remains independent of these loaded GLTF/stylesheet dependencies.
See [ADR 0040](../../docs/decisions/0040-shared-authoring-and-preview-boundaries.md).
