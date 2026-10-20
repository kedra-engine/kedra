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

The source links above identify current upstream archives; they are not a
complete binary-release compliance pipeline. Before publishing an installer,
archive the exact checksum-verified covered sources (including any local
modifications), bundle upstream license/copyright notices and this source-access
notice with the distribution, and verify recipient access. Other dependency-license
failures remain subject to
separate review, not automatic approval by these exceptions.

## Permissive dependency licenses

The reviewed native dependency graph also uses BSD-2-Clause, BSD-3-Clause, ISC,
MIT-0, Zlib and Unicode-3.0. These are explicitly admitted by `deny.toml`; preserve
the applicable upstream copyright, attribution and license notices when shipping.
See [ADR 0007](decisions/0007-dependency-policy.md) for policy rationale, exact
duplicate-version exceptions and the time-bounded unmaintained-parser exception.

## Source provenance and publication

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
