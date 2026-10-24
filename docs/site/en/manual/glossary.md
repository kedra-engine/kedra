# Glossary

Terms used across the manual, grouped by what they describe. English is canonical;
API identifiers keep the same spelling in both languages. Follow each link for details.

## Content, paths and identity

### Project

A directory of authoring sources identified by `kedra.toml`, with a project UUID, name
and asset directory. Creating one does not create a Cargo application or runnable game.
See [Projects](projects/index.md).

### Cargo application

An ordinary Rust executable configured through `Cargo.toml`; standalone Kedra builds
also require explicit distribution metadata. It is distinct from an editor project.
See [Application configuration](build-and-distribution/configuration.md).

### Source asset

An authored file retained with its adjacent `.meta` sidecar, rather than processed output
or a loaded resource. Source files and processing settings are the inputs to a delivery.
See [Assets](assets/index.md).

### Sidecar

The adjacent metadata file, such as `checker.png.meta`, holding an asset UUID and native
load, process or ignore settings. Keep it with the source; it is not a disposable cache.
See [Asset identity](assets/index.md).

### Asset delivery

A separately built directory of processed resources, loader metadata and a delivery
manifest. It is derived output for consumers, not a source-editing directory.
See [Processing and verification](assets/pipeline.md).

### Delivery manifest

`kedra-assets.json`, which records delivered asset identities, paths, loaders and member
digests. It is neither the project manifest `kedra.toml` nor a source catalog.
See [Delivery contents and verification](assets/pipeline.md).

### Source catalog

The admitted source set used to resolve documents and their identities; path-based native
lookups must use loaded catalog membership and the published revision. A native
`.prefabs.ron` catalog lists explicit references, not a directory scan or delivery manifest.
See [Paths and identity](projects/paths-and-identity.md) and [Loading](scenes/lifecycle.md).

### Logical path

A source-qualified asset reference such as `game://Hud.prefab.ron`. It names a member
of a supplied source namespace, not an absolute filesystem location or a URL to fetch.
See [Paths and identity](projects/paths-and-identity.md).

### Native filesystem path

A locator used by the operating system to access a file or directory. It is distinct from
the asset's logical path and from the relative label shown inside the editor.
See [Path forms](projects/paths-and-identity.md).

### Display path

A readable asset name or relative path shown in the project UI, rather than an absolute
filesystem locator. Formatting a label does not change the stored locator or identity.
See [Path forms](projects/paths-and-identity.md).

### Persistent identity

A UUID retained in authored data for a project, asset, document, node or nested composition
site. A live Bevy `Entity` or asset handle is not a replacement saved identity.
See [Identity](projects/paths-and-identity.md).

### Scoped node address

A source node UUID together with its outer-to-inner nested-instance identities. The scope
distinguishes repeated uses of the same prefab node; names are only labels.
See [Document identities and references](scenes/index.md).

## Source composition and lifecycle

### Scene

A source document describing a top-level authored hierarchy, suitable for an explicit
application entry point. It may contain prefab instances but cannot be nested as a prefab.
See [Scenes and prefabs](scenes/index.md).

### Prefab

A reusable source hierarchy that another document can include. Like a scene, it stores
authored data, not a running Bevy `World`.
See [Scenes and prefabs](scenes/index.md).

### Composition

Resolving bases, nested prefabs and ordered overrides into an effective source hierarchy
while retaining provenance. Composition alone does not create live entities.
See [Composition and overrides](scenes/composition.md).

### Typed compilation

Validating composed component data, versions, overrides and references in an isolated
source `World`. The resulting `CompiledPrefab` is immutable, not a live instance.
See [Rust component integration](programming/rust-components.md).

### Instantiation

Creating an owned graph of ordinary Bevy entities from a compilation, with checked parent
and external bindings. It is separate from presentation hydration and script authorization.
See [Loading and lifecycle](scenes/lifecycle.md).

### Runtime instance

One live materialization retaining its exact compilation and source-address-to-entity map
through `PrefabInstance`. Live changes do not edit sources or sibling instances, and new
source publication does not automatically replace it.
See [Instance ownership](scenes/lifecycle.md).

### Variant

A document derived from a `base` of the same kind, with its own document identity and the
inherited root-node identity. It specializes inherited content without copying the base.
See [Variants and nesting](scenes/composition.md).

### Override

An authored operation layered over composed content, such as a field replacement or
structural change. A nesting-site override does not edit the referenced prefab or sibling
sites; it is not an arbitrary live ECS mutation.
See [Composition and overrides](scenes/composition.md).

### Port

A declared external reference supplied by a containing document or runtime caller.
Binding a target does not make that target an owned child of the instance.
See [Port bindings](scenes/composition.md).

### Source revision

A published, validated source set and its immutable compiled entries, represented by
`SourceRevision`. Its publication counter is distinct from per-document working revisions,
filesystem timestamps and the build inventory's Git `source_revision`.
See [Source publication](scenes/lifecycle.md) and [Save revisions](scenes/source-editing.md).

### Working set

The authoring `WorkingSet`: working documents, revision guards and history, separate from
the acknowledged source baseline. Its dirty state is not a difference against gameplay.
See [Source editing and Save](scenes/source-editing.md).

### Save receipt

A host writer's correlated `Receipt` for the exact committed source operation and bytes.
Only a matching acknowledgment advances the baseline; a prepared `CommitRequest` has not
written anything, and a receipt does not mark newer edits saved.
See [Save protocol](scenes/source-editing.md).

## Hosts, presentation and readiness

### Host

The native Rust application constructing the Bevy `App`. It chooses plugins, source
catalogs, component policies, hydration and execution authority; a document cannot grant
itself these capabilities.
See [Programming](programming/index.md).

### Hydration

Turning inert source data into native presentation, for example `UiLayout` into Bevy
`Node`. It is not a source edit, script execution or proof of a rendered frame.
See [Authored UI](rendering-ui/authored-ui.md).

### Authored UI

A retained UI tree described by scene/prefab source components. A serving host hydrates
controls and binds application actions; source data alone does not install a live interface.
See [Authored UI](rendering-ui/authored-ui.md).

### Passive UI

An isolated presentation of authored UI without serving observation identities or native
button, text-input, autofocus and modal behavior. Its visual appearance does not authorize
interaction or project scripts.
See [Passive previews](rendering-ui/authored-ui.md).

### Readiness

A named lifecycle milestone, not a universal success flag: `PrefabReady` marks a complete
entity graph, `UiReady` UI hydration, and `PrefabRenderReady` CPU render-asset materialization.
None establishes a GPU frame or physical input acceptance.
See [Visible readiness](scenes/lifecycle.md).

### Play

The planned in-application workflow for starting execution, stopping and returning to
editing with an explicit unsaved-source policy. It is not implemented; Scene View and the
standalone CLI's `run` are not Play.
See [Current limitations](introduction/capabilities.md).

## Scripts and authority

### Behavior

A host-authorized Luau program with persistent per-instance `self` and non-yielding
lifecycle callbacks. Each run has its own VM; opening a project does not attach Behaviors.
See [Luau programs](programming/luau.md).

### Sequence

A finite host-authorized Luau operation using `run(ctx, args)`, which may wait and return
owned data. Schedule waits count executions or clocks, not rendered frames.
See [Luau programs and waits](programming/luau.md).

### Module

Shared Luau code imported from explicitly admitted sources, with a module cache per VM.
Importing it does not start a program or grant additional permissions.
See [Luau modules](programming/luau.md).

### Admitted script sources

Additional modules and applicable `.luaurc` configuration explicitly admitted through
`Program::sources`. The host supplies entry code separately through `Program::path` and
`Program::source`; imports do not search the ambient disk.
See [Module admission](programming/luau.md).

### Capability

A narrow operation the host exposes to a script, such as changing an owned target's local
position. It is not generic script access to the `World`, and type declarations grant none.
See [Lifetime and capabilities](programming/lifetime-and-capabilities.md).

### Scope

The Rust `Scope` guarding access for one authorized live script run. `with_world`
reacquires access during dispatch and returns owned data; borrows cannot cross a yield.
See [Scoped bindings](programming/lifetime-and-capabilities.md).

### Revocation

Ending a script owner's authority to perform further effects, for example on cancellation
or despawn. It is not rollback: effects already applied remain applied.
See [Revocation and lifetime](programming/lifetime-and-capabilities.md).

## Builds and verification

### Runnable assembly

A compiled executable with its verified delivery and notices in a complete runnable
directory or native bundle. `build` creates it without launching the application.
See [Build and run](build-and-distribution/build-and-run.md).

### Distribution package

A native installer, disk image or portable archive created and inspected by `package`
alongside a release assembly. Package creation is not publication or platform certification.
See [Packaging and licenses](build-and-distribution/packaging-and-licenses.md).

### Build handoff

The successful build's `build-result.json`, recording the selected application, target and
relative runnable/resource paths. It is output data, not project configuration or an input
for skipping a new compilation with `run`.
See [Build results](build-and-distribution/build-and-run.md).

### Launch dispatch

The operating system accepting an explicit launch request for a freshly built application.
It does not prove application readiness, a successful exit, a frame or working input.
See [Explicit launch](build-and-distribution/build-and-run.md).

### Integrity

Agreement between expected and actual bytes or structure, checked through hashes and
applicable package/signature verification. Checksums do not authenticate a publisher;
macOS ad-hoc signing does not establish operating-system trust.
See [Package trust](build-and-distribution/packaging-and-licenses.md).

### Native acceptance

Verification of the intended application behavior with real native input and rendered
frames. Compilation, headless tests, readiness flags and launch dispatch prove different
contracts and do not replace it.
See [Verification boundaries](introduction/capabilities.md).
