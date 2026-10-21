# kedra_update

Blocking, consumer-independent desktop update mechanisms. The editor is the first
consumer; this runtime-layer crate has no Bevy or workspace dependencies.

## Responsibility

- Validate the existing complete GitHub early-access release/inventory protocol.
- Bound HTTPS redirects, metadata, transfer sizes and timeouts; verify stored bytes.
- Own temporary packages, explicit manual-package persistence and native reveal commands.
- Validate explicit native installation markers against consumer-supplied identity/version.
- Stage a complete ARM64 macOS app from a verified DMG with `hdiutil`/`ditto`, acquire
  installation leases and explicitly replace/relaunch using `self_update::Move`.

The host owns resource-location resolution, workers, progress presentation, input/
document settlement, preferences and shutdown authorization. The library does not
schedule checks, display UI, save documents, request application exit or install on
drop. Installation writes are distribution maintenance, not an authoring writer.

## API and lifecycle

Construct `Config` from trusted application configuration: GitHub repository,
application/bundle/executable identity and the running application version. The
library's package version never selects updates or validates installation markers.
The current protocol deliberately retains five desktop payloads, fixed inventory/
checksum names, `assets`/`licenses` resources and `<application>-installation.json`.
This is not an arbitrary package-provider or installer framework.

Call `discover` once on a worker. Explicit activation can call `download`; progress
callbacks execute on that worker. `select_release` and `stage` allow deterministic
offline validation without exposing the HTTP client or metadata DTOs.
`VerifiedPackage` has private construction and removes its staging on drop unless
explicitly persisted for manual installation. Reverify before use; ownership is
not a durable trust receipt. Persistent-package cleanup belongs to the consumer.

For macOS, open the canonical validated installed bundle with `MacInstallation`,
which checks installed bundle identity/version/executable before acquiring its
lease, then call `prepare_macos` on a worker. `OpenError::Unavailable` permits the
host to disable updates without blocking normal startup; `OpenError::Busy` refuses
a launch against an installation held exclusively by another updater.
Dropping `Prepared` cancels staging. Only after
explicit host settlement and UI shutdown may the consumer call `Prepared::commit`
with sanitized native arguments. The returned child confirms spawn, not healthy
startup. Replacement errors retain recovery material; full crash/power-loss recovery
and automatic post-launch rollback are outside the contract. Relaunch clears the
existing Kedra distribution override `KEDRA_ASSETS`; hosts must not replay arbitrary
CLI arguments, environment credentials or automation access.

NSIS and AppImage are manual-only: reverify, reveal the package location and persist
it without exiting or executing an installer. Portable, source and unmarked layouts
remain disabled. No optional features are defined. Native DMG tests are ignored by
default and require an actual macOS host; core policy tests use local fixtures.

## Dependencies and limits

The extraction adds no third-party packages. It reuses the maintained HTTPS/TLS,
SHA-256, plist/Mach-O, filesystem traversal, temporary-directory and replacement
libraries already admitted for this implementation. It owns only the existing
application-specific release/installation policy, not alternative parsers or a
replacement framework. Public errors are diagnostic strings; callers must preserve
failure semantics rather than treating a failed check as permission to install.

HTTPS plus co-hosted SHA-256 trusts the configured GitHub repository; it does not
provide independent publisher authentication. No signature verification, elevation,
download resumption, background installation or post-launch rollback is implemented.
The isolated native DMG fixture covers complete replacement and relaunch using two
ordinary third-party executables. It must not use copied Apple platform executables,
whose launch constraints are unrelated to third-party app updates. Released-artifact,
OS-trust and real input/frame acceptance remain separate; headless tests do not
certify them.

```sh
cargo test -p kedra_update --locked
cargo test -p kedra_update native_dmg_stage_replace_and_relaunch --locked -- --ignored --nocapture
```
