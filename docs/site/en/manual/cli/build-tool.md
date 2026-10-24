# Project-local builder reference

`cargo kedra` is a project-local Cargo alias. In this repository it runs
`tools/kedra`, which forwards OS arguments unchanged to `kedra_build::run_cli`.
It does not require an installed editor and is not a global game SDK command.
The alias isolates its launcher in `target/kedra-build-bootstrap`.

## Application selection

`build`, `run` and `package` share these options:

| Option | Default and restriction |
| --- | --- |
| `--manifest-path PATH` | `Cargo.toml`, resolved from caller CWD |
| `--package NAME` | Omit only when exactly one workspace package has packager metadata |
| `--target TRIPLE` | Current supported native host; must match that host |

Accepted triples are `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`,
`aarch64-apple-darwin` and `x86_64-apple-darwin`. Selecting a different target does
not install SDKs or enable cross-compilation. Each platform still needs its own
native interaction checks.

The selected package needs exactly one binary, standard
`package.metadata.packager` and `package.metadata.kedra.distribution`. Cargo owns
name/version/binary identity. Distribution paths are package-relative and must
remain inside the Cargo workspace. A `kedra.toml` alone is not sufficient.
See [application configuration](../build-and-distribution/index.md).

## Build, run and package

For the full workflow, read [Build and run](../build-and-distribution/build-and-run.md)
and [Packaging and licenses](../build-and-distribution/packaging-and-licenses.md).

```sh
cargo kedra build --release --package kedra --output target/cli-build
cargo kedra run --package kedra --output target/cli-run -- --help
cargo kedra package --package kedra --output target/cli-package
```

- `build [APP_OPTIONS] [--release] [--output PATH]` assembles and verifies a runnable
  application. Without `--release` it uses a development build.
- `run` accepts the same options plus `[-- ARGUMENTS…]`; it builds and verifies a
  fresh result, then requests launch. It never selects an older output.
- `package [APP_OPTIONS] --output PATH` always uses release. It does not accept
  a separate `--release` flag and requires an explicit output directory.

Build/run without `--output` reserve a fresh directory under Cargo's target
directory. Explicit output must also be new and cannot be inside source assets.
Failures retain partial output for diagnosis instead of replacing a previous output. Build
and package do not execute application gameplay/CLI, but do execute trusted Cargo
build code and packaging helpers. Only `run` requests application launch.

A verified build writes `build-result.json`. Native package success additionally
produces inventory/checksum artifacts; neither result file is source configuration.
Packaging can download upstream helpers. Native tools and any configured license
collector must already be available; missing prerequisites fail rather than trigger
an implicit SDK/tool install.

Arguments after `run --` belong to the application, including `--help`. Thus that
example builds and launches; it is not builder help. Launch uses the verified
installed layout, not the source working directory, and does not inject
`KEDRA_ASSETS`. Windows/Linux children have closed stdin and persistent stdout/
stderr logs in the output; macOS uses Launch Services on the actual bundle. A
successful launch request does not prove GUI readiness, successful runtime exit or
physical input. A dispatch failure leaves the verified build available.

## Asset identities and icons

```sh
cargo kedra assets init assets
cargo kedra icons assets/editor/startup/project-mark-compact.svg --output target/cli-icons
```

`assets init SOURCE` is the only asset subcommand in this builder parser. For
standalone delivery build/verify, use the [editor asset CLI](editor-and-assets.md).
Initialization adds missing sidecars without replacing existing identities.

`icons SOURCE --output PATH` accepts a self-contained SVG and a new output
directory whose parent already exists. It creates native icon inputs, including
PNG sizes 16–1024, ICO and
ICNS, without compiling an application. It does not install icons into a project.
Initialization writes asset sources; icon generation writes only its output directory.

## Explicit license-tool setup

Syntax: `tools install-about --output PATH [--archive PATH]`.

```sh
cargo kedra tools install-about --output /OUTSIDE_CARGO_WORKSPACE/about-tool
```

This explicit operation obtains checksum-verified cargo-about 0.9.2 source,
resolves and audits an isolated lock and installs that graph. Cargo and cargo-deny
are prerequisites. Optional `--archive` supplies a local source archive; checksum
verification still applies. Use a new output outside any Cargo workspace and add
its resulting `bin` directory to PATH. Normal builds do not install a missing
collector. This is code/tool installation, not a read-only diagnostic.

Native packaging has additional platform tools and provenance restrictions. See
[distribution](../build-and-distribution/index.md) before installing dependencies;
this command is not a system SDK installer or a license-completeness guarantee.

## Project-owned Rust test target

Use [Native scenarios](../testing/native-scenarios.md) to prepare the desktop and decide
which results to record.

Syntax: `test --manifest-path PATH --package NAME (--bin NAME | --example NAME)
[-- ARGUMENTS…]`.

```sh
cargo kedra test --manifest-path tools/test/Cargo.toml --package kedra-test \
  --bin test -- features --only hierarchy --output target/cli-hierarchy
```

Manifest and package are required; exactly one of `--bin` and `--example` must be
present. This builds the ordinary target with `--locked`, discovers its executable
from Cargo messages and runs it from the manifest directory. An existing lockfile
is required. Scenario arguments preserve boundaries/Unicode; they are not Cargo
options. No builder `--release`, `--target` or `--output` applies to this route;
an output option after `--` belongs to the scenario.

The target owns setup, evidence and native shutdown; nonzero target exit fails
the command. Executing an arbitrary target is trusted code execution, not a
sandbox or a universal application test contract. The repository scenario example
can open windows and inject input. See [repository tests](repository-and-tests.md).

## Parsing and failure behavior

Help/version parsing needs no project or native packaging tools, though the Cargo
alias may first compile the launcher. Unknown/incomplete syntax exits with status
2; execution errors report `FAILED:` and failure status. Child progress/diagnostics
are visible. These operations do not offer an interactive task chooser or a
`--non-interactive` builder flag; unattended packaging is handled in its child.
