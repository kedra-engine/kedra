# What is Kedra?

Kedra is an open-source game-development environment built on Bevy. Its aim is to connect
project data, assets, scenes, reusable prefabs, programming and a native editor into one
workflow, while keeping ordinary Bevy development available.

The editor is the visible workspace, but it is not the whole environment. Shared runtime
services load content in applications; authoring tools manage editable sources; command-line
tools prepare assets and build standalone applications. These parts share data rather than
requiring the editor to be present in a running application.

## Start with the right expectations

Kedra is in early development. Today it is useful for exploring its project and content
model, inspecting supported content, and developing integrations against the shared Rust
foundations. It is not yet a complete game SDK or a visual game-creation workflow.
APIs and source formats are unstable.

You can create or open a project, browse its sources, open supported scenes and prefabs,
and inspect their hierarchy and component values. With an explicit matching asset delivery,
Scene View can render supported spatial/UI documents and preview PNG and static-GLB assets.
These are read-only workflows: moving the preview camera or selecting an object does not
edit the document, and a rendered preview does not run the project's gameplay.

Visual scene editing, editable document fields, document Save in the editor, project-code
loading and Play are not implemented. Existing Rust source-editing, build and scripting
APIs are foundations for those workflows, not substitutes for them.

## Who is this implementation for?

- **Curious developers** who want to understand how a Bevy-based environment connects
  sources, runtime data and editor views.
- **Rust and Bevy developers** evaluating the scene, asset, scripting or application
  services in a host they explicitly configure.
- **Early contributors** working toward a connected authoring workflow and willing to
  investigate unsupported content and incomplete native acceptance.

If you need to assemble, save and play a complete game entirely in an editor today,
Kedra does not yet provide that loop. You can still learn the concepts without first
learning its crate layout; this manual introduces the user-facing model before API details.

## A small example to keep in mind

Imagine a scene that contains two instances of a reusable cuboid prefab. The project holds
the scene and prefab sources, plus the texture they reference. Asset tools prepare a
delivery that an application can load. The scene system resolves the prefab instances into
ordinary Bevy entities; the editor presents a separate read-only view of supported content.

The two instances can share a prefab source without sharing live entity handles. Inspecting
one instance does not rewrite the prefab. This separation is central to Kedra's model:
editable sources describe intent; loaded instances and editor views serve different purposes.

For document structure and composition, continue with
[Scenes and prefabs](../scenes/index.md).

## Choose a route through the manual

- New to the environment? Read [Product map](product-map.md) for the roles of the editor,
  runtime and tools, then follow [Getting started](../getting-started/index.md) to try it.
- Already use Bevy? Read [Kedra and Bevy](kedra-and-bevy.md) to see what remains standard
  Bevy and what Kedra adds around it.
- Evaluating a workflow? Check [Current capabilities](capabilities.md) before treating a
  design goal as an available feature.
- Investigating content? Follow [Assets](../assets/index.md) and
  [Scenes and prefabs](../scenes/index.md) for the next level of detail.

Test native interactions on your target machine rather than relying on source code alone.
Desktop input, IME and mixed-DPI behavior are not yet certified across a platform set.
Linux, Windows and
macOS are desktop directions; web and Android/iOS runtime support remain future work.
