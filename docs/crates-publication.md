# Crates.io publication

The [Crates.io publication workflow](../.github/workflows/crates.yml) publishes the
libraries admitted by Cargo's workspace manifests. It is separate from desktop binary
releases and static-site publication. Publication is permanent and non-atomic across
the workspace; do not use it as a name-reservation or test-upload mechanism.

## Scope and admission

The workspace version is `0.1.3`. The publishable libraries are `kedra_assets`,
`kedra_scene`, `kedra_script`, `kedra_docking`, `kedra_ui`, `kedra_app`,
`kedra_authoring`, `kedra_desktop`, `kedra_import`, `kedra_inspector`,
`kedra_project`, `kedra_update`, `kedra_editor` and `kedra_script_bevy`.
`kedra_build`, `kedra_test`, the editor executable, project-local launchers and
`xtask` retain `publish = false`. Do not silently enable them or change versions
to work around a registry failure. New names may be taken or subject to registry quotas.

Cargo owns package selection, archive creation, inter-crate dependency ordering,
registry-index readiness and upload. Its workspace publication support avoids a
custom dependency walker, registry uploader or release scheduler. The policy helper
reuses the existing release CI-admission function and checks Kedra-specific identity
and dual-license delivery, not Cargo's internal resolution algorithm. Unlike
`cargo publish --workspace`, `cargo package --workspace` also selects non-publishable
members, so archive materialization uses explicit packages from Cargo metadata.

The workflow accepts only an explicit current `main` commit, running the workflow
at that same commit in `kedra-engine/kedra`. The source must be clean and have a
genuine successful official push CI run. Main advancement fails closed, including
the freshness check immediately before upload. Workflow changes require normal
protected-main review and CI; dispatching an arbitrary historical source is not supported.

## Credentialless verification and upload

The `verify` matrix performs `cargo publish --workspace --dry-run --locked --all-features`
on Linux, Windows and macOS without a registry credential. It then materializes archives
with Cargo and verifies their actual package identity and both license texts. Both
license files in every admitted crate must match the canonical root files byte for
byte. Preserve those copies when changing legal text. Archives remain seven-day
evidence, not a substitute for published registry checksums or legal review.

Only an explicitly requested `publish = true` reaches the upload job after the entire
matrix succeeds. The `crates-production` environment permits only branch `main`, with
administrator bypass disabled. The upload job rechecks source admission, then Cargo
repackages the same immutable source. Its `--no-verify` skips only a repeated compilation
already completed by the current-run matrix; it is not a route around failed validation.
No dirty-source or unlocked publication option is permitted in CI.

The upload step alone references `CARGO_REGISTRY_TOKEN`. The owner supplies this secret;
never paste it into chat, write it into the repository, pass it as a command argument
or print it. If exposed, revoke it rather than reuse it. A repository secret remains
repository-scoped: placing the job in an environment does not relocate the secret.
Prefer storing it directly in the `crates-production` environment through GitHub's UI
or `gh secret set CARGO_REGISTRY_TOKEN --repo kedra-engine/kedra --env crates-production`.
After that secure transfer, remove the repository-level copy. Builds never reference it.

Initial crate registration requires a bootstrap token authorized for new publications.
The narrow `use-trusted-publishing` exception applies only to that upload command,
not to unrelated workflows or security audits. After initial registration, configure
each crate's trusted publisher for this repository, workflow and environment, then
replace the static-key step with crates.io's maintained OIDC action and revoke the
bootstrap token. Do not enable `id-token: write` before that separate configuration.

## Operator commands

First merge the reviewed workflow and metadata through the ordinary PR gate. Obtain
the full SHA of current main and verify its push CI succeeded. Dispatch validation
without uploading:

```sh
gh workflow run crates.yml --repo kedra-engine/kedra --ref main -f source=FULL_MAIN_SHA
```

To authorize actual publication, dispatch with the additional `-F publish=true`.
Use the exact SHA, not a branch name. The dispatch flag defaults to false; validation
alone does not publish. Follow the run's live diagnostics and final summary.
No command here authorizes committing unreviewed changes or bypassing protection.

On failure, inspect Cargo's report and the registry before retrying. Some packages may
already be public even if the job failed; an upload/index timeout is not proof of
absence. Check names, versions, owners and archive checksums. Do not automatically
skip conflicts, yank packages, bump versions, relax policy, remove verification or
overwrite a registry identity. Resume only after an explicit reviewed recovery plan.

## Consumer and acceptance limits

Registry consumers do not inherit the source workspace's `[patch.crates-io]` table.
The macOS Winit client-titlebar correction still requires the consumer application's
root patch from [ADR 0020](decisions/0020-macos-client-titlebar-input.md), pinned to its
reviewed public fork revision. A successful registry archive build does not establish
native drag/drop behavior without that patch, or physical input/frame acceptance.
Do not remove the source patch or publish a renamed forwarding dependency to hide this.

The registry artifacts provide libraries, not the editor's external authored asset
delivery, standalone desktop installers or a complete game SDK. Repository-owned
native scenarios and source-relative test fixtures require the source checkout;
registry compilation is not the full repository/native test route. Keep existing
runtime/editor layering, license obligations and implementation limits.

After publication, verify every admitted crate/version and its registry checksum,
owner access, dependency links and an independent registry consumer. docs.rs builds
are asynchronous and separate from both the registry upload and Kedra's hosted API.
Do not claim publication, docs.rs availability or native acceptance from a dry-run.
