# Packaging and licenses

Use `package` when you need distributable artifacts as well as a runnable build. It always
builds release, inspects the actual package contents and only then records the successful
artifact inventory. Creating files is not the same as publishing or certifying a release.

## Native prerequisites

Start with the [build prerequisites](build-and-run.md) and the application's
[metadata](configuration.md). Use a native host matching the selected target:

| Host/target | Configured outputs | Additional package prerequisites |
| --- | --- | --- |
| Windows x64 MSVC | NSIS `.exe` and portable `.zip` | Maintained 7-Zip, `7z` on PATH; upstream NSIS helpers |
| Linux x64 GNU | `.AppImage` and portable `.tar.gz` | Ubuntu 24.04 provenance environment, `pax-utils`/`lddtree`, `dpkg-query`, `tar`, distro copyright/common-license files |
| macOS ARM64 or Intel | `.dmg` containing `.app` | System `hdiutil` and `ditto`, plus `codesign`/`plutil` |

The shared tool accepts both macOS architectures on their native hosts. The editor's
distribution selection is Apple Silicon only, alongside Windows/Linux x64; this does not
establish Intel editor release availability. `--target` is not a cross-compilation option.

Linux ownership discovery uses the distro's `/usr/bin/python3 /usr/bin/lddtree`, not a
virtual environment's Python. Other distributions need an explicit provenance adapter,
not guessed license files. Docker on native x86-64 Linux is needed for the separate
recipient-relink test, not ordinary package creation.

The maintained packager can download helpers into its OS cache and execute them. Some
upstream helper downloads are not cryptographically pinned: this is not a hermetic build.
The native worker runs from the application's manifest directory, with unattended
`CI=true` set only for that child. Relative native settings therefore belong to that app.

## Set up the configured license collector

When `license-config` is present, build and package require `cargo-about 0.9.2`. A missing
collector is an error, not an automatic installation. The explicit setup command is:

```sh
cargo kedra tools install-about --output /OUTSIDE_CARGO_WORKSPACE/about-tool
```

Replace that placeholder with a new user-local tool directory outside **any** Cargo
workspace; do not use the example build-output subtree. Setup needs network access,
Cargo and cargo-deny. It verifies the upstream source archive checksum, resolves an
isolated updated tool lock, audits advisories before compilation and installs that graph
with `--locked`. Add the resulting `bin` directory to PATH yourself.

This explicit command downloads and compiles a tool; it is not a read-only prerequisite
probe. Existing installation directories are preserved. Keep its resolved lock as evidence:
the audited graph can differ between installation dates. Do not downgrade application
dependencies to fit the collector's frozen upstream graph.

## Create packages

After setup, from the repository root on a supported native host:

```sh
cargo kedra package --package kedra --output target/documentation/examples/build-and-distribution/editor-package-01
```

The output must be new, and `--output` is required for package. The host target is inferred;
an explicit matching `--target` is also accepted. `package` has no `--release` flag because release
compilation is mandatory. It does not launch the packaged application.

The builder prepares the runnable assembly, then creates and inspects the native packages.
NSIS is extracted with 7-Zip, never executed as an installer during ordinary packaging.
DMGs are verified and mounted read-only, and the copied app's identity/signature is checked.
Portable archives are extracted and checked against staging. The actual AppImage receives
additional runtime and bundled-library provenance checks.

Final artifact basenames use `<package>-<version>-<target>` followed by the format's
extension. The output also includes:

- `build-result.json`: verified runnable application handoff.
- `artifact-inventory.json`: schema `1`, application/version/target, source revision,
  artifact basenames, formats, byte sizes and SHA-256 hashes.
- `SHA256SUMS`: checksums for the recorded package artifacts.

Intermediate `application/`, `packages/` and native configuration/report files may remain
for diagnosis. Their presence alone does not prove success. Only use a successful command's
verified results, and preserve the complete resources/notices inside each artifact.

## Preserve licenses and sources

Kedra is MIT OR Apache-2.0; dependencies and assets retain their own terms. The Cargo
license field is not a license grant for unrelated third-party content. Review fonts,
artwork, modified dependencies and each application's own materials separately.

Configured notices are copied into `licenses/`. With `license-config`, cargo-about
collects target-specific dependency license information and emits full copyright/license
text in `dependency-notices.html`. MPL-2.0 crates.io dependencies also get original
`.crate` archives under `licenses/covered-sources/`, verified against `Cargo.lock`.
Unknown non-registry covered-source provenance fails; a registry archive cannot stand in
for modified Git/local covered files. Preserve recipients' source access and MPL rights.

Linux additionally includes:

- `licenses/native-libraries/`: distro ownership/version records, full copyright files
  and referenced common-license texts. Bundled ELF identity must match a unique owner.
  Only reviewed Brotli, bzip2, libffi, libpng and Wayland notice identities are admitted;
  a changed notice or new library family requires review.
- `licenses/appimage-runtime/`: pinned runtime/libfuse/squashfuse source materials,
  the upstream libfuse patch, full notices, static objects/headers and recipient rebuild
  instructions. The selected runtime prefix and AppRun are checked against reviewed inputs.

The AppImage runtime includes LGPL-covered libfuse and GCC startup/runtime code with the
GCC Runtime Library Exception. Preserve full terms, rebuild/relink materials and the
recipient's right to modify/debug LGPL components. Compiler startup objects rely on the
normal recipient toolchain; they are not all supplied by the library archives. The
separate relink check retains actual link/toolchain evidence and does not claim identical
recompilation. Native relink and installed-material acceptance remain release prerequisites,
not something source inspection alone proves.

The editor also uses a modified public Winit fork. Preserve its Apache-2.0 notices and
access to the exact modified source. Fonts retain their OFL notices; artwork retains its
own provenance. Generated dependency notices do not replace these obligations or legal review.

## Trust, failure and release limits

macOS bundles default to complete ad-hoc signing, not Developer ID signing or notarization.
Signature integrity does not establish Gatekeeper trust. Windows packages remain unsigned.
The `v0.1.0` prerelease macOS asset has a known bundle-signature defect; it requires local
re-signing and is not evidence that the current builder's bundle path passed acceptance.
Do not disable operating-system protections globally to run a package.

A clean Git workspace whose root matches the Cargo workspace supplies `source_revision`.
Dirty, nested-workspace or non-Git builds record `null` and are not publication-eligible.
Checksums detect changed bytes; they do not authenticate the publisher. A clean revision
and an inventory alone still do not authorize release publication.

If native tools, source downloads, notices, signatures or inspection fail, keep the
diagnostics and partial output, repair the cause and retry into a new directory. Do not
skip provenance checks, omit sources or lower the policy to obtain an inventory.

Installation and packaged application behavior remain unverified for this example.
Before sharing a release, independently verify installed resource lookup, notices/source
access, native relinking where required, installation/uninstallation, rendered frames and
physical input on each intended platform. macOS 15 is the editor's configured minimum;
Linux glibc/GPU/FUSE compatibility and the Windows OS floor still need native acceptance.
AppImage is not universal system compatibility. Current hosted package/publication and
recipient-relink success are not established by these manual examples.

Contributors changing assembly, provenance or release publication should use the
[distribution contract](../../contributing/contracts/distribution.md) for the required guarantees
and independent-consumer checks. The canonical
[third-party obligations (English)][third-party] and original license texts remain the legal
authorities; this recipe is not an alternative license policy.

[third-party]: https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md
