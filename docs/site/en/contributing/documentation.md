# Documentation contributions

Documentation changes are contributions to a public contract. Write what a reader can do
with the current implementation, distinguish plans from working paths and retain safety,
licensing and provenance facts. The [documentation plan][plan] owns migration scope;
[documentation infrastructure][infrastructure] owns the current tooling and local commands.

## Choose the right surface

| Surface | Reader's need | Avoid |
| --- | --- | --- |
| Manual | Understand/use existing product workflows | Organizing every chapter by crate |
| Contributing | Change Kedra with the right owner and evidence | Copying repository policy wholesale |
| Rust API docs | Exact types, methods, invariants and failure/lifecycle semantics | Replacing API contracts with a tutorial |
| ADRs | Durable decisions, alternatives and verification criteria | Treating proposed designs as implementation |
| Roadmap | Explicit future scope and completion criteria | Describing goals as current capability |
| Crate/module docs | Local responsibility, features, examples and errors | Unmaintained duplicate authorities |

Describe source-editing APIs without presenting them as a visual editor Save workflow:
general visual document editing, new-file publication and Play remain unimplemented.
Only documented Inspector source edits and single-existing-document Save are available.
A Figma target is a design source, not a screenshot of implemented UI.

## Verify the claim before writing the recipe

Read the current production owner and its manifest/features, then inspect the relevant
tests or real CLI parser. Verify flags, required resources and failure behavior. A test
assertion is source evidence until actually executed; a previous run does not certify
the current dirty revision. Record limitations instead of quietly dropping them.

For external APIs, inspect `Cargo.lock` and the exact resolved source/version. Query the
defining Bevy subcrate, not only its facade. Git patches and target-specific behavior
need source inspection when external documentation does not match. Optional repository
search can locate evidence but cannot make an unverified excerpt authoritative.

Commands should state their working directory and prerequisites. Distinguish a complete
runnable example from an excerpt requiring surrounding setup. Keep exact identifiers and
avoid invented fixture mutations or new test infrastructure to make examples look runnable.
Highlighting a RON block as Rust does not validate its schema.

## Maintain English and Russian together

English is canonical. Paired pages live under `docs/site/en/` and `docs/site/ru/` at matching
relative paths and English slugs. Translate explanations, titles, captions and navigation
idiomatically; retain commands, paths, API names and actual UI labels unchanged. Refer to
панель Hierarchy, for example, rather than inventing a translated control name.

Update both versions in the same reviewed change. Review the same prerequisites,
restrictions, failure cases and evidence limits in both languages. A counterpart file alone
does not establish translation quality. Follow the shared [glossary](../manual/glossary.md)
for recurring terms, introducing English terminology when it clarifies a technical concept.

Use relative `.md` links for existing site articles, with the corresponding locale path.
Link repository-only engineering contracts, ADRs and provenance originals through repository URLs;
label those exits English on Russian pages. Do not copy an engineering document into the
site merely to make it appear translated. Rustdoc remains English initially.

## Validate text and links without conflating builds

Use the [infrastructure reference][infrastructure] for Node/Bun pins, frozen
installation, local site generation and preview behavior. Native editor builds
do not require the documentation toolchain.

From `docs/`, `bun run build` is Rust-free and includes the paired Luau reference.
`bun run build:api` additionally invokes Cargo and includes generated English rustdoc;
see the [API guide](../api/index.md). This opt-in local route does not change CI.

With the pinned tools already available, these focused checks run from the repository root:

```sh
rumdl check docs/site/en/contributing docs/site/ru/contributing
lychee --offline --include-fragments --no-progress \
  --root-dir "$PWD/docs/site" \
  docs/site/en/contributing docs/site/ru/contributing
node docs/scripts/check-locales.mjs
```

Adapt the lint/link paths to the section you changed. The locale check covers all site
pairs and verifies presence, not semantic parity. Offline links do not verify remote
repository URLs or current hosted service availability. Generated HTML navigation and
anchors need their separate build/link route in the infrastructure reference.

Run the relevant full review checks before acceptance; a focused text check alone is not
a site build, rustdoc build, Rust gate or native interaction result. Inspect code examples
against their real owners; Markdown lint does not compile snippets or validate source data.

Authored text stays within 1000 lines and 64 KiB per file under the repository growth
policy. Keep prose within 100 columns where practical; tables and code can need more room.
Split by coherent responsibility, not numbered fragments, minification or new exclusions.

## Preserve provenance and migration boundaries

New text, media, code examples and reference captures need a known right to contribute.
Keep license notices and reviewed design-source identities. The [third-party guide][legal]
and [visual acceptance contract][visual] retain their authority; translations do not replace
license originals. AI assistance does not remove the contributor's review obligations.

Write for the reader's feature or task, not the history of writing the page. Non-plan pages
describe current facts, contracts and limitations, not session transcripts, intermediate
attempts or historical local pass counts. Put generated evidence under
`target/` and summarize actual verification in the review handoff. Plans and proposed ADRs
may describe future work only when clearly separated from implementation.

Migrate incrementally. Before retiring an old document, account for all live contracts,
limitations, provenance and legal obligations, review both required languages and update
incoming links/anchors. Preserve stable public URLs where already published. Do not relabel
obsolete implementation descriptions as plans or leave two competing authoritative copies.
`CONTRIBUTING.md`, `AGENTS.md` and retained repository authorities are not retirement targets;
the [contract directory](contracts/index.md) identifies the published engineering owners.

Finally, check the reader path: can a contributor find the owner, perform the documented
step and understand what remains unverified? Site preview artifacts are not publication.
Hosting, permissions, release snapshots and public deployment require separate approval.
Continue with [Tests and review](tests-and-review.md) for the evidence handoff.

[plan]: https://github.com/kedra-engine/kedra/blob/main/docs/documentation-plan.md
[infrastructure]: https://github.com/kedra-engine/kedra/blob/main/docs/documentation-infrastructure.md
[legal]: https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md
[visual]: contracts/ui/visual-verification.md
