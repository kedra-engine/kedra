# ADR 0043: Narrow CSS matching and invalidation patches

Status: adopted; native shared acceptance remains separate.

## Decision

Keep Bevy Flair 0.9.0 and its `selectors` 0.38.0 engine. Use a pinned Git fork
of `bevy_flair_style`, shared by the editor and independent native project, to
reject absent mandatory subject class/ID keys before relational selector work.
Use `SelectorIter` and `Element::has_class`/`has_id`, not a selector parser or
replacement matcher. The iterator's first compound is the only inspected scope;
unknown components and nested alternatives always fall through to stock matching.
Candidates retain the complete original matching, cascade and dependency flags.
There is no cross-frame cache, new stylesheet language or game/editor policy.

Inline declarations are not selector inputs. An inline-only change recalculates
its owner but does not propagate relational selector flags to siblings, descendants
or ancestors. Changed selector data or independently pending selector recalculation
still propagates those flags. Variable and inherited-property changes retain the
existing downstream value propagation; transitions and final-value application
are unchanged. Equal matching block IDs alone are not sufficient to skip value work.

The owning internal selector method has no public extension point. Retained
hierarchy publication and selector matching are separate responsibilities: stable
hierarchy writes do not eliminate relational selector work. CSS rewrites that
remove required selectors, focus, clipping or theme boundaries are not acceptable
substitutes. Cache sharing is not part of this patch: it would enter additional
upstream filter paths and requires its own correctness evaluation.

## Source, maintenance and boundaries

Use <https://github.com/kedra-engine/bevy_flair> at
`9be09fb26caa0787afdb63617a82d62eaae0ab00`, based on upstream
`efc44a46be8f5adf772089dab8535b231c6d89dd`. The base matches the packaged VCS
identities and Rust sources of all five Flair 0.9.0 registry packages. The
`bevy_flair_style` base archive SHA-256 is
`bb389b8dab2848313d4c9ac2f18cdd61815badb076320474a8844aae5c1ffdc3`.
Only mandatory-subject rejection, the inline-only relational invalidation guard
and their private test modules differ. Preserve upstream version/API, module
layout, lints and MIT license.

Pin `bevy_flair`, `bevy_flair_core`, `bevy_flair_core_macros`, `bevy_flair_style`
and `bevy_flair_css_parser` to that one source in both Cargo roots. Their upstream
path dependencies must not create parallel registry/Git property registries and
types. Other applications must opt into the same root overrides; Cargo patches
are not inherited through dependencies. The development clone is not a required
build input, and no third-party package is vendored in Kedra. Transitive MPL
packages keep the obligations in [third-party notices](../third-party.md).
Publishing further changes or submitting an upstream PR requires separate approval.

Every Flair upgrade must review the patch against the verified registry source,
rerun its private and Kedra integration suites, and remove the override when an
upstream equivalent is available. Binary notices must include the upstream
license. Do not weaken dependency, file-growth or visual-reference lock policies
to accommodate the source override; changed screenshot graphs need owner review.

## Verification

Private engine tests own early-rejection/no-traversal and exact-matcher equivalence,
including negative cases, Unicode/case, combinators, nested alternatives and pseudo
elements. Private invalidation tests own inline-only isolation, simultaneous
selector changes and preservation of pending recalculation/reset. Public Kedra
integration owns live class/focus/hierarchy and independent stylesheet behavior,
inline geometry, variables, explicit/default inheritance and transitions.
Retained docking tests own unchanged edge/order ticks and
transfer/retirement/failure safety. The existing editor and independent native game
own assembled input/readback checks; their unresolved failures remain blockers.

Run the private engine suite in a clone of the pinned fork with
`cargo test -p bevy_flair_style --lib --all-features --locked`.
This explicit upstream suite is separate from the normal Kedra workspace checks;
the shared integration regression runs with `cargo xtask check`.

Performance evidence belongs under `target/`, with matched sources, features,
deliveries and workload boundaries. Sampling, CSS elapsed scopes, allocations,
process CPU, renderer frames and physical latency are distinct evidence. One
host's result does not establish game speedups or platform certification.
