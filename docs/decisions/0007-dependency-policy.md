# ADR 0007: Reviewed native dependency exceptions

Status: active native dependency policy.

## Context and decision

The native dependency graph requires narrowly scoped license, duplicate-version
and advisory exceptions. Retain registry/source checks, wildcard denial,
advisory scanning and default duplicate denial. Do not use subtree exclusions.

Local workspace dependencies specify the exact aligned workspace release version
(`version = "=0.1.3"`) alongside their path.
Wildcard path dependencies are not allowed.

## Licenses

Allow the permissive BSD-2-Clause, BSD-3-Clause, ISC, MIT-0, Zlib and Unicode-3.0
licenses in addition to MIT/Apache. These require preserving applicable notices;
they do not impose project-wide copyleft. Keep MPL-2.0 limited to exact versions
of the five CSS packages plus `colored` 2.2.0 (CLI diagnostics) and `option-ext`
0.2.0 (home-directory discovery). [Third-party obligations](../third-party.md)
records source availability and binary-distribution responsibilities. A green
license check is not proof that an installer contains all required notices.

The owner approves BSL-1.0 only for registry `xxhash-rust 0.8.19`, reached through `kedra_cache` →
`cacache 13.0.0` → `ssri 9.2.0`. This is the permissive **Boost Software License 1.0**, not Business
Source License. Its text permits use, modification and distribution; retain its copyright and
license in source copies and derivative works unless solely machine-executable object code. Preserve
the notice in generated binary notices as well. No global BSL allowance is granted; upgrades require
renewed review. The hash is for disposable cache integrity, not an authentication or
source-admission authority. See [ADR 0050](0050-project-cache-and-static-thumbnails.md).

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
| base64 0.21.7 | ssri 9.2.0 requires 0.21 versus Bevy glTF's 0.23 |
| sha1 0.10.7 | cacache 13.0.0 requires 0.10 versus cargo-packager's 0.11 |
| unicode-width 0.1.14 | miette 5.10 through cacache/ssri requires 0.1 versus Bevy tooling's 0.2 |
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
| variadics_please 1.1.0 | BMS 0.23 procedural binding helpers versus Bevy 0.20's 2.x |
| ordered-float 2.10.1 | mlua's serde-value 0.7 adapter versus WGPU's 5.5 line |
| thiserror / thiserror-impl 1.0.69 | smithay/calloop/ndk versus Bevy's 2.x |
| tiny-skia / tiny-skia-path 0.11.4 | sctk-adwaita versus resvg/usvg's 0.12 |
| windows-sys 0.45.0 / 0.52.0 / 0.59.0 | JNI / Winit / rustix/colored versus current Windows adapters |
| windows-targets and seven architecture libraries 0.42.2 | windows-sys 0.45 versus 0.52.6 target support |

The owner approves the three cache-related lower identities above. These are
unconditional dependencies of the pinned synchronous-only `cacache`/`ssri`
configuration, not optional features. Their semver constraints cannot unify with
the parallel lines. Retain the maintained storage library rather than introduce
a custom index/integrity format or patch unrelated upstream APIs solely to dedupe.
See [ADR 0050](0050-project-cache-and-static-thumbnails.md) for the cache boundary.

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
`cargo xtask dependency-policy` enforces the UTC deadline, exact parser/wrapper
versions and allowed immediate caller. This stage is part of `cargo xtask check`.
Remove the exception when upstream changes, or explicitly review it before the
deadline; do not automatically extend it. Linux exposure remains real even though
the current macOS UI does not use these caption fonts.

## Unmaintained BMS build-time diagnostics

The owner approves a narrow exception for
[RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173), which reports
`proc-macro-error2` as unmaintained, not a known vulnerability. No patched release
exists. It executes as build-time procedural-macro diagnostic support, not as the
Luau VM or gameplay interpreter. Build-time code still has developer/CI authority;
this distinction reduces the runtime concern but is not a security guarantee.

The reviewed registry-only chain is BMS 0.23.0 → `bevy_mod_scripting_lua 0.23.0` →
`mlua 0.11.6` → `mlua_derive 0.11.0` → `proc-macro-error2 2.0.1`. BMS's Lua backend
unconditionally enables mlua macros, so disabling default features does not remove
this path. Its only other admitted immediate consumer is
`bevy_mod_scripting_functions 0.23.0`, which also depends on the Lua backend.

Admit this advisory ID until **2026-12-26 UTC**, matching the existing dependency
review checkpoint. The policy checks exact versions, registry sources and all
immediate callers along the chain. Additional versions/sources/callers, a future
vulnerability advisory or a later review date do not inherit approval. Prefer an
upstream maintained resolution; remove the exception when no consumer needs it,
or explicitly review before the deadline rather than extending automatically.

Shared Luau execution is implemented under [ADR 0029](0029-luau-runtime-evaluation.md).
The required policy command always checks the main lock and review deadline. For the
canonical runtime-only game in `tests/fixtures/test_project`, supply the actual
resolved lock from its materialized copy explicitly before relying on the advisory
ignore. The commands below assume that copy is at `target/test-project`; substitute
the actual materialization directory consistently if different:

```sh
KEDRA_LUAU_LOCKFILE="$PWD/target/test-project/Cargo.lock" \
  cargo xtask dependency-policy --non-interactive
cargo deny --manifest-path target/test-project/Cargo.toml --locked check licenses advisories
```

Both commands must pass for the same unchanged lock. Pinned cargo-deny 0.19.0
can ignore an advisory ID but cannot express this chain or expiry; its success
alone is insufficient. A missing or unreadable explicitly supplied lock fails.
No generated/private fixture lock is required by ordinary CI. BMS's main-runtime
chain is covered by the mandatory main-lock policy as well.

## Unmaintained profiling build-time macro

The owner approves a narrow exception for
[RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436): `paste` is
unmaintained, with no patched release. Registry `perfetto-sdk 1.2.1` depends on
`paste 1.0.15` for procedural identifier generation. The reviewed SDK's only direct
workspace consumer is `kedra_profiling 0.1.3`; recording remains opt-in. Macro code
executes with build/CI authority even though it is not runtime trace processing.

Admit only this advisory ID until **2026-12-26 UTC**. The dependency-policy command
enforces the review deadline. Cargo and cargo-deny own package resolution, source,
license and advisory admission; tests do not repeat package pins or parse a Cargo
dependency tree. Cargo-deny's ignore applies to the advisory ID, not an individual
caller chain. The chain above is the reviewed scope, not an automated reachability
guarantee: version/source/caller changes require owner review. New vulnerability
advisory IDs remain errors. Remove the exception when the SDK uses a maintained
dependency, or explicitly re-review before the deadline; do not extend it
automatically. No fork or replacement profiler is authorized by this exception.

## Verification

Run `cargo deny --locked check` against the complete locked all-feature graph and
`cargo xtask check --non-interactive`. Policy regressions must reject broadened
exceptions, wrong versions/callers and expired review dates. Native rendering is
verified separately; dependency-policy acceptance is not platform certification.
