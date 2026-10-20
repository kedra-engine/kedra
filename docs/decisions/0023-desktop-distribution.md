# ADR 0023: Shared native application build and distribution tooling

- Status: accepted architecture; native package acceptance is pending
- Date: 2026-10-21

## Context

The editor needs native release downloads. Future ordinary Rust games must use
the same asset/build/packaging implementation without depending on editor crates
or copying repository scripts. The owner approved a separate tooling crate and
project-local command, with unsigned first releases and no paid services.

## Decision

- `kedra_build` is a tooling-layer crate with the editor and an independent
  application fixture as immediate consumers. It does not depend on xtask/editor;
  no runtime crate depends on build/import/packaging capabilities.
- A pinned, tiny project-local launcher and Cargo alias expose `cargo kedra`.
  `cargo xtask` remains repository verification. A dependency alone cannot install
  Cargo aliases; independent projects must explicitly include the launcher/config.
  Native packaging self-reexecutes that launcher with internal arguments, giving
  only the child the application's working directory and unattended packaging
  environment. Launchers forward process arguments unchanged; no process-global
  environment mutation or Finder automation is needed.
- Cargo and cargo_metadata own compilation and artifact discovery; existing
  Kedra import/delivery APIs own asset preparation and verification. Native
  packaging formats belong to maintained cargo-packager, not custom installers.
- Use its library from reviewed upstream commit
  `5c1824ba8022401e9ef0de36ff1a12c5acf60237`, with CLI features disabled, in tooling
  only. Resolve dependencies through the existing audited workspace Cargo.lock.
  Native graph, licensing/security policy and output checks must pass before use.
- Application identities, source resources and icons are configuration rather
  than fixed editor paths in generic tooling. Generate platform icons from the
  approved source using maintained image/SVG tooling, not replacement artwork.
- Installed resources use standard layouts: macOS Contents/Resources/assets,
  Linux usr/lib/<executable>/assets for usr/bin installs/AppDirs, and adjacent
  assets for portable/development executables. Explicit overrides retain priority.
- Compile/package jobs are read-only. Only the final complete-artifact publisher
  may write a GitHub prerelease after exact-revision successful CI. No code signing
  or notarization is claimed without certificates.

## Alternatives and dependency justification

Keeping packaging in xtask ties games to the editor repository. Copying xtask
spreads divergent build policy. A mandatory globally installed tool can mismatch
the project's engine version. The local launcher avoids these problems without
adding a new build language, dependency container or plugin runtime.

The released cargo-packager 0.11.8 CLI was evaluated but its published frozen graph
contains current advisories, including zip 0.6 advisories requiring a major upgrade.
The maintained upstream commit updates that major. Its source is immutable while
the workspace can resolve compatible security fixes and enforce the existing
policy, rather than install a known-vulnerable frozen CLI or suppress findings.
No patched private fork, compatibility forwarding crate or extra exempt lockfile
is introduced. The tradeoff is a larger tooling-only dependency graph.

Keep duplicate denial enabled with exact reviewed exceptions for the unavoidable
API splits below, not subtree waivers or wildcard ranges:

| Exact older identity | Existing/new immediate consumers and incompatible line |
| --- | --- |
| block-buffer 0.10.4 | digest 0.10.7 versus packaging digest 0.11/block-buffer 0.12 |
| crypto-common 0.1.7 | digest 0.10/cipher 0.4 versus digest 0.11/crypto-common 0.2 |
| digest 0.10.7 | existing sha2/minisign versus packaging sha1/sha2 0.11 |
| sha2 0.10.9 | runtime integrity/minisign versus packaging sha2 0.11 |
| getrandom 0.2.17 | minisign/ring versus existing 0.3/0.4 callers |
| core-foundation 0.9.4 | Winit/core-graphics versus packaging platform TLS 0.10 |
| png 0.17.16 | tauri-icns 0.1 versus existing PNG/image 0.18 |
| quick-xml 0.41.0 | existing font configuration versus plist 1.10 using 0.42 |

Two exact certificate-data packages, webpki-roots and webpki-root-certs 1.0.10,
use CDLA-Permissive-2.0. Its reviewed terms permit use/modification/sharing, require
providing the license text with shared data, and impose no restriction on results.
Allow that license only for those identities, not globally. Retain the text when
distributing tooling that embeds the data. Native TLS was evaluated but still
depends on certificate data while adding OpenSSL/platform API splits; retain
Rustls with platform verification rather than adding those unnecessary inputs.
No vulnerability advisory is suppressed by this packaging decision.

cargo-dist is a maintained release-oriented alternative, but adopting two
orchestrators duplicates release ownership. cargo-packager covers the required
native GUI formats without adopting a web runtime. Its native helper downloads
remain an upstream trust boundary: not every backend cryptographically pins every
helper. Inspect/document that behavior and do not claim hermetic or authenticated
build inputs beyond the evidence available.

## Consequences and verification

The application fixture must build/package outside the editor workspace with its
own metadata, resources and launcher; a public pinned Git-consumer check follows
the implementation commit. Tooling must never rely on its compile-time source
directory. Windows resource hooks compile local resources only; they do not
download SDKs, invoke Cargo or perform bundling.

Native package checks, real process startup, rendered frames/interaction and
physical OS integration are distinct evidence. Missing hardware is unverified,
not green. This does not implement a game runtime, scaffold product or Play.

## License collection and source access

Use maintained cargo-about 0.9.2 for license discovery, with an explicit application
configuration. Handlebars owns HTML escaping/presentation; Cargo's lockfile parser
and SHA-256 bind downloaded covered-source archives to the selected revision.
No custom license scanner or SPDX parser is added. Direct cargo-lock, Handlebars
and ureq dependencies reuse already-resolved versions; ureq handles TLS/downloads.

The released collector's frozen TLS dependencies fail current advisory checks.
Its library cannot join this workspace without changing krates' exact indexmap
2.9.0/petgraph 0.8.1 constraints, conflicting with existing maintained dependencies.
Do not downgrade Kedra or create a patch/fork/wrapper for notice collection. The
small installer downloads checksum-pinned official source, uses ordinary Cargo
update/advisory audit/install commands in an isolated directory and retains that
lockfile. It installs no system tool globally and executes no tool build before
the advisory audit succeeds. This trades bit-identical collector graphs across
dates for current compatible security updates, audited independently of the app.

Configured packaging is fail-closed on missing collectors, unresolved license
texts, wrong source checksums and unsupported covered-source provenance. Package
full copyright/license notices and MPL source archives, not merely a table of
package license names. Font and modified-Winit notices retain existing provenance.
