# ADR 0007: Reviewed native dependency exceptions

Status: active native dependency policy.

## Context and decision

The native dependency graph requires narrowly scoped license, duplicate-version
and advisory exceptions. Retain registry/source checks, wildcard denial,
advisory scanning and default duplicate denial. Do not use subtree exclusions.

Local workspace dependencies specify `version = "=0.1.0"` alongside their path.
Wildcard path dependencies are not allowed.

## Licenses

Allow the permissive BSD-2-Clause, BSD-3-Clause, ISC, MIT-0, Zlib and Unicode-3.0
licenses in addition to MIT/Apache. These require preserving applicable notices;
they do not impose project-wide copyleft. Keep MPL-2.0 limited to exact versions
of the five CSS packages plus `colored` 2.2.0 (CLI diagnostics) and `option-ext`
0.2.0 (home-directory discovery). [Third-party obligations](../third-party.md)
records source availability and binary-distribution responsibilities. A green
license check is not proof that an installer contains all required notices.

## Incompatible duplicate versions

Cargo metadata and inverse trees show different upstream semver lines, not
redundant compatible versions that `cargo update` can unify. Patching a major or
pre-1.0 minor across those constraints is not a valid deduplication strategy.
The exact lower releases in `deny.toml` are temporarily admitted for duplicate
detection only; their licenses, advisories and sources remain checked. Versions
outside that exact list fail. Review each skip on upstream/lockfile upgrades and
remove it when its consumer moves. No broad `skip-tree` or default warning level.

| Lower release admitted | Immediate upstream constraint / parallel line |
| --- | --- |
| bitflags 1.3.2 | core-graphics/defmt/Redox versus Bevy's 2.x |
| block2 0.5.1; objc2 0.5.3; objc2-app-kit/foundation 0.2.2 | Winit 0.30 / AccessKit versus rfd/arboard/wgpu's newer ObjC stack |
| cpufeatures 0.2.17 | sha2 0.10 versus blake3's 0.3 |
| downcast-rs 1.2.1 | wayland-backend versus Bevy's 2.x |
| foldhash 0.1.5; hashbrown 0.15.5 | petgraph 0.8 versus Bevy/fontique's newer tables |
| hashbrown 0.16.1 | Bevy platform/AccessKit/gpu-allocator versus fontique/wgpu's 0.17 |
| getrandom 0.3.4; r-efi 5.3.0 | ahash versus uuid/tempfile's newer entropy stack |
| itertools 0.14.0 | Bevy render/curve versus Flair/WESL's 0.15 |
| jni 0.21.1; jni-sys 0.3.1 | cpal/ndk versus android-activity's newer JNI stack |
| rustix 0.38.44; linux-raw-sys 0.4.15 | Winit/calloop/smithay versus Wayland/tempfile's newer stack |
| miniz_oxide 0.8.9 | png versus flate2's 0.9 |
| redox_syscall 0.4.1 / 0.5.18 | Winit / parking_lot_core versus libredox's 0.9 |
| rustc-hash 1.1.0 | naga/wgpu versus Flair's 2.x |
| syn 2.0.119 | Bevy macros versus serde/clap/bytemuck's 3.x |
| thiserror / thiserror-impl 1.0.69 | smithay/calloop/ndk versus Bevy's 2.x |
| tiny-skia / tiny-skia-path 0.11.4 | sctk-adwaita versus resvg/usvg's 0.12 |
| windows-sys 0.45.0 / 0.52.0 / 0.59.0 | JNI / Winit / rustix/colored versus current Windows adapters |
| windows-targets and seven architecture libraries 0.42.2 | windows-sys 0.45 versus 0.52.6 target support |

## Unmaintained Linux caption parser

RUSTSEC-2026-0192 marks `ttf-parser` as unmaintained, with no patched versions. It
is not a vulnerability advisory. The all-target path is Winit → sctk-adwaita →
ab_glyph → owned_ttf_parser 0.25.1 → ttf-parser 0.25.1. Bevy's Wayland feature
enables this decoration stack; it cannot be removed independently without
altering the upstream backend or dropping Wayland support. Fontations is the
maintained replacement, but cannot be substituted for ab_glyph's parser API.

Admit only this advisory ID until review on **2026-12-26 UTC**, without changing
the treatment of future vulnerability IDs or other unmaintained packages.
Pinned cargo-deny 0.19.0 lacks native expiry/reachability fields, so an
ordinary Rust policy regression enforces the UTC deadline, exact parser/wrapper
versions and allowed immediate caller. The test is part of `cargo xtask check`.
Remove the exception when upstream changes, or explicitly review it before the
deadline; do not automatically extend it. Linux exposure remains real even though
the current macOS UI does not use these caption fonts.

## Verification

Run `cargo deny --locked check` against the complete locked all-feature graph and
`cargo xtask check --non-interactive`. Policy regressions must reject broadened
exceptions, wrong versions/callers and expired review dates. Native rendering is
verified separately; dependency-policy acceptance is not platform certification.
