# Command-line reference

Choose among three command interfaces: the editor executable, the project-local builder
and repository development tasks. None requires a globally installed Kedra SDK.

## Task-to-command map

Editor and asset users need the editor/assets rows. Application and scenario authors use
the project-local builder rows. The `cargo xtask` rows are for Kedra contributors checking
this repository; they are not a project's build or Play workflow. For contribution review,
start with [Tests and review](../../contributing/tests-and-review.md).

| Task | Command surface | Reference |
| --- | --- | --- |
| Open the editor or an existing authoring project | `cargo run -p kedra -- …` | [Editor](editor-and-assets.md) |
| Initialize source sidecars | `cargo run -p kedra -- assets init SOURCE` | [Assets](editor-and-assets.md) |
| Process or verify a delivery | `cargo run -p kedra -- assets …` | [Assets](editor-and-assets.md) |
| Assemble a standalone Cargo application | `cargo kedra build` | [Builder](build-tool.md) |
| Build and explicitly launch that application | `cargo kedra run` | [Builder](build-tool.md) |
| Produce native distribution packages | `cargo kedra package` | [Builder](build-tool.md) |
| Generate icons or explicitly install the license collector | `cargo kedra icons` / `cargo kedra tools install-about` | [Builder](build-tool.md) |
| Execute a project-owned Rust test scenario | `cargo kedra test` | [Builder](build-tool.md) |
| Check repository Rust, policy or features | `cargo xtask TASK` | [Repository](repository-and-tests.md) |
| Run repository native scenarios or compare offline frames | `cargo xtask test -- …` / `cargo xtask visual …` | [Tests](repository-and-tests.md) |

An authoring project with `kedra.toml` is not automatically a Cargo application.
Opening it neither builds its Rust code nor implements editor Play. There is no
documented `kedra init` game-template command. `assets init` initializes asset
identities, not a game or a project.

## Working directory and prerequisites

Unless stated otherwise, examples start at the Kedra repository root. Relative
input/output paths resolve in the command's working directory; distribution metadata
has its own package-relative rules. Quote paths containing spaces.

The repository's `.cargo/config.toml` provides `cargo kedra` and `cargo xtask` aliases.
They compile local Rust launchers into separate bootstrap target directories. Another
project needs its own launcher/alias arrangement; installing Cargo alone does not
install these subcommands. See [build and distribution](../build-and-distribution/index.md).

Use the pinned Rust toolchain and native prerequisites described in
[getting started](../getting-started/index.md). Cargo may need network access and
executes trusted build scripts and procedural macros, even when the requested
application operation is headless. Packaging may download and execute native helper
tools. Explicit tool installation writes user-local tooling. Native tests may build
applications, open windows and inject input into the desktop session.

Review projects and tools before executing them. Neither delivery verification nor
output containment is a sandbox for code execution. Do not run native suites on a
desktop in active use. No command here establishes cross-platform acceptance.

## Separators are part of the command

```sh
cargo run -p kedra -- assets verify target/editor-assets
cargo kedra run --package kedra --output target/cli-run -- --help
cargo xtask test -- features --only hierarchy --output target/cli-hierarchy
```

In the first example, `--` ends Cargo's options and starts editor arguments.
The `cargo kedra` alias already contains the Cargo-to-launcher separator; the shown
separator instead starts arguments for the freshly built application. The final
example forwards everything after `--` to the repository's Rust scenario target.
It does not pass a filter to `cargo test`.

Choose a new output path on each invocation where required; do not delete existing
evidence just to reuse an example path.

## Help and errors

The clap parsers accept the following help requests:

```sh
cargo run -p kedra -- --help
cargo run -p kedra -- assets build --help
cargo kedra --help
cargo kedra build --help
cargo kedra test --help
cargo xtask --help
cargo xtask test -- --help
```

Cargo may compile the launcher before help appears. In particular, the last command
builds the scenario executable before requesting its help; it is not a zero-build
probe. Builder help/version parsing itself does not require application metadata or
packaging tools. `cargo xtask` uses a task value rather than subcommands: use its
top-level help, not an assumed `cargo xtask features --help` task-specific schema.

Unknown options and incomplete commands fail rather than silently selecting a
similar operation. Treat a nonzero exit as failure and inspect the live diagnostic.
A successful build, accepted launch request, native behavioral test and screenshot
comparison prove different things; none implies editor Save or Play.

## Continue by intent

- [Getting started](../getting-started/index.md): prepare the build host before running commands.
- [Build and distribution](../build-and-distribution/index.md): configure, assemble and package.
- [Automation](../automation/index.md): authorize and connect to debug application services.
- [Testing](../testing/index.md): choose a check that can detect the failure you care about.
- [Editor startup and asset operations](editor-and-assets.md): sources versus deliveries.
- [Project-local builder](build-tool.md): application selection, outputs and forwarding.
- [Repository checks and tests](repository-and-tests.md): Rust, native and visual routes.
- [Assets](../assets/index.md): resource ownership and supported processing.
