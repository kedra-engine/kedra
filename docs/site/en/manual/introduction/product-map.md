# Product map: editor, runtime and tools

Kedra's editor, runtime and tools work with the same content, but each has a different role.

## The editor: a workspace for inspecting content

The native editor manages project opening and a dockable workspace with floating windows.
Asset Browser navigates source folders and opens supported content. Hierarchy shows the
active document's composed nodes, Inspector presents values for the selected source node,
and Scene View renders supported content when its resource prerequisites are met.

These panels coordinate around a source document session. Browser selection is not the
same as opening a document, and source-node selection is not a persistent gameplay entity.
The editor currently inspects documents read-only; its own authored UI does not mean that
users can visually edit that UI or install project-defined editor windows.

## Runtime: services inside an application

Here, **runtime** means the shared code that an application uses while it runs: asset
loading, scene composition and instantiation, supported rendering integration, UI and
explicitly enabled scripting or application services.

A Rust host chooses which services to install. It may select an explicit startup scene
from a loaded source catalog and mount it as ordinary Bevy entities. Startup completion
means the scene was instantiated, not that a GPU frame is ready or gameplay was authorized.
The runtime does not need an installed editor or the source-writing tools.

This is useful application infrastructure, not yet a complete game SDK with a ready-made
project-to-Play integration.

## Authoring: managing editable intent

**Authoring** is creating and changing source content rather than changing a running
simulation. Kedra's Rust authoring APIs manage working documents, validation, revisions,
history and Save requests. A host integrates the actual persistence operation and reports
its result through the protocol.

For example, changing an instance override in a working document is a source operation;
moving a live entity is not. The [source-editing guide](../scenes/source-editing.md)
explains these APIs. They do not currently appear as visual editing or document Save in
the editor.

## Import and tools: preparing content and applications

Asset processing turns admitted sources into a checked, immutable **delivery**: the
resources and metadata a consumer loads. A delivery is derived output, not the place to
make the next source edit. The editor does not silently rebuild it when a document opens.

Separate command-line application tools use Cargo and native packaging tools to build,
verify and package configured Rust applications. Only explicit `run` launches an
application; building is not editor Play. Unlike opening a project in the editor, building
a Cargo project executes its build scripts and requires trusting that project.

## Follow one piece of content

```text
Authored sources ── processing + verification ──> delivery
                                                  │
                                      runtime loaders + source catalog
                                                  │
                                      composed scene → live instance

Authored sources ── validated editor session ──> read-only editor projection
                                                  ↑
                                     matching delivery for rendering
```

The source catalog identifies the scene/prefab sources admitted for runtime composition;
it is not the Asset Browser's folder listing. A loaded scene resolves composition and
registered component values before instantiation. The editor separately retains accepted
source content for inspection and builds a presentation from it.

This explains a common situation: a document can be valid enough to inspect while its
preview cannot render. Missing or mismatched delivery resources are a separate problem
from invalid source content. See [Asset processing](../assets/pipeline.md) for diagnosis.

To try this route, follow [Getting started](../getting-started/index.md).
For deeper concepts, choose [Projects](../projects/index.md) to organize content,
[Assets](../assets/index.md) to prepare it, or [Scenes and prefabs](../scenes/index.md)
to understand composition and instances. Use [Current capabilities](capabilities.md)
to distinguish these working foundations from the planned editing loop.
