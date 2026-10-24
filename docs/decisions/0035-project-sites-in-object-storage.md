# ADR 0035: Project sites in Yandex Object Storage

Status: accepted; production deployment requires successful protected-main CI.

## Decision

Keep both sites in the main GitHub repository and publish their static output to
separate Yandex Object Storage buckets:

- `kedra.tech`: English landing and `/ru/` translation at `https://kedra.tech/`.
- `docs.kedra.tech`: English/Russian manual, Luau reference and complete English
  Rust API reference at `https://docs.kedra.tech/`.

The owner authorizes the existing working tree, both buckets and secret-backed CI
publication. No second GitHub repository, Pages deployment, VM, Kubernetes cluster or
CDN is required. A managed Certificate Manager certificate serves both domain names;
Cloud DNS uses the existing zone, an apex ANAME and a documentation CNAME. Keep the
certificate's DNS validation CNAMEs for automatic renewal.

## Build and publication authority

The read-only required repository job validates both sites on contributions. Separate
publication jobs in CI run only after the scheduler reports `CI required`
successful for the same main push or explicit main CI dispatch. It rejects a superseded
main revision before upload. Protected-main and PR safeguards remain.
No pull request, fork or arbitrary supplied source reference receives publication authority.

Build jobs receive no Object Storage credentials. They run frozen installs, dependency
audits, landing contracts, both site builds and generated-link checks. Publication uses
the complete `build:api` route, not the manual-only CI preview; preserve rustdoc's
fonts, notices, source links and bounded HTML compatibility checks.

A separate deployment job downloads only the current workflow's artifacts, without
checking out or running source build scripts. The `sites-production` environment admits
only `main`. Its two encrypted GitHub secrets supply a dedicated static S3 access key
to the AWS CLI upload step, never to PRs, builds, source files or logs.

The publisher service account has no folder-level roles. Per-bucket READ/WRITE ACLs
provide the base access required by Object Storage; restrictive bucket policies then
allow only the listing and object operations required for synchronization. Neither
mechanism alone is sufficient: ACLs alone expose configuration operations, while policies
do not supply the initial IAM/ACL authorization. Never remove the policy while a publisher
key remains active. The owner retains separate administration of these two buckets.
Anonymous readers can fetch objects but cannot list buckets or read configuration.
The CI key cannot manage certificates, DNS, IAM or bucket settings.

## Deployment semantics and cost

AWS CLI owns S3 synchronization; do not introduce a custom uploader or crawler. Each
bucket is exclusively managed static output, so synchronization may delete obsolete
objects within that fixed bucket. Uploads are not an atomic transaction across files or
sites; failure can leave partial output. Fix the cause and retry the approved revision.
Objects use revalidation rather than long-lived stale HTML/search caches.

The landing storage limit is 64 MiB; documentation is limited to 2 GiB. These are
storage limits, not spending caps: requests, egress and DNS remain metered. Preserve
existing unrelated buckets, service accounts and DNS records. Key rotation changes
GitHub environment secrets; it must not expand the account's permissions.

## Alternatives and verification

GitHub Pages would require two hosting repositories for the selected domain layout;
the owner chooses Object Storage instead. CDN and server-based hosting add resources
without a current requirement. Static-key publication is explicitly requested; identity
federation may replace it separately without weakening the deployment gate.

Verify real IAM refusal paths, secret presence, bucket settings, certificate status,
DNS answers, hosted CI, HTTP(S), locale links, search and Rust API transitions. Local
builds and configuration do not establish live deployment or native editor acceptance.
The resource configuration and operator procedure are in
[static-site hosting](../../tools/hosting/README.md).
