# Kedra project website

<!-- impeccable:product-schema 1 -->

## Platform

web

This record covers the project landing site, not Kedra's native editor or runtime targets.

## Stack

Owner-approved static HTML/CSS, with no UI framework or application server. Vite supplies
the local development server and static build. Keep it separate from the VitePress manual.

## Users

Developers evaluating Kedra and its Rust/Bevy foundations, looking for source code,
documentation, development plans or a way to contribute.

## Product Purpose

Introduce Kedra and direct visitors to its repository or documentation. Kedra is an
open-source game-development environment built on Rust and Bevy, in early development.
The landing must not imply a complete game-production workflow or promise unsupported features.

## Operating Context

English is the canonical language; Russian is also supported. The landing and documentation
are separate static sites. The documentation address is configurable. Local documentation
preview uses `http://127.0.0.1:4173/`. Production targets `https://kedra.tech/` and
`https://docs.kedra.tech/`, with separate Object Storage buckets and protected-main CI publication.

## Brand Commitments

The owner supplied Figma file `2EZlK40cfx2Mub7GmDyI70`, frame `384:1497`, as the landing
reference. Preserve its copy, composition, Inter typography and T3 bird artwork; adapt
the layout for narrow screens without inventing additional marketing sections.

## Evidence on Hand

The Figma frame is the visual reference. Repository README and roadmap define product status.
Existing Inter fonts and their SIL OFL notice are under `assets/editor/fonts/`.
The artwork's exact source and the differing source-rights statements are recorded in
`public/brand/README.md`. Do not invent customers, downloads, benchmarks or testimonials.

## Capabilities and Constraints

Visitors can follow repository, documentation, roadmap, contribution and license links,
and change language without JavaScript. No download claim, mailing list, analytics,
account, cookie banner or contact form is part of this site.
Publication must follow the protected-main CI gate and the hosting procedure. Commits
still require owner inspection and approval; implementation alone does not authorize them.
