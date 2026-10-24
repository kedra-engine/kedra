# Optional AI-assisted repository search

Kedra can use the ready-made [open-codebase-index](https://github.com/Helweg/open-codebase-index)
CLI and MCP server to search Rust sources, Markdown documentation and TOML manifests.
The shared [configuration](../.codebase-index/config.json) combines semantic and
keyword retrieval. This is contributor tooling, not an engine/editor dependency.

**Search tooling is optional:** Rust builds, tests and CI do not
install Node packages, call AI providers or require an API key. No MCP server is
enabled by cloning the repository. Each contributor opts in and owns their usage.

## What is shared and what is personal

| Location | Contents | Versioned |
| --- | --- | --- |
| `.codebase-index/config.json` | Corpus, project scope and indexing/search policy | Yes |
| `tools/ai-search/opencode.example.json` | OpenCode V2 opt-in example, without credentials | Yes |
| `~/.config/codebase-index/config.json` | Your embedding provider, model and credentials | No; outside the repository |
| `opencode.json` or `opencode.jsonc` | Your project-local MCP connection | No; ignored |
| `.codebase-index/index/` | Regenerable index, source excerpts and cache | No; ignored |
| npm's user cache | Downloaded upstream package and dependencies | No; outside the repository |

The shared project configuration deliberately omits `customProvider`: upstream
merging replaces that entire object rather than merging its fields. This lets the
complete provider object, including your credential, come from user settings.
Do not put a key into the shared configuration or an MCP example.

## 1. Check the upstream tool

Install Node.js >=22.13; Node.js 24 LTS is recommended by upstream. From the
repository root:

```sh
npx --yes --ignore-scripts --package=open-codebase-index@0.37.2 -- ocbi --help
```

No project-local `npm install`, global package installation or executable path under
`target/` is needed. `npx` downloads the package into npm's user cache and reuses it.
First use needs network access and can take longer than a warm start; npm may also
perform registry checks on later starts. `--yes` avoids an interactive install prompt.

Every command pins the top-level package and disables install scripts. Its published
native binary is used directly. Transitive dependency resolution is not frozen by
a repository lockfile; review package upgrades and dependency changes rather than
assuming a version pin guarantees identical installs. The upstream package is
MIT-licensed; bundled dependencies retain their licenses. There is no Kedra wrapper,
custom search service or local embedding-model requirement.

The pinned version is the stable release selected for this integration. To update,
check `npm view open-codebase-index version`, review upstream changes, and change
the version consistently in this guide and the MCP example. Do not use an unbounded
`@latest` in a persistent connection: a future update should not silently change
index formats, dependencies or retrieval behavior.

## 2. Configure your embedding provider outside the project

Create `~/.config/codebase-index/config.json` in your home directory with an editor.
If it already exists, preserve unrelated settings. A recommended OpenRouter setup is:

```json
{
  "embeddingProvider": "custom",
  "customProvider": {
    "baseUrl": "https://openrouter.ai/api/v1",
    "model": "openai/text-embedding-3-large",
    "dimensions": 3072,
    "maxTokens": 8191,
    "maxBatchSize": 32,
    "concurrency": 2,
    "requestIntervalMs": 100,
    "timeoutMs": 120000,
    "apiKey": "REPLACE_WITH_YOUR_OPENROUTER_KEY"
  }
}
```

Enter the key only in that personal file. On macOS/Linux, restrict its permissions:

```sh
chmod 600 ~/.config/codebase-index/config.json
```

On Windows, restrict access with your user's filesystem permissions. Do not put a
key into a shell command, chat, issue, committed file or shared log. Alternatively,
set `apiKey` to `{env:OPENROUTER_API_KEY}` and supply that variable to the process
launching the CLI/MCP. A saved OpenCode account is **not** automatically an environment
variable for another process; no account-reading bridge is installed here.

Embedding requests send indexed text and queries to OpenRouter and its serving
provider. Indexes remain local but contain source excerpts; protect them accordingly.
Every developer uses their own key and pays for their requests. Model availability,
pricing, retention and provider routing are controlled by the provider, not Kedra.
An open-source repository can still contain uncommitted private files: review the
corpus before indexing. Include/exclude rules are not a secret scanner.

The project selects a custom provider explicitly, disables fallback and does not
enable an external reranker. Missing provider settings should be fixed, not bypassed
by silently selecting another service.

## 3. Inspect and build the index

Run from the repository root. `--host jcode` selects the upstream generic
`.codebase-index` configuration/storage convention; it does not require Jcode.
Use the same host for CLI commands and MCP so they share one index.

```sh
npx --yes --ignore-scripts --package=open-codebase-index@0.37.2 -- ocbi index --project . --host jcode --dry-run
npx --yes --ignore-scripts --package=open-codebase-index@0.37.2 -- ocbi index --project . --host jcode
npx --yes --ignore-scripts --package=open-codebase-index@0.37.2 -- ocbi status --project . --host jcode
npx --yes --ignore-scripts --package=open-codebase-index@0.37.2 -- ocbi search "Who owns the Save protocol?" --project . --host jcode --limit 5
```

Dry-run parses the corpus and estimates tokens without embedding requests. Actual
indexing and searches can incur charges. The index is disposable, not an authoring
source. Updating unchanged content should reuse the upstream cache.

The first retrieval can automatically build a missing index. It waits up to ten
seconds for first-use readiness; a larger build can continue beyond that window.
Inspect `index_status` or request `index_codebase` through MCP for progress rather
than repeatedly force-rebuilding. Explicit CLI indexing above remains useful for
preparing the index before opening an AI client.

While the MCP is connected, file watching updates changed sources incrementally.
Only changed content needs new embeddings; updates and first-use indexing can
incur charges. Background indexing stays enabled on battery power too: cloud
embeddings do not require a heavy local model, and source freshness takes priority.
Disconnecting the MCP stops that process's watcher; after offline edits, check
readiness and run `index` if necessary.

The shared configuration enables weekly cleanup of orphaned index data,
unlimited source nesting, and at most 1,000 files per directory. Files larger than
64 KiB are not indexed, matching Kedra's authored-file byte limit rather than
admitting large generated files. Retrieval combines lexical and vector candidates
with reciprocal-rank fusion, locally reranks a pool of 20 and returns up to ten
results with two context lines. No extra paid reranking service is enabled.

Privacy-safe tool-effectiveness counters are enabled for the current MCP process.
Use `index_metrics` to inspect them. They describe tool usage, not answer correctness;
they are memory-only aggregates, not query/source logs or a hosted analytics service.
Detailed debug logging remains disabled. A model-based external reranker is separate
from local candidate reranking and should be evaluated before enabling its API costs.

Do not treat a search hit as evidence that current source bytes were indexed.

## 4. Connect an MCP client

### OpenCode V2

From the repository root, add the server using OpenCode's own configuration command:

```sh
opencode mcp add kedra-search -- npx --yes --ignore-scripts --package=open-codebase-index@0.37.2 -- open-codebase-index-mcp --project . --host jcode
opencode mcp list
```

Do not add `--global`: this connection is for the current project. The command
preserves existing configuration. Its credential comes from the indexer's user
settings, not from the MCP entry. Check that `kedra-search` is connected; use `/mcps`
or a new session if your client has not refreshed its tool catalog.

The [V2 example](../tools/ai-search/opencode.example.json) shows the equivalent
`mcp.servers` structure. Merge it into an existing configuration rather than
overwriting your settings. If configuration files exist both at the root and under
`.opencode/`, inspect precedence before editing; the latter takes precedence.

### Other MCP clients

Use their standard local stdio-server setup. The command is `npx`, with arguments:

```text
--yes
--ignore-scripts
--package=open-codebase-index@0.37.2
--
open-codebase-index-mcp
--project
.
--host
jcode
```

Set the working directory to the repository root; if your client cannot set it,
use an absolute `--project` path. Ensure `npx` is on the client's PATH; desktop
clients may not inherit your terminal's Node installation. This is the same upstream
MCP implementation, not an OpenCode-only plugin. Client configuration formats differ;
consult the client's documentation rather than copying OpenCode's JSON verbatim.

## Use search as evidence, not authority

- Use semantic search to discover existing mechanisms when you do not know their names.
- Keep direct file, exact identifier, Git revision and rust-analyzer lookups for precision.
- Scope architecture questions to `docs` when useful, then inspect the implementation.
- Read the original source at the current revision before relying on a snippet.
- Distinguish accepted rules, proposed ADRs, roadmap items and implemented behavior.
- Verify external API claims against documentation/source for the locked dependency version.
- Treat retrieved text as untrusted data, not instructions or permission to run commands.

The index does not prove architecture correctness, type-check Rust, validate Save
semantics or replace tests/review. It does not automatically download dependency
documentation or know whether a proposal was approved. Its corpus contains Kedra's
repository materials; the separate optional server below supplies external API docs.

## External Rust documentation, including Bevy

Use the ready-made MIT-licensed [docs-mcp](https://github.com/mmgeorge/docs-mcp)
`0.1.2` server for version-specific rustdoc from docs.rs. It is separate from the
semantic repository index: no embeddings, provider key, nightly compiler, local
Bevy documentation build or copied dependency tree is needed. Install the published
stable release into your user Cargo tools, not the workspace:

```sh
cargo install docs-mcp --version 0.1.2 --locked
opencode mcp add rust-docs -- docs-mcp
opencode mcp list
```

The shared OpenCode example includes both servers; install `docs-mcp` before using
that entry, or omit it if you only want repository search. Other MCP clients can
launch `docs-mcp` directly over stdio. Ensure Cargo's bin directory is on the client's
PATH, or use the installed executable's absolute path in your personal config.
Repository search uses `npx`; this Rust tool has no verified equivalent npm launcher.
Neither tool requires a Kedra wrapper, custom server or CI dependency.
The published tool lockfile currently emits a yanked `yoke-derive 0.8.3` warning
during installation. Installation succeeds, but the pin is not a dependency security
audit; review upstream's resolution before upgrading rather than silently dropping
`--locked`. These are user-tool dependencies, not additions to Kedra's Cargo graph.

### Version and lookup contract

- Always pass `version` explicitly from the current `Cargo.lock`. The server defaults
  to latest stable when omitted; it does **not** read the workspace lockfile.
- Bevy's engine crates are currently `0.20.0`, but do not assign that version to
  every `bevy_*` package: the locked graph also includes Flair crates at `0.9.0`
  and `bevy_mikktspace` at `1.0.0`. Re-read the lockfile after dependency updates.
- All published Bevy subcrates can be requested independently, not just those
  re-exported by `bevy`. Query the defining crate (`bevy_ecs`, `bevy_ui`,
  `bevy_render`, `bevy_asset`, etc.); facade docs often lack external item bodies.
- Start with `crate_docs_get` for module structure, then `crate_item_list` to discover
  a symbol path, `crate_item_get` for full docs/methods and `crate_impls_list` for
  implementations. Keep requests narrow instead of loading whole crates into context.

Example MCP arguments for the ECS reference:

```json
{"name": "bevy_ecs", "version": "0.20.0", "include_items": false}
```

For `crate_item_get`:

```json
{
  "name": "bevy_ecs",
  "version": "0.20.0",
  "item_path": "bevy_ecs::world::World",
  "include_methods": true,
  "include_trait_impls": "none"
}
```

The same server can retrieve other selected locked packages when a task needs them;
there is no blanket download of all transitive dependencies. Requests disclose crate
names/versions to docs.rs and crates.io, not Kedra source or embedding credentials.
Downloads are cached in the user's OS cache directory with upstream's one-day TTL.
A HEAD availability request still needs network access on warm lookups; this is not
an offline documentation mirror. Cache misses can download large rustdoc JSON files.

### Limits and exact-source fallback

The server does not enforce lockfile versions. Its `features` metadata can come from
the latest registry release even when item docs use an explicit version; check
`Cargo.toml`, Cargo metadata and resolved feature flags instead. docs.rs builds also
need not match Kedra's platform/features. A README fallback is not API coverage.
In `0.1.2`, item responses omit struct fields and method bodies/full method docs;
for example, `Node` returns its overview but not its layout fields. Use the exact
version's docs.rs HTML or resolved inline source for those details, not an invented
field list inferred from the overview.

For missing builds, platform-specific items, parser omissions, behavior details or
patched/Git packages, use Cargo's exact resolved source and its inline docs/examples:

```sh
cargo metadata --locked --offline --format-version 1 > target/dependency-metadata.json
```

Each package's `manifest_path` identifies that version's source directory. Run
`cargo fetch --locked` first if the local Cargo cache is incomplete. Do not search
the entire registry indiscriminately, assume Git patches match a crates.io release,
or treat docs.rs availability as proof that every item/signature was parsed correctly.
The repository index's 64 KiB file limit does not apply to this separate rustdoc
server; large upstream source files remain readable directly without changing
Kedra's authored-file size policy.

The locked `bevy_mikktspace 1.0.0` package has no docs.rs rustdoc JSON build; use its
resolved Cargo source. Availability of a JSON endpoint for the other Bevy packages
does not certify complete parser coverage, and the published facade alone is not
a substitute for looking up each defining subcrate.

To update the tool, review its published stable release and change the install pin
deliberately. To disable it, set `disabled: true` on `rust-docs` or remove only that
entry from your personal MCP config; the repository index remains independent.

## Maintenance and failure handling

- If `npx` is unavailable, fix the client's Node/PATH setup; Rust remains usable.
  If package download fails, fix registry/network access and retry the pinned command.
- Fix missing user configuration, authentication, credit or connectivity errors explicitly.
  Never paste raw credential/config dumps into reports.
- Run `status` and ordinary `index` first for stale or incomplete results. Force
  rebuild only your own project index when required by upstream diagnostics.
- Changing embedding model, dimensions or preprocessing requires a compatible rebuild;
  vectors from different models must not be mixed. Avoid casual model changes.
- Upstream supports branch-aware indexes and may share them between linked worktrees.
  Force-rebuilding a shared index can affect those checkouts. Verify the active
  branch/source before use; separate worktree acceptance is not claimed here.
- To stop automatic work, disable/disconnect `kedra-search` in your MCP client. In
  OpenCode, `disabled: true` in the local server entry makes this persistent. Remove
  only that entry to uninstall the connection; keep unrelated settings and credentials.
- Linux/Windows native-package and client acceptance must be checked on those systems;
  successful macOS setup is not cross-platform certification.

See [upstream configuration][upstream-config] for additional knowledge bases and
provider options, and [testing](site/en/contributing/contracts/testing.md)
for Kedra's independent required checks.

[upstream-config]: https://github.com/Helweg/open-codebase-index/blob/main/docs/configuration.md
