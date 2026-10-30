# Codebase map

Find the authority behind the behavior before choosing a directory. Cargo package layers
describe allowed production dependencies; they are not a ranking of feature importance.
The [architecture contract][architecture] owns the dependency rules, and each crate's
manifest declares its actual layer. Directory names alone do not determine that layer.

## Workspace and entry points

The root [workspace manifest][workspace] includes `crates/*`, `apps/kedra`, `tools/kedra`,
`tools/test` and `xtask`. `tests/fixtures/test_project` is explicitly excluded: ordinary
workspace tests do not execute its independent game. Its [fixture contract][fixture]
explains preparation and ownership; it is not a second engine implementation.

| Path/package | Responsibility | Layer |
| --- | --- | --- |
| `apps/kedra` / `kedra` | Executable and CLI composition for editor/assets | `tooling` |
| `tools/kedra` / `kedra-build-tool` | Thin project-local build CLI | `tooling` |
| `tools/test` / `kedra-test` | Concrete native editor/runtime scenarios | `tooling` |
| `xtask` | Repository command orchestration and policy checks | `tooling` |

The executable's tooling layer does not permit runtime code to depend on its CLI. Follow
the call into the owning library before adding behavior to an entry point.

## Runtime mechanisms

The following packages declare `runtime`. Their mechanisms can be consumed without an
editor dependency, but hosts still choose plugins, sources and authorization explicitly.

| Package | Owns | Does not grant |
| --- | --- | --- |
| `kedra_assets` | Verified deliveries, runtime loaders and optional model admission | Publisher authentication or source editing |
| `kedra_scene` | Documents, composition, instances, startup scene and opt-in rendering | Catalog scanning, Save or script execution |
| `kedra_docking` | Validated topology and prepared candidates as ordinary Rust values | Windows, pixels, documents or close approval |
| `kedra_ui` | Authored UI projection, styling, stable observation IDs and geometry | An editor theme, implicit window or renderer |
| `kedra_script` | Luau programs, modules, budgets, scopes and owner lifetime | Automatic project-code discovery or execution |
| `kedra_script_bevy` | Opt-in Transform/UI script bindings and Activate forwarding | Generic reflected World access |
| `kedra_app` | Shared application launch, debug automation and scripting adaptation | Editor authority or a complete Play workflow |
| `kedra_update` | Verified update/package lifetime and explicit installation mechanisms | Host exit, document settlement or automatic installation |

Read local READMEs where present and module docs otherwise. For example,
[`kedra_scene::lib`][scene] explicitly separates source documents, compilations and live
instances; [`kedra_docking::lib`][docking] describes prepare/commit and unchanged-state
errors. These contracts are better starting points than copying a nearby host call site.

## Sources, import and editor integration

| Package | Layer | Contribution boundary |
| --- | --- | --- |
| `kedra_project` | `authoring` | Project identity, validation, safe creation and separate recent preferences |
| `kedra_authoring` | `authoring` | Working revisions, overrides and source-save protocol |
| `kedra_import` | `import` | Processing/admission and fresh delivery publication |
| `kedra_editor` | `editor` | Project startup, source-editing tools, preview and host coordination |
| `kedra_desktop` | `editor` | Native window/camera/UI-root ownership and acknowledged auxiliary surfaces |
| `kedra_inspector` | `editor` | Consumer-neutral reflected values and presentations |
| `kedra_build` | `tooling` | Application assembly, package verification and explicit standalone launch |
| `kedra_test` | `tooling` | Native process/transport/evidence/comparison mechanisms |

In particular, `kedra_desktop` is not currently a runtime-layer package despite its general
name. `kedra_inspector` is editor-layer but has an additional gate: production workspace
dependencies must be core/runtime, not authoring or editor adapters. Put editor source
selection in the editor adapter rather than making reflection depend on a document host.

No current member declares `core`; the layer is an allowed policy category, not an
unimplemented core crate that contributors should scaffold.

## Trace a cross-cutting symptom

For docking, separate four responsibilities:

1. `kedra_docking` validates topology and candidate ownership.
2. `kedra_ui::docking` projects retained authored chrome.
3. `kedra_editor` owns gesture sessions, feature bindings and preferences.
4. `kedra_desktop` owns native surfaces and their lifecycle acknowledgements.

If a native window disappears while topology stays valid, start with the window lifecycle,
not the topology tests. Conversely, test rejection of an invalid candidate without launching
a window. Use [Tests and review](tests-and-review.md) to choose the test environment.

For resources, distinguish runtime delivery integrity (`kedra_assets`), processing and
publication (`kedra_import`), source changes (`kedra_authoring`) and editor preview binding.
An admitted source is not automatically a ready render resource. The editor's
[Scene View contract][scene-view] owns its supported preview and isolation behavior.

## Engineering contract owners

Use the [contract directory](contracts/index.md) to find the contract for your change.
Repository policy, ADRs and legal sources keep their authority. Repository links
are not a release snapshot or proof that uncommitted changes are available on GitHub `main`.

| Concern | Owning reference |
| --- | --- |
| Startup and native surface lifecycle | [Startup][startup] |
| Source composition, instances and Save protocol | [Scene runtime][scenes-guide]; [source authoring](contracts/source-authoring.md) |
| Editor source admission and session retention | [Source documents][documents] |
| Preview resource binding and isolation | [Scene View][scene-view] |
| Reflection presentation and editor bindings | [Inspector][inspector] |
| Dock topology integration and borrowed header content | [Docking][dock-guide]; [window chrome][chrome] |
| Debug connection, operation and capture authority | [Application automation][automation] |
| Assembly, packages and release orchestration | [Distribution][distribution] |
| Test ownership and visual evidence policy | [Testing][testing]; [visual comparison][visual]; [UI corpus][corpus] |
| CI execution and repository administration | [CI][ci]; [hosting][hosting] |
| License obligations and asset provenance | [Third-party obligations][legal]; [authored assets][assets] |
| Documentation builds, output ownership and publication boundary | [Documentation infrastructure][docs-infrastructure] |

## Locate the next source, not another framework

Start with the owner's manifest, `src/lib.rs`, public contract target and direct consumer.
For editor changes, consult the affected [source-document][documents],
[Inspector][inspector] or [docking implementation][dock-guide] contract. For tools, read
the [distribution][distribution] or [native test][test-crate] contract first.

The checked DAG concerns declared workspace production/build edges, including optional,
renamed and inactive target dependencies. Dev dependencies are excluded. It does not
analyze arbitrary external capability graphs or prove semantic isolation within a mixed
crate. Review responsibility and feature/target behavior as well as the layer result.

Continue with [Architecture and changes](architecture-and-changes.md) before introducing
a dependency edge or separating a new mechanism from its host.

[architecture]: https://github.com/kedra-engine/kedra/blob/main/docs/architecture.md
[workspace]: https://github.com/kedra-engine/kedra/blob/main/Cargo.toml
[fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md
[scene]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/src/lib.rs
[docking]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_docking/src/lib.rs
[scene-view]: contracts/editor/scene-view.md
[documents]: contracts/editor/source-documents.md
[inspector]: contracts/editor/inspector.md
[dock-guide]: contracts/ui/docking.md
[distribution]: contracts/distribution.md
[test-crate]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_test/README.md
[startup]: contracts/startup.md
[scenes-guide]: contracts/scene-runtime.md
[chrome]: contracts/ui/window-chrome.md
[automation]: contracts/automation.md
[testing]: contracts/testing.md
[visual]: contracts/ui/visual-verification.md
[corpus]: contracts/ui/ui-corpus.md
[ci]: https://github.com/kedra-engine/kedra/blob/main/docs/ci.md
[hosting]: https://github.com/kedra-engine/kedra/blob/main/docs/hosting.md
[legal]: https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md
[assets]: https://github.com/kedra-engine/kedra/blob/main/assets/README.md
[docs-infrastructure]: https://github.com/kedra-engine/kedra/blob/main/docs/documentation-infrastructure.md
