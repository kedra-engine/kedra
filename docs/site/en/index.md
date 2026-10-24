# Kedra documentation

## Start reading

Learn Kedra's project model, try the editor, or integrate its services into a Bevy
application. The documentation is available in English and Russian, with language
switching and local full-text search.
Start with [What is Kedra?](manual/introduction/index.md), then follow
[Getting started](manual/getting-started/index.md) for prerequisites and source setup,
[prepare resources and launch](manual/getting-started/launch-editor.md), and
[open and inspect your first document](manual/getting-started/first-document.md).
Use the [Editor](manual/editor/index.md) chapter for workspace and panel controls.

For deeper concepts, read [Projects](manual/projects/index.md) for identity and paths,
[Assets](manual/assets/index.md) for sources and deliveries, and
[Scenes and prefabs](manual/scenes/index.md) for documents and composition.
The twelve manual sections cover the available tools, not a complete game-creation workflow.
Source inspection and previews in the editor are read-only; visual document
editing, editor Save and Play are not available.

Use the [Glossary](manual/glossary.md) to distinguish shared terms across the chapters.

## Host and tool integration

For advanced Rust integration, continue to [Loading and lifecycle](manual/scenes/lifecycle.md)
or [Source editing and Save](manual/scenes/source-editing.md). These describe integration
contracts, not additional editor controls.

For host programming, continue with [Programming](manual/programming/index.md), then
[Graphics and UI](manual/rendering-ui/index.md) for explicit rendering and UI integration.
For standalone application assembly, follow
[Build and distribution](manual/build-and-distribution/index.md): application configuration,
building, explicit launch, packaging and license obligations.

To enable debug connections and understand their permissions, read
[Automation and diagnostics](manual/automation/index.md). Use
[Testing](manual/testing/index.md) to choose checks and prepare native tests, and
the [Command-line reference](manual/cli/index.md) to select the correct command surface.

## Contributing and API reference

To change Kedra, start with the [contributor guide](contributing/index.md) for the
codebase map, architecture, tests and documentation workflow.
The [API reference](api/index.md) connects Rust integration to optional generated
English rustdoc and the paired [Luau API reference](api/luau/index.md).
Luau pages are included in the default build; generated Rust pages require `build:api`.
Site search covers the EN/RU manual, contributor guides and Luau pages; rustdoc has
its own English search.

## Beyond these chapters

For details not yet covered in the manual, use the
[engineering contract directory](contributing/contracts/index.md)
for English and Russian contracts on startup, sources, UI, automation, distribution,
testing and provenance. The [codebase map](contributing/codebase-map.md) also links CI,
repository policy and legal requirements; those documents remain authoritative.
For planned scope, read the
[roadmap (English)](https://github.com/kedra-engine/kedra/blob/main/docs/roadmap.md).
Repository references use GitHub `main`. Publication targets
[docs.kedra.tech](https://docs.kedra.tech/) with the same static pages and complete Rust API.
The site follows `main`; release-specific documentation snapshots are not configured.

## Scope

Kedra is a game development environment built on Bevy, in early development.
Feature availability is limited, and native editor input and rendering still need
platform-specific testing.

API names such as `kedra_scene` remain unchanged in both languages.
