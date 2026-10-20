# Contributing to Kedra

Kedra is in its engineering-foundation stage. Discuss substantial design changes
before implementing them. Small documentation and regression-test improvements
are welcome. There is no stable engine API yet.

## Setup

1. Install Git and rustup.
2. Install a native linker: Xcode Command Line Tools on macOS, Visual Studio C++
   Build Tools with the Windows SDK on Windows, or a C compiler/linker toolchain
   on Linux (for example `build-essential` on Debian/Ubuntu).
3. Clone <https://github.com/kedra-engine/kedra> and run `cargo xtask check`.

Rustup selects `rust-toolchain.toml`. Commit `Cargo.lock`; normal checks use
`--locked`. Dependency updates deliberately regenerate it and receive review.
Linux native builds also need `pkg-config`, ALSA/udev and Wayland/X11 development
libraries. On Debian/Ubuntu:

```sh
sudo apt-get install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev
```

Headless Rust checks do not require a GPU. Native UI acceptance requires real
rendering and input; see [automation](docs/automation.md). Future CI workers need
the same public platform packages. No hosted CI is configured yet.

## Workflow

- Open an issue for a new feature or architectural change; describe the user problem.
- Create a focused branch and include tests and docs in the same pull request.
- Explain the design, compatibility impact, commands run and remaining limitations.
- A bug fix needs a test that fails before the fix and passes afterward.
- Public API or dependency-boundary changes may require an architecture decision.
- Do not include generated caches, credentials or assets without known provenance.
- Keep docs factual: current behavior, contracts, limitations, plans and strategy.
  Review narratives and local execution logs belong in PR discussion or `target/`,
  not permanent product documentation. Preserve licensing and source provenance.
- Use English for public collaboration. Respect the [Code of Conduct](CODE_OF_CONDUCT.md).

### Reports and change proposals

- Bug reports need the exact revision, commands, smallest reproduction, expected
  and actual behavior, OS/architecture/toolchain and redacted relevant logs.
- Proposals describe the problem and intended user, solution and alternatives,
  boundaries, failure behavior, affected platforms, tests and maintenance cost.
- Change requests include a summary and related issue/ADR, design and compatibility
  impact, actual verification commands/results/platform, skipped checks and risks.
  Include regression/failure-path evidence; native UI work needs real input and frames.
- Confirm tests and docs cover the change, `cargo xtask check` results are reported,
  no credentials/private infrastructure/unlicensed content is included and material
  AI assistance is disclosed and reviewed. Do not post private security details.

See [testing](docs/testing.md) for commands and [quality](docs/quality.md) for merge policy.
Passing local checks is necessary, not sufficient: maintainers review behavior and design.
GitHub CI, branch protections and private reporting channels still require
administrator setup and verification; see [hosting](docs/hosting.md).

## AI-assisted work

AI contributions are welcome under the same review standard. Read [AGENTS.md](AGENTS.md).
Review every generated line, verify citations and APIs, and report actual tests.
Do not submit unattended bulk-generated changes or synthetic test results.
Disclose material AI assistance in the PR; disclosure is not a substitute for review.

## Licensing

By intentionally submitting a contribution, you agree to license it under
MIT OR Apache-2.0 unless explicitly stated otherwise. You must have the right
to contribute all code, documentation and assets. No CLA is currently required.
Do not copy third-party or prototype content without checking its license.
