# ADR 0034: Bilingual static documentation infrastructure

Status: implemented; [ADR 0035](0035-project-sites-in-object-storage.md) extends the local build
with protected-main publication.

## Decision

Use VitePress 1.6.4 for a product-oriented English/Russian documentation site.
English remains canonical; Russian supplements it. Rust APIs, commands and
identifiers remain English. API reference generation stays with rustdoc.

Keep the site sources under `docs/site/`, separate from existing
engineering documentation. Replace old documents incrementally only after their
contracts, limitations, provenance and incoming links have been accounted for.
ADRs, legal originals and crate-local contracts are not disposable migration input.

Node 24.22.1 runs the tooling; Bun 1.3.13 owns dependency installation and its
native text lock. The standard lock representation fits the existing file-growth
policy without exceptions, compression or custom lock tooling. Frozen installs
disable package lifecycle scripts. The documentation graph has no Cargo/runtime
dependency and builds without an editor, credentials or private infrastructure.

Override VitePress's vulnerable Vite 5 dependency with Vite 6.4.5. The resolved Vue
plugin admits Vite 6, but this remains a Kedra integration responsibility until
the generator's stable dependency baseline catches up. Use Vite's static preview
because VitePress 1.x preview does not honor a loopback host option. Keep the
maintained default theme without bundled fonts and use local search.

The existing repository CI job owns frozen install, public advisory audit, EN/RU
file-pair validation, build, bundled-license validation, generated-site links and
preview artifacts. It remains within the existing required-check gate and read-only
permissions. This build decision introduces no Pages deployment or publishing credentials;
the separate publication boundary is defined by ADR 0035.

## Alternatives and dependency rationale

- Existing Markdown alone preserves source review but does not provide the
  connected bilingual manual, navigation and browser search being requested.
- mdBook is a natural Rust documentation option. VitePress is selected for its
  built-in locale configuration and maintained default-theme/local-search path,
  rather than adding separate localization integration.
- A custom site, search engine or translation framework would duplicate maintained
  tools. The only custom check enforces Kedra's page-pair policy using Node's glob;
  it does not parse Markdown, crawl links or assess translation quality.
- Rollup's maintained license plugin checks bundled SPDX expressions and emits
  copyright/license notices. Lychee and rumdl retain link/Markdown ownership.

## Consequences and verification

Documentation adds an independently pinned JavaScript toolchain and dependency
review responsibility. The Vite override needs build, dev/preview and search
verification when updated; advisory checks must not be silenced to retain a pin.
Bundled notices must accompany preview artifacts and any future deployment.

Run the commands in [documentation infrastructure](../documentation-infrastructure.md).
Check EN/RU routes and search in a real browser. A missing translation must stop
the build; broken generated anchors must fail the link check. These checks do not
establish translation quality, native editor acceptance or hosted CI success.
Hosting and release-version snapshots remain separate decisions.
