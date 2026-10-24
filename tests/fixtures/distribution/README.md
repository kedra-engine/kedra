# Independent distribution acceptance fixture

This separate Cargo workspace tests distribution, not an engine/game implementation.
The application consumes only existing runtime asset APIs. Its small build launcher
depends on tooling separately; the player dependency graph must not include it.

`tools/distribution/validate_fixture.py` copies these inputs outside the source
workspace, explicitly configures current local or pinned public Git dependencies,
generates its own Cargo.lock and native icon input, and invokes the real
`cargo kedra` alias. No large build script is copied into the application.

The source template intentionally does not contain a second tracked lockfile or
machine-specific paths. It is validated in an isolated generated workspace; do
not mistake its registry version placeholders for a published crates.io SDK.
Its existing `cargo xtask` alias is retained as a behavior check, not overwritten.
