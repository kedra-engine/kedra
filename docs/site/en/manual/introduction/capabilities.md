# Current capabilities and limitations

Kedra supports the tasks below. Platform testing is incomplete, including native input
and window interactions. APIs and source formats remain unstable.

## What you can use now

| Area | Current implementation | Important limit |
| --- | --- | --- |
| Projects | Create/open a project and validate its manifest and asset-root location. | A new project is not a runnable game template. |
| Workspace | Dock panels and use floating native windows. | Native input, IME and mixed-DPI acceptance remain incomplete. |
| Asset Browser | Browse folders, search names in the current folder, switch grid/list and explicitly open supported content. | Read-only; not an asset-writing or import-settings editor. |
| Scene/prefab inspection | Open admitted documents; select composed nodes in Hierarchy and inspect values in Inspector. | Source-bound, read-only presentation; not arbitrary project-defined components. |
| Scene View | Render supported spatial/UI documents; preview PNG/static-GLB content; use spatial camera navigation and scoped opaque-mesh selection. | Requires an explicit matching verified delivery; not Play or full renderer parity. |
| Scene runtime | Compose scenes/prefabs, apply overrides, compile registered components, instantiate and select an explicit startup scene. | Host configuration is required; scene loading does not authorize scripts. |
| Source authoring APIs | Validated source transactions, revision history, undo/redo and guarded Save protocol. | Rust integration only; no visual scene editing or editor document Save. |
| Assets | Initialize missing source metadata, process supported profiles and verify deliveries for shared loaders. | Not an unrestricted importer for every format or Bevy asset feature. |
| Standalone applications | Build, verify, package and explicitly launch configured desktop Rust applications with shared tooling. | Not a complete game SDK or an editor build/Play workflow. |
| Luau | Host-authorized behaviors, finite sequences and admitted-source modules with scoped capabilities. | No automatic project-code discovery/loading or authorization on project Open. |
| Diagnostics and testing | Explicit debug-only remote observation, injected input, renderer captures and shared native QA tooling. | Not release remote control, arbitrary remote source evaluation or proof of physical input quality. |

To try the editor, follow [Getting started](../getting-started/index.md).
For a deeper content-focused route, use [Projects](../projects/index.md),
[Assets](../assets/index.md) and [Scenes and prefabs](../scenes/index.md).
These explain existing workflows without assuming the missing editor authoring loop.

## Working foundation versus planned workflow

The next authoring milestone is intended to connect scene/prefab changes, an editable
component Inspector, undo/redo and source-owned Save with reopening. That connection is
**planned**, even though some underlying Rust APIs already exist.

In-application **Play** follows authoring: start a scene, observe execution, stop, and
return to editing with an explicit policy for unsaved revisions. It is not implemented.
Neither a Scene View preview nor the standalone CLI's `run` command supplies this workflow.

Project-defined inspectors and editor windows are a later planned stage. There is no
current public project-extension workflow to follow. Automatic project-code loading and
the execution/isolation boundary must not be inferred from existing Luau support.

The longer-term runtime targets include Linux, Windows and macOS on desktop, web, and
Android/iOS on mobile. Web/mobile runtime support is future work; the editor is
desktop-only. These goals do not certify a platform set today.

## Choose a useful experiment

**Understand existing content.** Open a prepared project, inspect a supported document's
hierarchy and values, and compare the preview with its sources. If rendering fails, check
delivery readiness separately from document validity. Do not expect Inspector changes
or a Save command.

**Evaluate a Rust integration.** In an explicitly configured Bevy host, investigate scene
loading, source-editing APIs or scoped scripts. Treat each as an integration task, not a
drop-in game template. The host still chooses policies, resources and execution authority.

**Prepare an application.** Shared build tooling can assemble a configured standalone
application without an installed editor. Native prerequisites, package trust and license
obligations still apply; successful launch dispatch does not establish GUI readiness.

## Verification boundaries

Run the repository's unit/integration tests and native scenarios for the revision and
machine you are evaluating. Native behavior requires real frames and input;
headless checks and documentation builds do not test those interactions.
Native interaction and capture reliability still have unresolved intermittent failures.

When evaluating Kedra, keep three questions separate: does the mechanism exist, does your
content fit its supported profile, and has the intended interaction been verified on the
target machine? [Product map](product-map.md) helps identify which part owns each question.
