---
apiHeading: Generated workspace libraries
apiAvailable: This complete local build includes the workspace libraries listed below.
apiUnavailable: This default build includes Luau but no rustdoc. Run build:api for crate links.
---

# API reference: Rust and Luau

## Choose your task

The [Luau reference](luau/index.md) is available in the default site build:

| Task | Start here |
| --- | --- |
| Write game scripts | [Game scripting](luau/game-host.md) and [live-object operations](luau/bevy.md) |
| Observe editor state | [Editor diagnostics, not plugins](luau/editor.md) |
| Drive a debug application | [Application automation](luau/application.md) |
| Maintain a test scenario | [Test fixtures](luau/testing.md) |
| Install and authorize Luau from Rust | [Native integration](luau/integration.md) |

[Runtime](luau/runtime.md), [conventions](luau/conventions.md) and
[implementation limitations](luau/limitations.md) apply across the host APIs.
These are host-installed capabilities, not a universal game SDK. Luau pages share
the EN/RU site search with the manual.

For Rust integration, use the generated-reference guide below. For workflow context, see
[Programming](../manual/programming/index.md); to change Kedra, use
[Contributing](../contributing/index.md).

## Rust API guide

The Rust API reference is generated from the current checkout's Rust documentation
comments. It describes English identifiers and contracts, not a translated API or
a separately hosted release. Kedra's APIs and source formats are not stable.

## Open the generated reference

The generated library list at the end of this page contains crate links only in
a complete build. The default Rust-free build explains their absence instead.

From the repository root, after installing the documentation dependencies:

```sh
cd docs
bun run build:api
bun run preview
```

Open this page in the loopback preview. Rustdoc pages live under `/api/`, retain
their own navigation and English API search, and are not part of the bilingual
manual search. Use the browser's Back button to return to the manual.
`bun run build` produces the faster Rust-free preview, including Luau, and removes
generated rustdoc from that preview. Do not run the two builds concurrently.

## Scope and feature contract

The complete build requires the repository's pinned Rust toolchain, native build
prerequisites and locked Cargo dependencies. It runs:

```sh
cargo doc --workspace --no-deps --all-features --locked
```

The integration adds an isolated `--target-dir target/documentation/cargo` and
denies warnings and broken intra-doc links, matching the repository's rustdoc gate.
The displayed versions come from Cargo metadata for this checkout, not a published
release promise. All workspace libraries are linked below; Cargo also generates
workspace binary documentation. External dependency documentation is not bundled;
some rustdoc links lead to upstream websites and require network access.

This is one all-features, host-platform, debug-assertions-enabled documentation
build. It does not prove default, minimal, release or other target/feature
combinations. For example, `kedra_scene/render`, `kedra_assets/model-support` and
`kedra_ui/text-capture` expose opt-in surfaces. Read each crate's contracts and
feature requirements before using an item; inclusion here grants no runtime or
editor authority and does not implement Play or source Save.

## Output and licenses

The generated preview belongs under `target/documentation/site/`; it is not
authored source. A failed complete build removes that preview rather than serving
stale or partial API pages. Cargo's isolated build cache remains for retry; the
ordinary `target/doc/` tree is not changed by this integration.

The preview adds checked range aliases around real source-line anchors, preserving
rustdoc's source links and JavaScript highlighting. A bounded compatibility step
also resolves six inherited dependency-link forms to their verified versioned
upstream pages. It changes only copied HTML, not Cargo's original documentation,
source code, search scripts or legal assets. Unreviewed toolchain/dependency
versions or incompatible known output shapes stop the complete build.

Keep the entire generated `api/` directory when copying a complete preview.
Rustdoc's static assets include their own copyright and license files, including
SIL Open Font License fonts and Rust-logo attribution. The manual's
`third-party-notices.txt` does not cover these assets. Kedra's MIT and Apache-2.0
license texts are copied alongside the API. The production documentation build includes
this complete reference; release-specific snapshots are not configured.
