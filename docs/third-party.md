# Third-party license obligations

Kedra's project license is MIT OR Apache-2.0. Dependencies retain their licenses.
MPL-2.0 is allowed only for exact versions of the CSS stack and CLI/home-directory
utilities in `deny.toml`, not as a global license allowance for other third-party
code or assets.

## MPL-2.0 dependencies

These upstream packages are consumed without local source modifications:

| Package | Version | Covered source archive |
| --- | --- | --- |
| cssparser | 0.37.0 | <https://crates.io/api/v1/crates/cssparser/0.37.0/download> |
| cssparser-color | 0.5.0 | <https://crates.io/api/v1/crates/cssparser-color/0.5.0/download> |
| cssparser-macros | 0.7.1 | <https://crates.io/api/v1/crates/cssparser-macros/0.7.1/download> |
| dtoa-short | 0.3.5 | <https://crates.io/api/v1/crates/dtoa-short/0.3.5/download> |
| selectors | 0.38.0 | <https://crates.io/api/v1/crates/selectors/0.38.0/download> |
| colored | 2.2.0 | <https://crates.io/api/v1/crates/colored/2.2.0/download> |
| option-ext | 0.2.0 | <https://crates.io/api/v1/crates/option-ext/0.2.0/download> |

License text: <https://www.mozilla.org/MPL/2.0/>.

MPL applies to covered files, not to every file linked into a larger work. When
distributing a binary containing these packages, preserve their copyright and
license notices, inform recipients how to obtain the covered source under MPL,
and make that source available. Any modifications to covered files must also be
provided under MPL. Do not impose terms that restrict recipients' MPL rights.

The source links above identify upstream archives. Configured native packaging
now bundles full cargo-about copyright/license notices and original MPL-covered
crates.io archives verified against Cargo.lock under `licenses/covered-sources`.
Non-registry covered sources fail pending explicit support; do not omit local
modifications from their required source distribution. Before publication, verify
recipient access to the installed notices and archives. Other dependency-license
failures remain subject to
separate review, not automatic approval by these exceptions.

## Permissive dependency licenses

The reviewed native dependency graph also uses BSD-2-Clause, BSD-3-Clause, ISC,
MIT-0, Zlib and Unicode-3.0. These are explicitly admitted by `deny.toml`; preserve
the applicable upstream copyright, attribution and license notices when shipping.
See [ADR 0007](decisions/0007-dependency-policy.md) for policy rationale, exact
duplicate-version exceptions and the time-bounded unmaintained-parser exception.

## Source provenance and publication

### Native Linux components

Cargo license collection covers Rust crates only. Linux packages additionally
preserve full distro-native copyright records and referenced common-license
texts, and include an AppImage runtime source/relink kit. See
[distribution](distribution.md) for locations and acceptance limits.

The reviewed bundled ELF source families are Brotli (MIT), bzip2 (its BSD-style
license), libffi (MIT/Expat runtime implementation), libpng (libpng license) and
Wayland (MIT/X11). Full distro copyright files are preserved, including notices
for build/test/packaging sources that are not part of those runtime libraries.
Exact copyright hashes gate admission; unknown or changed terms require review.
This is intentionally not a custom parser guessing which DEP-5/SPDX clauses apply
to a compiled library. lddtree/dpkg-query own dependency/ownership discovery.

The AppImage runtime's libfuse 3.15.0 is LGPL-2.1 and has the AppImage project's
2024-11-24 fusermount-discovery patch. The full original source, exact patch,
LGPL text, complete runtime source and relink inputs accompany Linux packages.
musl, squashfuse, zstd, zlib and mimalloc retain their own notices. The included
Alpine APKs preserve corresponding linkable objects/headers; supplemental upstream
musl/zlib source archives are not represented as Alpine's patched sources.
Modification and reverse engineering for debugging LGPL modifications remain
permitted. Native relink and installed-material acceptance must pass before release.
These safeguards provide compliance evidence, not an independent legal opinion.

The native runtime also contains GCC startup/runtime code under GPL-3.0 with
the GCC Runtime Library Exception 3.1. Preserve both full texts, record the original
GCC/clang versions and retain the recipient rebuild's actual link map/toolchain
inventory. The kit explicitly relies on the LGPL standard-toolchain exception
for compiler-provided startup objects rather than claiming those are included
in the other libraries' APKs. AppRun's detected GCC startup/compiler versions are
older and use the same runtime-exception terms; its glibc is dynamically supplied
by the host, not bundled inside that helper.

AppRun is a reviewed MIT-licensed mirrored helper. Its copyright notice is retained
in the accompanying pinned AppRun.c; the mirrored binary is hash-verified. No
bit-identical recompilation claim is made for that older permissive binary.

The regression fixture `tests/fixtures/distribution/native-brotli.copyright` is an
unmodified MIT notice from Ubuntu's Brotli `1.1.0-2build2` package:
<https://changelogs.ubuntu.com/changelogs/pool/main/b/brotli/brotli_1.1.0-2build2/copyright>.

### Winit and project sources

Winit 0.30.13 is consumed from the public
[Kedra fork](https://github.com/kedra-engine/winit), pinned to
`948d27f0007602d2536a8974fd483c6fd33d19d3`. Its upstream base is
`e9809ef54b18499bb4f2cac945719ecc2a61061b` (`v0.30.13`) from
<https://github.com/rust-windowing/winit>. The delta is the macOS content-view drag
policy and registry `dpi` resolution described in
[ADR 0020](decisions/0020-macos-client-titlebar-input.md).
The upstream Apache-2.0 license and notices remain intact; distribute them with
binaries and preserve access to the exact modified source. No other fork changes
or dependency upgrades are included.

The local experimental prototype used for the scene/authoring work belongs to
Kedra's project owner. The owner confirms its provenance and authorizes inclusion
of that owner-authored work under Kedra's MIT OR Apache-2.0 project license.
The prototype's former `UNLICENSED` designation is not a distribution restriction
on those contributions to Kedra. No separate prototype-permission blocker remains
for the mechanisms described in [ADR 0005](decisions/0005-authored-scenes.md).

This confirmation covers owner-authored material, not third-party dependencies
or assets, which retain their applicable licenses and notices.

Fonts retain SIL OFL-1.1 notices. Figma artwork and font source identifiers are in
[asset provenance](../assets/README.md). Do not treat a design export or a Cargo
license field as an independent grant from a third-party rights holder.
