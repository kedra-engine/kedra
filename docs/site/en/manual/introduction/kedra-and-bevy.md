# Kedra and Bevy

Kedra extends Bevy rather than placing a replacement engine underneath your game.
Components, entities, systems, schedules, reflection, assets and plugins remain Bevy
mechanisms. Kedra connects those mechanisms to authored content and application workflows.
The current workspace pins Bevy **0.20.0**; this is not a compatibility promise for other
Bevy releases or arbitrary third-party plugins.

## What stays familiar

In a Rust application, you still configure a Bevy `App` and install plugins. Systems
operate on ordinary ECS data. A scene instance becomes Bevy entities and components, not
objects in a second ECS with a separate scheduler.

Bevy's asset system still owns loading, dependency tracking and subasset labels. Kedra adds
verified deliveries and stable source identity around that loading path. Likewise, Kedra's
component policies use Bevy reflection to interpret supported authored values rather than
introducing a second reflection system.

For example, a native host can register a reflected Rust component under an authored
schema key, load a scene that uses that key, then instantiate it into its Bevy World.
Registration and validation are explicit: adding a Rust type to your application does not
automatically make every value of that type usable in documents or visible in the editor.

## What Kedra adds

- **Authored scenes and prefabs:** source hierarchies, reusable composition, instance
  overrides and validation before instantiation.
- **Content preparation:** source registration, processing and deliveries verified before
  consumption by shared runtime loaders.
- **Source authoring foundations:** working revisions, validated transactions, undo/redo
  and a Save protocol for tools that integrate the Rust APIs.
- **Application services:** explicit scene startup, opt-in Luau execution and debug-only
  application observation, input injection and renderer capture.
- **A native editor and external tools:** a read-only content workspace today, plus
  separate application build, packaging and explicit launch tooling.

Applications can combine these services as needed. The host—the Rust application
configuring the plugins—chooses the component policies, asset sources, presentation
adapters and script capabilities it supports.

## A runtime feature is not automatically an editor feature

The runtime can instantiate a validated scene without granting the editor permission to
edit it. Rust authoring APIs can change a working source and prepare a Save request without
providing an editable Inspector or a Save command in the editor. Both distinctions matter
when evaluating examples or API documentation.

Rendering is also an explicit integration. The scene crate's optional `render` feature
adds native spatial hydration; loading scene data alone does not install a renderer.
The editor supports a bounded presentation profile, not every Bevy rendering feature.

Luau follows the same pattern. A native host explicitly supplies program sources and
narrow capabilities, then authorizes execution. Behaviors, finite sequences and modules
exist, but opening a project or loading a scene does not discover or execute project
scripts. Script support is not Play, an editor extension API or a general-purpose sandbox
for untrusted native host functions.

## Keep source identity and live state separate

A source node has a durable document identity; a live entity handle belongs to one World
and one lifetime. Two prefab instances receive distinct live entities. Changing a live
component does not change the prefab source or persist an editor edit.

For native content lookup, use full logical paths such as `game://Cuboid.prefab.ron`
through the loaded source catalog. Such a path names content in a source namespace; it is
not an absolute filesystem path. Persistent identities remain in the source data, while
runtime handles remain transient.

Continue with [Product map](product-map.md) to follow content across these boundaries, or
[Loading and lifecycle](../scenes/lifecycle.md) for the scene integration details.
