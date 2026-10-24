# Contributing to Kedra

Change Kedra's crates, editor integration, tools and documentation using the routes below.
To use Kedra rather than change it, start with the
[Manual](../manual/introduction/index.md).
Kedra's APIs and source formats are unstable; a useful contribution protects an explicit
contract without implying that planned editor or game workflows already exist.

The repository's [contribution policy][contributing], [contributor rules][agents] and
[architecture contract][architecture] remain authoritative. These pages help you apply them;
they do not replace policy or authorize implementation, publication or a release.

## Choose a route

| Your question | Read next | Expected result |
| --- | --- | --- |
| Where does this behavior belong? | [Codebase map](codebase-map.md) | A production owner and its local contracts |
| Which lifecycle or failure guarantee am I changing? | [Engineering contracts](contracts/index.md) | The invariants, failure behavior and responsible module |
| Should I extend a mechanism or create a crate? | [Architecture and changes](architecture-and-changes.md) | A bounded design with justified dependencies |
| What should fail before my fix? | [Tests and review](tests-and-review.md) | An owner-level regression and honest evidence |
| How do I change both documentation languages? | [Documentation contributions](documentation.md) | Paired, verified guidance with provenance |

## Prepare a working checkout

Use [Prerequisites and source setup](../manual/getting-started/index.md) for Git, rustup,
the pinned Rust toolchain and platform linker/development libraries.
Keep `Cargo.lock`; ordinary checks use locked resolution.
The first dependency fetch needs network access. No private service, AI account or
credential is required for the normal Rust contributor loop.

Headless checks do not require a GPU. Editor interaction needs a working native desktop;
building the binary is not proof of native input or rendering. Documentation has its own
public toolchain, described in [Documentation contributions](documentation.md).

Before editing, read the repository entry documents and the affected module's README or
module-level Rust documentation. Not every crate has a README: its `src/lib.rs` and the
linked engineering guide can be the local entry. Inspect the current tree, not just an
earlier branch, search excerpt or design proposal.

These read-only commands help establish context from the repository root:

```sh
git status --short
rustup show active-toolchain
git diff --stat
```

Preserve unrelated work in a shared or dirty checkout. A clean-looking output directory
does not imply that existing sources or fixtures are disposable. Agree on file ownership
when several contributors work in the same tree.

## Describe the change before expanding it

Start with the user-visible problem and the smallest observable failure. For a bug report,
record revision, platform/toolchain, exact commands, expected/actual behavior and a small
reproduction with redacted logs. Keep private security details out of public reports;
follow the repository's [security policy][security].

Discuss substantial features and architectural changes before implementation. A proposal
should identify the owner, alternatives, compatibility impact, failure behavior and the
environments needed to verify it. A new dependency is not justified solely because it
makes one call site shorter. A planned ADR is not evidence that an API exists.

For example, “a stale docking candidate must not replace the current topology” identifies
a model contract. “Dragging feels broken” needs a reproduction that separates topology,
retained projection, editor coordination and actual native event delivery. The
[codebase map](codebase-map.md) helps turn that symptom into a bounded change.

## Work through one contribution

1. Find the production owner and read its current contract and test suite.
2. Define the rejected/failure path as well as the successful result.
3. Choose the least expensive environment that can expose that failure.
4. Change the mechanism and its direct consumers, without a duplicate compatibility path.
5. Update public API docs and affected English/Russian guidance together.
6. Run the focused check, then the required review checks for the change.
7. Report actual results, unrun checks, native limits and remaining risks.

Do not mutate canonical project fixtures to make a local experiment convenient. Native
scenarios that change project data use complete private copies; see the
[testing guide][testing]. Do not import prototype content, assets or private infrastructure
without explicit approval and provenance review.

## Prepare the review handoff

Use a focused branch and change request. Explain the behavior and why the owner is right,
not only which files changed. Link the issue or ADR when relevant, state compatibility
effects and include the exact commands and exit results on the reviewed sources.

`cargo xtask check` is the Rust review route, not a native acceptance or documentation-site
build. [Tests and review](tests-and-review.md) explains its stages and separate checks.
Skipped or interrupted checks remain unverified. A local pass cannot establish hosted CI
success or live branch protection.

Contributions use English for public collaboration and follow the [Code of Conduct][conduct].
Review material AI assistance, disclose it in the change request and verify every generated
claim against source. Optional [repository search][search] can assist orientation; it is
not required by CI and never replaces source inspection or human approval.

The owner must inspect and explicitly approve changes before a commit under the
[contributor rules][agents]. Pushes, permissions, releases and publication need separate
authorization. Consult [quality policy][quality] for merge requirements and
[repository administration][hosting] for administrative boundaries.

[contributing]: https://github.com/kedra-engine/kedra/blob/main/CONTRIBUTING.md
[agents]: https://github.com/kedra-engine/kedra/blob/main/AGENTS.md
[architecture]: https://github.com/kedra-engine/kedra/blob/main/docs/architecture.md
[security]: https://github.com/kedra-engine/kedra/blob/main/SECURITY.md
[testing]: contracts/testing.md
[conduct]: https://github.com/kedra-engine/kedra/blob/main/CODE_OF_CONDUCT.md
[search]: https://github.com/kedra-engine/kedra/blob/main/docs/ai-search.md
[quality]: https://github.com/kedra-engine/kedra/blob/main/docs/quality.md
[hosting]: https://github.com/kedra-engine/kedra/blob/main/docs/hosting.md
