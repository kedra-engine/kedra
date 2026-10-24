# Public static sites

The source repository owns two independently built sites, published by
[CI site publication jobs](../../.github/workflows/ci.yml) to Yandex Object Storage:

| Domain and bucket | Content | Storage limit |
| --- | --- | --- |
| `kedra.tech` | English landing and `/ru/` | 64 MiB |
| `docs.kedra.tech` | English/Russian manual, Luau and complete English Rust API | 2 GiB |

The buckets contain only managed public site output, never project sources, credentials,
private artifacts or unrelated data. Storage, requests, egress and DNS are billable;
the storage limits are not spending caps. No VM, CDN or second source repository is needed.

## Publication gate

Official `main` push CI calls publication only after its `CI required` job succeeds.
A maintainer may also dispatch CI on `main`; the same scheduler dependency applies.
The source must still be current `main` before uploading. Failed, skipped, fork and PR
runs cannot publish. Keep protected-main rules and review requirements enabled. The
`sites-build` and `sites-deploy` jobs run within CI; there is no privileged
`workflow_run` trigger or independent manual entry point. Retry a failed upload by
rerunning failed jobs of the eligible CI run, or dispatch CI on current `main`.

The read-only build job installs frozen dependencies without lifecycle scripts, audits
both graphs, runs landing contracts, builds both sites and checks generated links.
Documentation uses `docs/`'s complete `build:api`, including license texts, rustdoc
resources and bounded HTML adaptation. The required PR job's faster manual build remains
Rust-free. Landing builds set `KEDRA_DOCS_URL=https://docs.kedra.tech/`.

The deployment job does not check out source or execute its build scripts. It downloads
only the current run's two artifacts, checks their entry pages/notices, rechecks source
freshness and uses pinned AWS CLI 2.34.0 for S3 synchronization.

Synchronization owns the entire two fixed buckets and uses `--delete` to remove obsolete
output. It is not atomic across objects or sites. A failed transfer can leave partial
output; fix the cause and rerun the approved revision. The CLI reports failures, and a
later site is not uploaded after an earlier failure. Objects use `Cache-Control: no-cache`
so unchanged names, HTML and API search resources are revalidated.

## Credentials and permissions

GitHub environment `sites-production` accepts only branch `main`. It contains:

- `YC_SITES_ACCESS_KEY_ID`
- `YC_SITES_SECRET_ACCESS_KEY`

Both are encrypted GitHub environment secrets, mapped to AWS environment variables only
in the upload step. Do not put them in source, artifacts, URLs, command arguments or logs.
Builds and pull requests do not receive the key. No folder-wide publisher role is required.

Service account `kedra-sites-publisher` (`ajetie3i301pghia7bkk`) has no folder roles.
Its READ/WRITE ACL grants apply only to these two buckets and provide base access.
[Landing policy](landing-policy.json) and [documentation policy](documentation-policy.json)
then restrict that access to the required listing and object operations. Both mechanisms
are required: policies alone do not supply base authorization, while ACLs alone also
expose configuration operations. Never remove a policy while a publisher key is active.
The human owner retains separate bucket administration. Anonymous object read is enabled;
anonymous listing and configuration access are disabled. Do not grant the publisher
storage admin, DNS, Certificate Manager, IAM, compute or registry roles.

Object Storage can expose policy metadata to an authenticated publisher independently
of its policy-operation filters. Policies contain resource and subject IDs, not secrets.
Live acceptance must still refuse ACL/website configuration reads and website/policy
writes; policy metadata visibility is not permission to change it.

Rotate the static key with `yc iam access-key create`, transfer both returned values
directly to `gh secret set --env sites-production` over stdin without printing or saving
them, verify publication, then revoke the old key with `yc iam access-key delete`.
If setup fails, revoke the incomplete new key rather than leaving an untracked credential.
Never share the unrelated compiler-cache account or its key with this deployment.

## Cloud configuration

Resources belong to the project cloud `cloud-kedra`, folder `b1g892pnva968bknaq84`.
Website settings are in [landing configuration](landing-website.json) and
[documentation configuration](documentation-website.json). Nested directories use
`index.html`; documentation also supplies its real `404.html`. No SPA fallback hides
missing reference pages. Do not reset bucket contents when applying website settings.

Managed Certificate Manager certificate `fpqabvb4d7cm19376f0t` covers `kedra.tech` and
`docs.kedra.tech`. Bind it to each bucket with `yc storage bucket set-https` after issuance.
Keep deletion protection and both DNS challenge CNAMEs for renewal. Certificate private
keys are managed by Yandex Cloud and are not supplied to CI.

The existing public zone is `dns7seihajbngl7qtncl`. Preserve its delegation and other
records. The landing uses an apex ANAME to `kedra.tech.website.yandexcloud.net.`;
`docs` uses a CNAME to `docs.kedra.tech.website.yandexcloud.net.`. Do not create wildcard
records or replace NS/SOA/mail records. Save a record-set snapshot under ignored `target/`
before DNS writes. HTTPS and HTTP-to-HTTPS redirects are provided by Object Storage.

## Acceptance and recovery

Run `cargo xtask check`, site builds/audits/contracts, rumdl, Lychee and actionlint.
Check the workflow security audit configured in CI. Then verify the actual hosted
workflow, anonymous listing refusal, publisher configuration refusal, DNS, certificate
status, HTTP redirects, HTTPS, locale links, search, fonts/notices and Rust API pages.
Compilation, local files and provisioned buckets alone do not establish live publication.

On failure, retain diagnostics without credentials and retry through the workflow.
Do not widen publisher permissions, bypass main protection, suppress audits or upload
an unvalidated local tree to force a deployment. Certificate/DNS changes are operator
tasks, not permissions to give the CI publisher. Site publication does not establish
native editor acceptance or independently clear artwork rights.
