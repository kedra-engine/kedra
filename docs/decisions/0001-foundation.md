# ADR 0001: Public engineering foundation before engine migration

- Status: accepted direction; implementation is subject to verification
- Date: 2026-10-18

## Context

Kedra is becoming an international open-source project hosted at
<https://github.com/kedra-engine/kedra>.
Public contributions require reproducible verification, clear provenance and
separation from private infrastructure. Prototype/research material is not a
released feature or an automatic source/license dependency.

## Decision

- Use `kedra-engine/kedra`, a modular Rust workspace and MIT OR Apache-2.0.
- Use English for public code, documentation and collaboration.
- Keep a small, portable Rust xtask as the local/CI check entry point.
- Enforce dependency layers, tests, formatting, lint, documentation and supply-chain policy.
- Keep human review and server-side branch protection distinct from executable CI.
- Build the foundation without importing engine code or private infrastructure.
- Plan game runtime targets for Linux/Windows/macOS desktop, web and Android/iOS
  mobile. Keep the editor desktop-only; certify exact targets later.

## Alternatives

Copying the old repository preserves history but also imports private assumptions
and unverified public content. Splitting every module into separate repositories
complicates atomic API changes. Requiring proprietary AI tooling excludes contributors.

## Consequences

No working engine is promised by this bootstrap. A SHA pin does not eliminate
supply-chain risk; dependency updates require review. CI cannot prove arbitrary
correctness. Future engine migration must bring tests, licensing/provenance,
documentation and consumer/target evidence with each coherent vertical slice.
