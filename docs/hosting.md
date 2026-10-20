# Repository administration

Repository: <https://github.com/kedra-engine/kedra>, public, default branch `main`.
The initial complete GitHub Actions run passed; comparable warm acceptance is
pending. Active ruleset `Protected main` (25504920) requires PRs, resolved review
threads, an up-to-date base and `CI required` from GitHub Actions (app 15368).
It denies force pushes and branch deletion and has no bypass actors. Required
human approvals are zero for the single-maintainer model. Server rules were read
back; a real green PR still needs acceptance. Confidential contacts remain
unverified. Local source files alone do not apply or prove server settings.

## Before wider public contributions

- Verify account protection, recovery access and a trusted backup owner without
  sharing accounts or credentials. Use two-factor authentication where available.
- Establish confidential security and conduct contacts; publish only verified
  channels. Do not assume platform reporting is private or reaches maintainers.
- Configure reviewed change requests, resolved conversations and up-to-date source
  requirements; block force pushes/deletion and explicitly limit bypass permissions.
- Require an independent approval when another maintainer is available; document
  solo-maintainer limitations rather than claiming independent review is enforced.
- Establish real maintainer ownership for policy, automation and public APIs.
- After CI exists, verify exact required result identities on the destination.
  Missing, failed, cancelled or skipped required checks must prevent acceptance.
- Review outside-contributor execution before approval; untrusted code must not
  receive secrets, privileged tokens or access to trusted worker state/caches.
- Verify available dependency alerts, secret detection and push protection rather
  than claiming they are enabled. Keep the local audits in [testing](testing.md).

## Automation and releases

The [GitHub workflow](../.github/workflows/ci.yml) uses managed runners with
read-only repository permissions, dependency-only caching and no private
infrastructure credentials. No release workflow, Kubernetes cluster or self-hosted
runner is installed by it; see [CI boundaries and acceptance](ci.md).

Preserve the active ruleset and verify legitimate PR acceptance after checks pass;
never bypass it to make automation convenient. The single-maintainer model must
not demand a second unavailable human reviewer. A YAML job name alone is not
server-side enforcement; see [actual CI status and measurements](ci.md).

Before publishing, define protected tags, explicit approval, least-privilege tokens,
artifact provenance, license notices, clean-machine verification and rollback.
Never promote untrusted contribution artifacts directly into releases.
