# Kedra

**An open-source game development environment built on Bevy.**

Kedra brings together runtime extensions, authoring tools and a native editor to
make creating games faster and more convenient, while preserving Bevy's
flexibility and direct access. It extends Bevy rather than replacing it.

The editor is one way to work with the environment, not the whole product. Rust
code, visual tools, command-line workflows and remote application control are
intended to work with the same project data and lifecycle.

## Goals

- **A connected workflow.** Bring projects, assets, scenes, prefabs, editing and
  game execution together, instead of requiring each game to assemble that
  workflow from scratch.
- **Convenience without a lower ceiling.** Provide useful defaults and reusable
  tools while keeping ordinary Bevy components, systems and plugins available
  for custom gameplay and low-level work.
- **Composable foundations.** Build with ordinary Rust crates and Bevy plugins.
  Keep runtime functionality independent of the editor and authoring tools, with
  explicit integration contracts rather than a parallel ECS or scheduler.
- **Authoring as a first-class workflow.** Keep editable scenes, prefab sources
  and assets distinct from running game state. Tools should edit sources;
  playing a game should not implicitly overwrite them.
- **Automation alongside interactive tools.** Make application observation,
  control and verification accessible beyond manual editor interaction.

These are product goals, not a claim that every workflow is already implemented
or that every third-party Bevy plugin automatically integrates with Kedra's tools.

## Current status

**Early development: the foundations of the environment are being built.**
Kedra is not yet ready to serve as a complete game-development workflow, and its
APIs and source formats are not stable.

The repository currently provides:

- Project creation/opening and a native editor shell with dockable panels and
  floating windows. The read-only Asset Browser supports source-folder navigation
  and selection, folder search, grid/list and read-only scene/prefab document Open.
  Hierarchy displays composed source nodes with read-only navigation and selection.
  Read-only Scene View renders supported spatial/UI documents and previews
  PNG/static-GLB assets with an explicit matching delivery. Spatial navigation and
  scoped opaque-mesh selection are implemented. The read-only Inspector follows
  source selection, with lock, search and shared reflected field presentations.
- Shared scene and prefab documents, composition, instantiation and guarded
  source-editing APIs. The editor's own UI uses authored sources.
- Asset processing and verified deliveries consumed through shared runtime loaders.
- Opt-in remote application observation, input injection and renderer captures.
- Rust unit/integration tests, headless QA-tool tests and local native UI fixtures.

**Not yet implemented:** visual scene/document editing, document Save in the
editor, project code execution or a complete game runtime/Play workflow.
The next milestone combines scene/prefab editing, an editable component inspector
and source-owned Save. In-application Play follows, then project-defined inspectors
and editor windows. See the [roadmap](docs/roadmap.md) for scope and completion criteria.

Native interaction and capture reliability still have unresolved intermittent
failures. GitHub Actions acceptance is pending; the full real-window visual/interaction
corpus requires separate local execution. A passing headless check is not
native UI acceptance. See [testing](docs/testing.md) and
[native docking checks](docs/ui/docking-tests.md).

Planned game runtime targets include Linux, Windows and macOS on desktop, web,
and Android and iOS on mobile. Web and mobile runtime support is future work;
the editor remains desktop-only. No platform set is certified yet, including
physical input, IME and mixed-DPI monitor behavior on desktop.

## Try the current editor

Install [rustup](https://rustup.rs/), clone the repository and follow the native
platform prerequisites in [CONTRIBUTING.md](CONTRIBUTING.md). Then build a new
asset delivery and launch:

```sh
cargo run -p kedra -- assets build assets/editor target/editor-assets --namespace editor
cargo run -p kedra -- assets verify target/editor-assets
cargo run -p kedra -- --assets target/editor-assets
```

The delivery output directory must not already exist; choose a new path for a
subsequent build. This opens the current project manager/editor shell, not a
runnable game template. See [startup](docs/startup.md) for project paths and asset
commands.

## Contribute

Repository: <https://github.com/kedra-engine/kedra>.
The GitHub Actions workflow is defined locally; hosted acceptance and repository
protection verification remain pending.

To run the repository's Rust checks:

```sh
cargo xtask check
```

The pinned toolchain installs through rustup. The first run needs network access
to fetch dependencies. Rust checks require no private infrastructure, credentials,
Python or AI subscription. Python is used for the separate QA-tool tests and
opt-in native fixtures. Discuss substantial changes before implementation; see
[contributing](CONTRIBUTING.md) and the [architecture contract](docs/architecture.md).

## Project guides

- [Contributing](CONTRIBUTING.md) and [testing](docs/testing.md)
- [Architecture](docs/architecture.md) and [decisions](docs/decisions/0001-foundation.md)
- [Native startup and shared asset pipeline](docs/startup.md)
- [Authored scenes and source editing](docs/scenes.md)
- [Docking controls, architecture and acceptance limits](docs/ui/docking-implementation.md)
- [Shared application automation and native UI checks](docs/automation.md)
- [Quality gates](docs/quality.md) and [repository administration](docs/hosting.md)
- [Local validation and GitHub CI](docs/ci.md)
- [Roadmap and acceptance prerequisites](docs/roadmap.md)
- [AI contributor instructions](AGENTS.md)
- [Security](SECURITY.md), [governance](GOVERNANCE.md), [conduct](CODE_OF_CONDUCT.md)

## License

Except where otherwise noted, Kedra is dual-licensed under
[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE), at your option.
Third-party dependencies retain their own licenses.
See [third-party obligations](docs/third-party.md) before distributing binaries.
