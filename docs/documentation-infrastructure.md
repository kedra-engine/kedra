# Documentation infrastructure

The documentation site is a static VitePress build for local preview and publication.
It supports English and
Russian pages, language switching and local search.
Repository policy, ADRs and crate-local contracts remain at their existing paths.
Existing guides remain authoritative until their reviewed replacements are ready.
See the [contents and migration plan](documentation-plan.md) for coverage and source ownership.
The tooling and ownership decision is recorded in
[ADR 0034](decisions/0034-bilingual-documentation-infrastructure.md).

## Prerequisites and commands

Use Node **24.22.1** and Bun **1.3.13**. Node runs VitePress and the locale check;
Bun installs the locked dependencies and runs package scripts. Install these
public tools using your normal version manager. The default manual build needs no
Rust build, editor window, private service or credentials. The opt-in API build
also requires the repository's Rust toolchain and native compilation prerequisites.

From the repository root:

```sh
cd docs
bun install --frozen-lockfile --ignore-scripts
bun audit
bun run build
bun run preview
```

Run commands separately and stop on failure. Preview serves the built site on
loopback; open the address printed by the server. `bun run dev` starts the local
development server with hot reload. Both servers bind to `127.0.0.1`, not the LAN.
Stop either server with Ctrl+C. Build before previewing changed source content.
Preview uses Vite's static multi-page server: VitePress 1.x's own preview ignores
the host option. The configured preview binds loopback and fails on an occupied
port rather than silently choosing a different one.

`bun run check:locales` checks that every English Markdown page has a Russian
counterpart at the same relative path, and vice versa. `bun run build` includes
that check, then VitePress rendering, missing-page detection and bundled-license
validation. It does not validate translation quality or native editor behavior.

### Complete local API preview

From `docs/`, run `bun run build:api`, then `bun run preview`. The EN/RU API guide
entries are `site/en/api/index.md` and `site/ru/api/index.md`. In a complete build,
their generated library lists link to actual English rustdoc pages under `/api/`.
In a default build they explain how to generate the missing reference and contain
no API-page links. The default build remains Rust-free and is the CI artifact.

The complete command reuses `cargo doc --workspace --no-deps --all-features --locked`
with `--target-dir target/documentation/cargo`. It sets
`RUSTDOCFLAGS="-D warnings -D rustdoc::broken_intra_doc_links"`, matching
`cargo xtask doc`. Cargo metadata supplies the documentable workspace library
names and versions; every linked entry must exist before VitePress renders.
Workspace binary documentation is also included. External dependency docs are
not bundled. This is one host-platform, debug-assertions-enabled all-features
build, not proof of other feature/target combinations or a release snapshot.

The small assembly script uses Node subprocess/filesystem APIs, Cargo metadata and
VitePress rather than another generator, Markdown parser or publisher. A default
theme slot renders the conditional library list; it does not fork rustdoc or the
manual theme. No authored Markdown link points at a nonexistent generated page.
Manual EN/RU search is unchanged. Rustdoc retains its separate English API search.

`build:api` owns and clears only `target/documentation/site/`,
`target/documentation/api-manifest.json` and its isolated Cargo `doc/` subdirectory.
It retains the isolated compilation cache and never changes ordinary `target/doc/`.
Before deletion or generation, a bounded filesystem preflight validates the
canonical repository root and every existing component of the fixed output paths:
`target/`, `documentation/`, `cargo/doc/`, `site/api/`, the API entry/license files,
manifest and VitePress cache. Symlinks, redirected ancestors and wrong file types
are rejected using Node's `lstat` and `realpath`; missing generated paths are
allowed. No directories are created or removed by preflight itself.

Only a successful preflight permits cleanup of the fixed preview, manifest and
rustdoc output paths. Checks repeat after child processes and before copied
output writes and failure cleanup. A failed preflight disables cleanup, preserving
both redirected foreign data and any existing preview rather than deleting paths
whose ownership could not be established. After ordinary generation failures with
valid ownership, preview and manifest are removed so partial output is not accepted.
Resolve a rejected filesystem layout and rebuild before using that preview.

This is a check against pre-existing redirects, not a sandbox for Cargo/VitePress
or protection against concurrent malicious filesystem replacement between checks
and operations. A default build replaces the complete preview with a manual-only
one. Do not run builds concurrently or validate files while another build replaces
them. A failed build can interrupt an already running preview; rebuild successfully
before using it again.

## Source and output ownership

| Location | Responsibility |
| --- | --- |
| `docs/package.json`, `docs/bun.lock` | Exact direct dependencies and resolved graph/integrity |
| `docs/.node-version`, `docs/bunfig.toml` | Node pin and installer settings |
| `docs/.vitepress/` | Site configuration and maintained default theme entry |
| `docs/site/index.md` | Language chooser at the root, without server redirects |
| `docs/site/en/`, `docs/site/ru/` | Paired published pages with matching English slugs |
| `docs/site/public/` | Shared static media; retain provenance and licenses |
| `docs/scripts/check-locales.mjs` | Kedra's required EN/RU page-pair policy |
| `docs/scripts/build-api.mjs` | Opt-in Cargo/rustdoc and manual assembly |
| `docs/scripts/adapt-api-html.mjs` | Reviewed Rust 1.98.0 copied-HTML compatibility |
| `target/documentation/site/` | Generated HTML, assets, local search and notices |
| `target/documentation/site/api/` | Complete rustdoc output and Kedra license texts |
| `target/documentation/cargo/` | Isolated API compilation cache and fresh rustdoc output |
| `target/documentation/api-manifest.json` | Generated library names, versions and verified links |
| `target/documentation/cache/` | Disposable VitePress cache, when used |

Only content under `docs/site/` is built into the site. Documents elsewhere in the
repository are not automatically included as manual chapters. `docs/node_modules/`
and generated output are ignored by Git.

The site uses explicit HTML URLs (`cleanUrls: false`), so a static host does not
need extensionless-page rewrites. Language entry paths use directory indexes.
The current configuration assumes hosting at a domain root. A subpath deployment
needs an explicit VitePress `base` configuration and another link/preview check.

English is canonical. The maintained locale routing switches corresponding page
paths; CI requires the counterpart rather than publishing a missing translation.
The check validates file presence only. Search uses an in-browser index, with
Russian search controls; no Algolia credentials or external search service is used.

## Local validation

From the repository root, after a successful build:

```sh
rumdl check .
lychee --offline --include-fragments --no-progress --root-dir "$PWD/docs/site" './*.md' docs
lychee --offline --include-fragments --no-progress --root-dir "$PWD/target/documentation/site" --index-files index.html --no-ignore 'target/documentation/site/**/*.html'
cargo xtask check
```

The site build checks Markdown page targets; Lychee checks generated navigation,
local assets and fragments, including VitePress-generated anchors. `--no-ignore`
is intentional for the generated site under ignored `target/`. Source Markdown
and generated HTML are different check surfaces; neither replaces the other.
RON code blocks use Rust token highlighting, not schema validation. Check example
syntax and field semantics against the owning source format and production tests.
Pass `docs` as a directory so Lychee honors Git ignores for installed dependencies;
an explicit recursive glob also expands third-party `node_modules` documents.
Use the pinned rumdl/Lychee versions in [testing](site/en/contributing/contracts/testing.md)
and [CI](ci.md).

### Rustdoc HTML compatibility

Rust 1.98.0 emits JavaScript-handled source-range fragments such as `#34-61`, but
only individual source-line IDs. It also copies several inherited dependency
documentation links without resolving their original crate context. Neither
rustdoc nor Lychee has a standard option that repairs both the static artifact and
its validation. Hiding sources would lose useful content; fragment exclusions,
Lychee-only remapping or preprocessing would leave visitors with different or
broken output. These alternatives are not used.

`adapt-api-html.mjs` adapts only the copied `site/api/` HTML. Cargo's original
`cargo/doc/` remains unchanged. It reuses the already locked, MIT-licensed Vue
`@vue/compiler-dom` **3.5.44** HTML parser for element structure and exact source
offsets, not a custom HTML parser or serializer. It is build-only; no dependencies
or browser bundles are added. Node's built-in glob owns file discovery.
The bare parser import relies on the frozen Bun transitive graph and its current
installed package layout; it is not a separately declared direct dependency.
Recheck resolution on Bun/installer-layout, Vue or VitePress upgrades.

For each observed local source-range URL, the adapter validates the contained,
non-symlinked `.rs.html` target, the reviewed rustdoc version, and the real numeric
anchors throughout that range. It wraps the existing start-line number anchor in
spans carrying the exact range IDs. The original numeric IDs, links, source text
and rustdoc range-highlighting JavaScript are preserved. Multiple ranges sharing
a start line have nested aliases around the same real line anchor, not empty
placeholders or fabricated source lines.

Six exact inherited href forms are mapped to versioned upstream documentation.
The adapter checks their enclosing trait implementation, member and link text,
and requires the exact reviewed registry package versions in locked Cargo metadata.

| Original href | Owning dependency and source | Versioned target |
| --- | --- | --- |
| `dispatcher#setting-the-default-subscriber` | tracing 0.1.44, `src/instrument.rs`, `WithSubscriber::with_current_subscriber` | [Dispatcher defaults](https://docs.rs/tracing/0.1.44/tracing/dispatcher/index.html#setting-the-default-subscriber) |
| `crate#per-style` | yansi 1.0.1, `src/macros.rs`, `Paint::whenever` | [Per-style conditions](https://docs.rs/yansi/1.0.1/yansi/index.html#per-style) |
| `TypePath#anonymity` | bevy_reflect 0.20.0, `src/type_path.rs`, `TypePath` name methods | [TypePath anonymity](https://docs.rs/bevy_reflect/0.20.0/bevy_reflect/trait.TypePath.html#anonymity) |
| `Reflect` | bevy_reflect 0.20.0, `src/reflect.rs`, `PartialReflect::try_*reflect*` | [Reflect](https://docs.rs/bevy_reflect/0.20.0/bevy_reflect/trait.Reflect.html) |
| `TypeInfo` | bevy_reflect 0.20.0, `src/info/typed.rs`, `Typed::type_info` | [TypeInfo](https://docs.rs/bevy_reflect/0.20.0/bevy_reflect/enum.TypeInfo.html) |
| `%60States::DEPENDENCY_DEPTH%60` | bevy_state 0.20.0, `src/state/state_set.rs`, `StateSet::SET_DEPENDENCY_DEPTH` | [State dependency depth](https://docs.rs/bevy_state/0.20.0/bevy_state/state/trait.States.html#associatedconstant.DEPENDENCY_DEPTH) |

The original forms and ownership were checked against exact Cargo-resolved source;
the upstream pages and fragment IDs were checked at those versions. Offline
Lychee does not validate remote availability. External API links require a
network connection, as do other rustdoc dependency links.

All edits are planned and validated before writing. Unreviewed versions, changed
known link shapes, conflicting aliases, missing source lines or escaped/symlinked
paths stop the complete build, whose failure cleanup removes the preview. Unknown
broken links are not generically guessed or removed; the unchanged generated-site
Lychee command remains the validation gate. Toolchain/dependency upgrades require
reviewing this bounded adapter and rerunning complete build and link checks.
Static assets, scripts, legal notices and fonts are not rewritten. No link or
fragment exclusions, accepted-error overrides or weakened checks are introduced.

## Dependencies, security and licensing

VitePress **1.6.4** is the stable generator, licensed MIT. It provides the routing,
theme, Markdown renderer and local search instead of custom infrastructure. Its
upstream Vite 5 dependency has known advisories. The exact override to Vite
**6.4.5** avoids those versions; `@vitejs/plugin-vue` 5.2.4 admits Vite 6. The
override is an integration responsibility, not an upstream VitePress support
promise. Recheck build, development/preview and search when updating the graph.
Do not remove the override or suppress advisories merely to satisfy installation.

Bun is MIT-licensed. Its native text lock format fits the repository file-growth
policy without minification or lockfile exemptions. CI installs with a frozen
lock and ignores package lifecycle scripts. `bun audit` checks the resolved graph
against public advisories and fails the job when vulnerabilities are reported.
Audit availability is a network prerequisite, not evidence of a passing audit.

The installed documentation dependencies declare MIT, BSD-2-Clause, BSD-3-Clause,
ISC and CC0-1.0 licenses; build-only SPDX helpers also declare CC-BY-3.0 or
`MIT AND CC-BY-3.0`. Preserve their upstream attribution if redistributing the
toolchain. `rollup-plugin-license` **3.7.1** (MIT)
checks bundled dependency licenses and generates `third-party-notices.txt` with
upstream copyright/license text. Unknown or disallowed bundled licenses fail the
build. Its allowlist also admits Apache-2.0. Preserve that file with any preview
artifact or future deployment. It is not a complete license audit of every
build-only dependency or a replacement for reviewing dependency updates.

Use VitePress's default theme without its bundled font assets; system fonts avoid
an additional font distribution obligation. New media, branding and third-party
content need their own provenance review. Generated notices do not cover them.

The opt-in API copy preserves the entire rustdoc output, including `static.files/`,
sources, search JavaScript, fonts and their notices. Rust 1.98.0's emitted
`COPYRIGHT-*.txt` identifies Rustdoc resources under MIT OR Apache-2.0,
`normalize.css` under MIT, Fira, Source Code Pro, Source Serif 4 and Nanum Barun
Gothic fonts under SIL OFL-1.1, and Rust logos under CC-BY with Rust Foundation
trademark-policy attribution. Preserve the emitted copyright and license files,
font reserved-name conditions and logo attribution; do not imply Rust endorsement.
Kedra's `LICENSE-MIT` and `LICENSE-APACHE` are copied to `api/` for its own source
and documentation. Vite's bundled notices do not cover rustdoc's additional
resources. Recheck the pinned toolchain's emitted notices before redistributing a
complete artifact or updating Rust; the publisher must retain the entire API output.
Inherited dependency documentation retains its upstream licensing. Rustdoc's
emitted asset notices are not a complete audit of those excerpts or of every
dependency; review those obligations before public redistribution as well.

## CI and publication boundary

The `repository` job in [CI](../.github/workflows/ci.yml) installs the
pinned Node/Bun versions, performs the frozen install and audit, builds EN/RU,
checks generated links/fragments and runs the repository Markdown/link checks.
It uploads a seven-day `documentation-site` preview artifact only after successful
validation. It is part of the fail-closed `CI required` gate.
The complete API integration is not part of this Rust-free job or its manual-only
preview artifact. The repository job also validates the separate landing build,
dependency audit, contract tests and generated links.

Known site/configuration/dependency paths select the repository checks without
the Rust matrix; unknown inputs still select Rust conservatively. Workflow
changes also select documentation validation. Repository-wide policy and secret
checks remain enabled. `cargo xtask check` remains the Rust check route; it does
not silently install Node/Bun or substitute for the separate site commands.

After `CI required` succeeds for official main CI, the
[static-site build job](../.github/workflows/ci.yml) builds the complete API
site and landing without cloud credentials. A separate environment-scoped job uploads
the validated artifacts to Yandex Object Storage. Production targets are
`https://docs.kedra.tech/` and `https://kedra.tech/`; see
[hosting](../tools/hosting/README.md) and
[ADR 0035](decisions/0035-project-sites-in-object-storage.md)
for credential, DNS and HTTPS boundaries.
Required PR checks do not depend on cloud secrets. Downloading a preview artifact is
not publication. Verify actual hosted CI, uploads and HTTPS externally; local checks
do not establish them. Release-specific documentation snapshots are not configured.
