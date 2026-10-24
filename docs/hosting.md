# Repository administration

Repository: <https://github.com/kedra-engine/kedra>; the administrative contract is
a public repository with default branch `main`. The expected `Protected main`
ruleset (25504920) requires PRs, resolved review threads, an up-to-date base and
`CI required` from GitHub Actions (app 15368). It must deny force pushes and branch
deletion and have no bypass actors. Required human approvals are zero for the
single-maintainer model. Live settings, rule identities, current hosted results
and confidential contacts require owner confirmation. Local source files alone
do not apply or prove server settings.

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
- Verify exact configured required result identities on the destination.
  Missing, failed, cancelled or skipped required checks must prevent acceptance.
- Review outside-contributor execution before approval; untrusted code must not
  receive secrets, privileged tokens or access to trusted worker state/caches.
- Verify available dependency alerts, secret detection and push protection rather
  than claiming they are enabled. Keep the local audits in
  [testing](site/en/contributing/contracts/testing.md).

## Automation and releases

The [GitHub workflow](../.github/workflows/ci.yml) uses managed runners with
read-only repository permissions, dependency-only caching and no private
infrastructure credentials. It validates changes, separately from the manually
dispatched [Desktop distribution workflow](../.github/workflows/release.yml).
Neither workflow configures Kubernetes or self-hosted runners;
see [CI boundaries and acceptance](ci.md).

Preserve the required safeguards and verify legitimate PR acceptance after checks pass;
never bypass it to make automation convenient. The single-maintainer model must
not demand a second unavailable human reviewer. A YAML job name alone is not
server-side enforcement; see [configured CI checks and acceptance requirements](ci.md).

Release preflight checks exact source identity, main ancestry, successful push CI
and unused release identity; see
[distribution](site/en/contributing/contracts/distribution.md#release-orchestration-boundary).
Before publishing, confirm live protected tags and explicit approval, least-privilege
tokens, artifact provenance, license notices, clean-machine verification and rollback.
Never promote untrusted contribution artifacts directly into releases.
