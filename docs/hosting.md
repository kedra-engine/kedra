# Repository administration

Repository: <https://github.com/kedra-engine/kedra>.
GitHub CI, protection rules, security channels and platform capabilities have
not been configured or verified. Local source files do not apply server settings.

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

No CI provider configuration or release workflow is installed. Possible future
self-hosted Yandex Cloud workers are not provisioned or authorized by this document.
Worker isolation, budgets, credentials and untrusted contribution handling need
separate approval and verification; see [CI requirements](ci.md).

Before publishing, define protected tags, explicit approval, least-privilege tokens,
artifact provenance, license notices, clean-machine verification and rollback.
Never promote untrusted contribution artifacts directly into releases.
