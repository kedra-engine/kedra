# Getting started

Follow this route to build Kedra from public source, open a populated project and inspect
your first document. The current editor is a desktop, read-only authoring viewer; this
walkthrough does not create or run a game.

## Before you begin

For the product model and limits, first read [What is Kedra?](../introduction/index.md).

You need Git, [rustup](https://rustup.rs/), network access for the first dependency download
and a native compiler/linker for your platform:

| Platform | Native build prerequisites |
| --- | --- |
| macOS | Xcode Command Line Tools |
| Windows | Visual Studio C++ Build Tools and the Windows SDK |
| Linux | C compiler/linker toolchain, `pkg-config`, ALSA/udev and Wayland/X11 development libraries; native font discovery also uses Fontconfig |

On Debian/Ubuntu, install the compiler toolchain (for example `build-essential`) and these
native development packages. This command is for Debian/Ubuntu only, not other Linux
distributions or macOS:

```sh
sudo apt-get install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libfontconfig1-dev
```

Headless asset processing does not require a GPU. Running the editor and rendering previews
require a working desktop graphics environment. Linux, Windows and macOS have configured
CI checks, but no desktop platform set is certified for physical input, IME or mixed-DPI
behavior. Web and mobile are not editor targets.

Node.js and Bun belong to the documentation-site toolchain. **Neither is required to build
or run the native editor.** No private infrastructure or AI service is required either.

## 1. Get the source and pinned Rust toolchain

Clone into a new checkout directory, then run subsequent commands from its root:

```sh
git clone https://github.com/kedra-engine/kedra.git
cd kedra
rustup show active-toolchain
```

The checked-in `rust-toolchain.toml` selects Rust **1.98.0**, with `rustfmt` and `clippy`.
Rustup resolves that pin from inside the checkout; do not substitute an arbitrary latest
toolchain. If the pinned toolchain is missing, install it explicitly:

```sh
rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy
```

The commands below use `--locked` to retain the dependency resolution recorded in `Cargo.lock`.
The first Cargo invocation compiles dependencies and can take substantially longer than
later runs. Keep the terminal open to see progress and diagnostics.

## 2. Prepare and launch

Continue to [Prepare resources and launch the editor](launch-editor.md).
You will build a delivery for the editor's own interface, verify it, then choose between
the Projects window and directly opening the repository's populated test project.

For your first inspection, use the existing fixture rather than **New project**:
a newly created project has an empty `assets/` directory and no scene to open.
The separate [Create or open a project](../projects/create-open.md) guide covers that route.

## 3. Inspect a document

Continue to [Open and inspect your first document](first-document.md) to find a known prefab,
select its node in Hierarchy and read its components in Inspector. A matching project
delivery is optional for source inspection, but required for a rendered preview.

For panel controls, keep the [Editor workspace](../editor/index.md) reference at hand.
Check native interaction and rendering on your machine: a successful build or verify
command does not establish that the editor produced a frame.
