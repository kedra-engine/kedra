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

Work toward these goals is incomplete. Not every third-party Bevy plugin
automatically integrates with Kedra's tools.

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
- Shared CLI tooling to build, verify, package and explicitly launch standalone
  desktop applications without an installed editor; see
  [distribution](docs/site/en/manual/build-and-distribution/index.md).
- Debug-only, explicitly enabled remote observation, input injection and renderer captures.
- Shared opt-in Luau behaviors, finite sequences and standard admitted-source modules
  with host-owned API scopes. Native QA uses sequences under external Rust supervision.
- Rust unit/integration tests and local native UI scenarios with real frames/input.

**Not yet implemented:** visual scene/document editing, document Save in the
editor, project code execution or a complete game runtime/Play workflow.
The next milestone combines scene/prefab editing, an editable component inspector
and source-owned Save. In-application Play follows, then project-defined inspectors
and editor windows. See the [roadmap](docs/roadmap.md) for scope and completion criteria.

Button/text-field screenshot checks use shared Rust tooling and project-owned
scenarios through `cargo kedra test` / `cargo xtask test -- …`. The component policy
pins the explicitly approved dependency graph; changed graphs require reviewed lock
approval. Feature checks run separately through Rust with isolated state and real
native input/frames. See the [testing guide](docs/site/en/contributing/contracts/testing.md).
Screenshot coverage is restricted to 22 kit button/input cases; other native scenarios
check behavior without image goldens.
The additional `luau` feature scenario runs an editor-owned script against real
startup state and UI; see [scripting](crates/kedra_script/README.md). Runtime support
does not authorize automatic project-code execution or implement Play.
The independent native application and project-based scenarios share one canonical
[test project](tests/fixtures/test_project/README.md); its ordinary game executable
has runtime-only dependencies and debug-only remote diagnostics.

Native interaction and capture reliability have unresolved intermittent
failures. The kit screenshot corpus and native behavior checks require separate native execution.
A passing headless check is not native UI acceptance. See
[testing](docs/site/en/contributing/contracts/testing.md) and
[native docking checks](docs/site/en/contributing/contracts/ui/native-docking-tests.md).

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
runnable game template. See [startup](docs/site/en/manual/getting-started/launch-editor.md)
for project paths and asset commands.

## Contribute

Repository: <https://github.com/kedra-engine/kedra>.
GitHub Actions is configured for Linux, Windows and macOS validation with fresh
coverage for Rust-affecting changes. Current hosted results and live branch
protections require external verification; see [CI](docs/ci.md). Native distribution is still
being implemented and requires acceptance testing; see the implementation and acceptance
[limits](docs/site/en/contributing/contracts/distribution.md#signing-updates-and-acceptance-limits).

To run the repository's Rust checks:

```sh
cargo xtask check
```

The pinned toolchain installs through rustup. The first run needs network access
to fetch dependencies. Rust checks require no private infrastructure, credentials,
Python or AI subscription. Opt-in native QA uses ordinary Rust scenarios, without
separate QA-driver or report-generator self-tests.
Discuss substantial changes before implementation; see
[contributing](CONTRIBUTING.md) and the [architecture contract](docs/architecture.md).

## Project guides

The [project landing site](website/README.md) is a separate English/Russian static build.
It links to the documentation without changing the manual's routes or appearance.

### Read by task

- Learn existing workflows: [Manual (English)](docs/site/en/index.md) /
  [Руководство (Русский)](docs/site/ru/index.md).
- Change Kedra: [Contributor guide (English)](docs/site/en/contributing/index.md) /
  [Участие в разработке (Русский)](docs/site/ru/contributing/index.md).
- Look up interfaces: [API guide (English)](docs/site/en/api/index.md) /
  [Справочник API (Русский)](docs/site/ru/api/index.md).
  Generated Rust reference requires the opt-in local API build.
- Evaluate planned scope: [Roadmap and acceptance prerequisites](docs/roadmap.md).

Build and read the site locally using the
[documentation infrastructure guide](docs/documentation-infrastructure.md).
Publication targets are [kedra.tech](https://kedra.tech/) and
[docs.kedra.tech](https://docs.kedra.tech/); the [hosting guide](tools/hosting/README.md)
defines CI publication and its verification. Documentation maintainers should also read the
[contents and migration plan](docs/documentation-plan.md).

### Engineering contracts and repository policy

- [Contributing](CONTRIBUTING.md) and [testing](docs/site/en/contributing/contracts/testing.md)
- [Architecture](docs/architecture.md) and [decisions](docs/decisions/0001-foundation.md)
- [Engineering contract directory](docs/site/en/contributing/contracts/index.md)
- [Native startup and shared asset pipeline](docs/site/en/contributing/contracts/startup.md)
- [Authored scene runtime](docs/site/en/contributing/contracts/scene-runtime.md) and
  [source editing](docs/site/en/contributing/contracts/source-authoring.md)
- [Docking controls and acceptance limits](docs/site/en/contributing/contracts/ui/docking.md)
- [Application automation and native UI checks](docs/site/en/contributing/contracts/automation.md)
- [Quality gates](docs/quality.md) and [repository administration](docs/hosting.md)
- [Local validation and GitHub CI](docs/ci.md)
- [AI contributor instructions](AGENTS.md)
- [Security](SECURITY.md), [governance](GOVERNANCE.md), [conduct](CODE_OF_CONDUCT.md)

## License

Except where otherwise noted, Kedra is dual-licensed under
[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE), at your option.
Third-party dependencies retain their own licenses.
See [third-party obligations](docs/third-party.md) before distributing binaries.
