# Testing

Choose verification by the failure you need to detect, not by the largest command available.
Use model and API tests for product contracts, native scenarios for application interaction,
and repository checks for policy. Passing one does not replace the others.

Run repository commands from its root.
Native commands can build applications, create windows, change focus and inject input.
Read their prerequisites before launching them.

## Choose the cheapest sufficient environment

Name the module responsible for the behavior, the expected result and the conditions it
depends on. Then identify what could go wrong and which existing test would detect it.

| Question | Start here | Evidence boundary |
| --- | --- | --- |
| Does a rejected topology change preserve the original model? | Pure `kedra_docking` contracts | Values and ownership, not native dragging |
| Does a plugin retain the correct projected entities? | Headless owner-crate tests | Registration/scheduling, not production pixels |
| Does publication preserve sources on failure? | Owner's real filesystem contract | I/O and published state, not editor usability |
| Does the assembled editor route a gesture correctly? | Native feature scenario | Injected input, live state and renderer fences |
| Does a listed kit control match its reviewed policy? | Component corpus | Declared profile, typography and image comparison |
| Does physical dragging work across monitors? | Separate physical platform acceptance | Actual OS input and frames on that setup |
| Are dependency layers and Rust diagnostics valid? | Repository checks | Repository policy, not feature acceptance |

Do not replay a complete domain error matrix through a native window.
Use higher-level tests only when they expose a different integration failure.
Compilation cannot replace input/frame evidence; a screenshot cannot replace a persistence test.

## Find the contract owner

Keep primary contract tests with the implementation they test. Application scenarios check
how those implementations work together rather than replacing their lower-level tests.
For Kedra changes, follow [Tests and review](../../contributing/tests-and-review.md) and the
[authoritative testing ownership map][testing-owner].

Source-save API tests do not imply that editor Save or visual editing exists.
The editor remains read-only for these workflows.

## Put a regression at its owner

When changing Kedra, use the contributor guide's
[placement and failure-path workflow](../../contributing/tests-and-review.md).
The [testing contract][testing-owner] owns the detailed placement rules.
Source/persistence failures need evidence of preserved state; native input and frame evidence
answer different questions and do not replace those checks.

## Keep evidence categories separate

1. **Source/test inspection:** explains an implemented path or assertion; proves no execution.
2. **Pure/headless execution:** proves the tested model or host contract in that environment.
3. **Synthetic layout:** proves response to supplied geometry, not production layout quality.
4. **RPC and injected events:** prove observed state/delivery, not physical OS interaction.
5. **Renderer readback:** proves delivered image bytes; a fence does not assert scene content.
6. **Pixel assertion:** checks explicit image content under the declared comparison policy.
7. **Physical input and human review:** evaluate real interaction and acceptance on a setup.

These are complementary forms of evidence, not interchangeable levels of a single score.
Human acceptance must name its scope; reviewing one control does not certify the editor.

## Record a useful result

Keep the exact command, revision and dirty changes, OS/architecture/toolchain, selected cases,
exit status, artifact location and limitations. Distinguish failed setup, failed cases,
unrun cases and interrupted execution. A partial selection is not complete acceptance.

Preserve diagnostic evidence before a retry. Flakiness needs a cause, owner and repair plan,
not unlimited retries, deleted tests, relaxed budgets or newly blessed references.
Review logs for credentials and personal paths before sharing them.

## Continue by task

- [CLI task map](../cli/index.md): choose the command owner before dispatching a check.
- [Automation](../automation/index.md): connection authority, observations and operation receipts.
- [Build and distribution](../build-and-distribution/index.md):
  application assembly versus acceptance.
- [Repository checks](repository-checks.md): focused Cargo tests, Rust gate and feature builds.
- [Native scenarios](native-scenarios.md): desktop preparation, explicit launch and recovery.
- [GUI performance](performance.md): bounded RPC/Luau metrics, timelines and native profiling.
- [Visual verification](visual-verification.md): source inspection, captures, diffs and review.
- [Getting started](../getting-started/index.md): native build and startup prerequisites.
- [Assets](../assets/index.md): why a valid source and a matching delivery are different inputs.

[testing-owner]: ../../contributing/contracts/testing.md
