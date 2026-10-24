# Kedra project landing

A separate English/Russian static website matching the owner-supplied
[Figma landing](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/Kedra?node-id=384-1497).
English is served at `/`, Russian at `/ru/`. The documentation keeps its own build,
appearance and routes; it is not copied into the landing output.

## Run locally

Use Node 24.22.1 and Bun 1.3.13, as for the documentation toolchain. From `website/`:

```sh
bun install --frozen-lockfile --ignore-scripts
bun audit
bun run build
bun run preview
```

Run commands separately and stop on failure. Preview binds to `http://127.0.0.1:4174/`;
development uses `bun run dev` at `http://127.0.0.1:5174/`. Occupied ports fail rather
than silently selecting another port. Neither server binds to the LAN.

Vite writes only its generated output under `target/website/site/`. Keep both language
pages, bundled font assets, brand artwork and license texts when copying that output.
Builds of the landing and documentation use separate output directories.

## Documentation address

The default is the local manual preview, `http://127.0.0.1:4173/`. Start that preview
separately using `docs/documentation-infrastructure.md` in the repository.
Before preparing output for another environment, set the documentation root explicitly:

```sh
KEDRA_DOCS_URL=https://docs.kedra.tech/ bun run build
```

`KEDRA_DOCS_URL` is a public build-time URL, not a credential.
It must be an absolute HTTP(S) URL without
credentials, query or fragment. A base path is supported; a trailing slash is normalized.
English and Russian links append `en/` and `ru/`; contribution links use the same locale.
The repository and roadmap links use the project's public GitHub repository.
The landing assumes it is served at a domain root. Production targets `https://kedra.tech/`
with documentation at `https://docs.kedra.tech/`. The [hosting guide](../tools/hosting/README.md)
defines the secret-backed Object Storage publisher and HTTPS/DNS configuration.
Building or previewing locally does not publish anything.

## Assets and dependencies

Vite 6.4.5 is pinned to the same reviewed version as the documentation build. It owns
HTML/CSS processing, font bundling and the local server; no UI framework, runtime script,
remote font provider, analytics or new asset-processing tool is needed. Build tooling
is separate from the Rust workspace and is not required to run native checks.

CSS imports the existing Inter Regular, Medium and SemiBold sources without changing
or duplicating them. The build emits their original SIL OFL notice to
`licenses/Inter-OFL.txt`. The project license texts are emitted as
`licenses/LICENSE-MIT` and `licenses/LICENSE-APACHE`. Preserve each license under its
own terms; the project license does not relicense fonts or third-party assets.
The artwork's source identity and rights statements are in
[brand provenance](public/brand/README.md). Preserve that provenance with copied output.

## Verify

Build and inspect both languages at desktop and phone widths, test keyboard focus and
all navigation links, and run the repository gate from the repository root:

```sh
cargo xtask check
rumdl check website
lychee --offline --include-fragments --no-progress --root-dir "$PWD/target/website/site" --index-files index.html --no-ignore 'target/website/site/**/*.html'
```

Offline link checks do not validate the availability of GitHub or an independently
hosted manual. Vite's build is not a visual or accessibility test. The required repository
CI job also validates this landing; a separate post-gate job handles publication.

From `website/`, `bun run test` invokes real builds with an isolated output and checks
Kedra's locale/manual-link, unchanged asset and license-delivery contracts. Invalid
documentation roots must fail without replacing existing output. Each build has an
external process deadline; tests do not start a browser or the native editor.
