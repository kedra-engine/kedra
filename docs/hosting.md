# Repository administration

Destination: <https://github.com/kedra-engine/kedra>.
GitHub Actions CI is defined locally. Publication, hosted acceptance, main rules
and confidential security channels remain pending verification. Local source
files do not apply server settings or prove the actual rules are active.

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

After full hosted acceptance, require the actual GitHub Actions `CI required`
result on main, PR-only changes, an up-to-date base and resolved conversations,
with force pushes and branch deletion denied and no silent bypass actors.
The single-maintainer model must not demand a second unavailable human reviewer.
Read back and test native repository rules; a YAML job name alone is not enforcement.

Before publishing, define protected tags, explicit approval, least-privilege tokens,
artifact provenance, license notices, clean-machine verification and rollback.
Never promote untrusted contribution artifacts directly into releases.
