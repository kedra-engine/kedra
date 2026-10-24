# Build and run

Use `build` to assemble an application without launching it. Use `run` only when you
intend to execute the selected application. Both commands compile trusted project code;
neither is a substitute for reviewing an unfamiliar Cargo project.

## Before building

Check [application configuration](configuration.md), source sidecars and a reviewed
`Cargo.lock`. The inner application compilation uses `cargo build --locked` and discovers
the executable from Cargo's artifact messages rather than guessing a target path.

Install rustup and a native linker: Xcode Command Line Tools on macOS, Visual Studio C++
Build Tools with the Windows SDK on Windows, or a C compiler/linker on Linux. This checkout
pins its toolchain in `rust-toolchain.toml`. Linux editor builds also need `pkg-config`
and ALSA/udev, Wayland/X11 and Fontconfig development libraries. On Debian/Ubuntu,
Fontconfig needs `libfontconfig1-dev`; see the full
[platform package list](../getting-started/index.md).

Preflight checks Cargo plus `codesign`/`plutil` on macOS or `ldd` on Linux. If metadata
contains `license-config`, it also requires `cargo-about 0.9.2` on PATH. The check does not
install missing tools or guarantee that every compiler/system dependency is present.
See [packaging prerequisites](packaging-and-licenses.md) for the separate explicit setup.

## Build the configured editor

From the repository root, after satisfying those prerequisites:

```sh
cargo kedra build --package kedra --release --output target/documentation/examples/build-and-distribution/editor-build-01
```

This command builds `apps/kedra`, not a game from an editor project. For each repeat,
choose a fresh suffix or another new output directory.

`--manifest-path` selects the Cargo manifest (default `Cargo.toml`). `--package` selects
one application package; without it, exactly one workspace package must have `packager`
metadata. `--target` defaults to the host triple and must match it. Omitting `--release`
uses the development profile; no arbitrary Cargo feature/profile passthrough is exposed
by these build/run commands.

`--output` is optional for build/run. Without it, the tool reserves a unique directory
under Cargo's target directory at `kedra-build/build-*` and prints its location. An explicit
output is resolved from the invocation directory. Neither form may be inside source assets.

## What happens during assembly

1. Validate metadata and check native prerequisites.
2. Reserve a new output directory and prepare `application/` staging.
3. Generate icon variants, process the source assets and verify the delivery.
4. Copy configured notices and, when configured, collect dependency license/source material.
5. Compile the selected Cargo binary with the target's native toolchain.
6. Assemble the runnable layout and verify executable architecture, resources and notices.
7. Write the successful `build-result.json` handoff.

macOS assembly creates a complete `.app` and signs the bundle, ad-hoc by default, without
requiring a DMG. Windows/Linux use an executable with adjacent resources. Build does not
execute the application's CLI or gameplay, though Cargo build scripts and native helpers
are still code execution within the trusted build workflow.

## Read the result, not a guessed path

`build-result.json` is output data, not a project setting. Schema version `1` records:

| Field | Meaning |
| --- | --- |
| `application`, `version`, `target` | Selected Cargo application and native target |
| `application_root` | Runnable directory or bundle, relative to the reserved output |
| `executable` | Executable location relative to that same output |
| `resources` | Verified asset delivery location relative to that same output |

For Windows/Linux, the application root is `application/`, with `assets/` and `licenses/`
beside the executable. On macOS it is `native-application/<productName>.app`, with the
executable under `Contents/MacOS/` and assets under `Contents/Resources/assets/`.
Keep the complete runnable root, not just its executable.

Verification checks executable format/architecture, executable permission where applicable,
delivery checksums and installed notices against staging. Linux also checks unresolved
dynamic libraries; macOS checks bundle identity and the complete signature. These are
structural checks, not a renderer/input test. Build alone creates no package inventory.

## Explicitly launch a fresh result

To build and request a launch of the editor's help command:

```sh
cargo kedra run --package kedra --output target/documentation/examples/build-and-distribution/editor-run-01 -- --help
```

Run this command only if you intend to execute the editor.
Application arguments go after `--`; their boundaries and Unicode are preserved.
`run` uses the result of this invocation only, never an older build directory.
It does not accept a saved `build-result.json` as an input for skipping compilation.

The launch working directory is the reserved output, not the source checkout. The launcher
removes inherited `KEDRA_ASSETS`, so the application must implement its installed resource
lookup rather than depend on a source working directory. It does not inject a source path.

On Windows/Linux, the process has closed stdin and writes to `application-stdout.log`
and `application-stderr.log` in the output directory. On macOS, Launch Services opens
the actual `.app` using `/usr/bin/open -n ... --args`; do not expect the same log files.
The launcher does not intentionally terminate the application when it exits.

Startup, rendering and input remain unverified for these command examples.
“Launch request accepted” means dispatch succeeded. It does not wait for the application
to exit, render a frame or accept physical input. Check the application's readiness and
observe its native window and input separately. This command does not add editor Play.

## Recover from failure

| Symptom | Action |
| --- | --- |
| Application selection/metadata error | Select one package and check both metadata tables and its one binary target. |
| Missing prerequisite or wrong cargo-about version | Install the reviewed prerequisite explicitly; inspect PATH before retrying. |
| Existing output / output inside assets | Choose a new directory outside source assets; preserve previous evidence. |
| Missing sidecars, invalid SVG or delivery error | Repair the owned source inputs; do not replace established identities. |
| Cargo failure or stale lockfile | Read live compiler diagnostics; review dependency/lockfile changes separately. |
| Invalid executable, resources or bundle signature | Treat assembly as failed; do not copy partial staging as a release. |
| Launch failure after verification | The verified build remains on disk; diagnose OS dispatch and application requirements. |

Record the exact command, platform and diagnostics. Retry with fresh output after fixing
the cause. Next: [packaging and licenses](packaging-and-licenses.md).
