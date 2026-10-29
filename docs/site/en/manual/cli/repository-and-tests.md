# Repository checks and test dispatch

`cargo xtask` is repository development tooling, not a command to build the open
authoring project. Its alias compiles the orchestrator in `target/xtask-bootstrap`;
the orchestrator changes CWD to the Kedra repository root before dispatch.

For the development and review workflow, follow
[Tests and review](../../contributing/tests-and-review.md).
Application and scenario authors testing their own application should use the
[project-local builder's test target](build-tool.md#project-owned-rust-test-target).

## Repository tasks

```sh
cargo xtask check --non-interactive
cargo xtask features --non-interactive
```

| Task | Current dispatch |
| --- | --- |
| `files` | Per-file growth policy, including tracked ignored files |
| `architecture` | Workspace dependency layers, license and lint-inheritance policy |
| `dependency-policy` | Offline advisory exception expiry/reachability and bounded duplicate/license waivers |
| `fmt` | `cargo fmt --all -- --check`; does not apply formatting |
| `lint` | Workspace all-target/all-feature locked Clippy; unsafe code forbidden and warnings denied |
| `test` | Locked workspace all-target/all-feature tests, then separate doctests |
| `doc` | Locked workspace all-feature API docs without dependency docs; strict rustdoc warnings/links |
| `check` | `files`, `architecture`, `dependency-policy`, `fmt`, `lint`, `test`, `doc`, in that order |
| `features` | Default/minimal workspace, independent runtime/data crates and release diagnostic boundary |
| `visual` | Explicit offline reference inspection or screenshot comparison |

`check` has eight stages because `test` contains two Cargo invocations. It does not
include `features`, native desktop scenarios or offline visual comparison. `doc`
generates local Rust API documentation, not an installed or hosted API-reference service.

`features` checks workspace default and no-default-feature all-target builds, then
isolated no-default-feature libraries `kedra_app`, `kedra_script`, `kedra_scene`,
`kedra_assets`, `kedra_ui` and `kedra_wakeup`, plus profiling-disabled `kedra_profiling`
tests. Finally it runs `kedra_app`'s `launch::tests` under
the `release-boundary` profile without default features. It does not certify all
target/feature combinations or native interaction.

`dependency-policy` reads the repository's `deny.toml` and `Cargo.lock` without
invoking Cargo or requiring its package cache. `KEDRA_LUAU_LOCKFILE` optionally
selects an additional resolved consumer lock; a missing, malformed or unreviewed
explicit lock fails. Cargo-deny remains the separate advisory/license scanner.

Omitting the task in an interactive terminal opens a keyboard chooser. Outside that mode,
omitting the task fails instead of prompting or selecting `check`. `CI` presence,
redirected streams, `TERM=dumb` or `--non-interactive` disable the interactive UI.
An explicit task does not require a menu.

Ordinary task runs stream child output and record separate plain-text logs under
`target/xtask/logs`. A failed stage stops later stages; the summary reports skipped
work. Early compilation failure can precede log creation. Logs are not automatically
uploaded or pruned; inspect paths and sensitive data before sharing. Forced
termination can prevent the final summary.

## Native scenario forwarding

```sh
cargo xtask test -- features --only hierarchy --output target/cli-hierarchy
cargo xtask test -- components --output target/cli-components
```

Only `test` accepts trailing scenario arguments. With no arguments after `--`, it
runs Rust tests/doctests. With arguments, it instead builds and executes the
ordinary `tools/test` target through `kedra_test`; it does not also run the Rust
suite. Put native options after the separator. Visual options before it are
rejected when native dispatch is selected.

All scenario commands accept optional `--output PATH`. An explicit directory must
be new; relative outputs are rooted at the repository. Without it, the runner
reserves a directory under `target/test/ui-runs` with prefix `rust-`. Native runs
need an idle supported desktop and platform prerequisites. They build trusted
code, launch applications and may deliver injected input. Do not run them merely
to inspect syntax.

### Current scenario arguments

- `features [--only NAMES]`: comma-separated named behavior scenarios. Current
  names are `startup`, `luau`, `browser`, `hierarchy`, `inspector`, `workspace`,
  `window-chrome`, `docking`, `docking-states`, `scene-view`, `scene-navigation`,
  `inspector-fixture` and `dock-fixture`. Unknown/duplicate names fail. A selected
  subset is not complete-suite acceptance.
- `components [--source PATH] [--policy PATH] [--only NAMES]`: the 22 button/input
  cases in two sessions. Defaults are `docs/ui/corpus/text` and
  `docs/ui/corpus/text-policy.json`, relative to the repository. `--only` selects
  in-scope case IDs for diagnostics, not feature names or full acceptance. A changed
  dependency graph still requires reviewed lock approval; a subset cannot bypass it.
- `native-switch` and `native-projection`: each requires `--manifest-path PATH`,
  `--assets PATH` and `--project-assets PATH`. These are concrete contracts for a
  complete private copy of the canonical test project, not arbitrary game runners.
- `export`: exactly one of `--text-plugin` or `--text-bundle PATH` is required.
  It writes a local Figma plugin or imports a downloaded source bundle. It is
  explicit source export/import, not native acceptance or automatic publication.

Feature scenarios check behavior and renderer synchronization, not pixel matches.
The component corpus compares only its listed controls with independent references,
not every editor panel. Follow the testing guides below for readiness, source isolation
and report review. Neither route permits unreviewed baseline updates.

## Offline visual task

```sh
cargo xtask visual --config PATH/TO/comparison.json \
  --actual PATH/TO/evidence.json --output target/cli-comparison
cargo xtask visual --inspect-reference PATH/TO/manifest.json \
  --output target/cli-reference
```

Comparison requires `--config`, `--actual` and `--output`. Reference inspection
requires `--inspect-reference` and `--output`; it conflicts with `--config` and
`--actual`. Output is a new artifact directory. These flags are valid only for
`visual`; trailing native arguments are not accepted for that task.

This route consumes existing files and does not capture a running application.
Reference integrity is not native acceptance; successful image comparison is not
independent design approval. Do not replace a failing baseline with the actual
image to make a check pass.

## Where to go next

Follow [Repository checks](../testing/repository-checks.md) for the development loop,
[Native scenarios](../testing/native-scenarios.md) for supervised behavior runs and
[Visual verification](../testing/visual-verification.md) for capture/comparison review.

For first launch use [editor commands](editor-and-assets.md). For standalone
application builds and project-owned scenario targets use [the builder](build-tool.md).
[Editor](../editor/index.md) and [rendering](../rendering-ui/index.md) explain the
capabilities whose verification must be distinguished from CLI exit status.
