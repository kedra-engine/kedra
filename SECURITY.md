# Security policy

Kedra is pre-release. There are no supported production releases or response-time
guarantees yet; security fixes target the default branch.

## Report privately

The project repository is <https://github.com/kedra-engine/kedra>.
No dedicated confidential maintainer contact has been established, and private
vulnerability reporting on GitHub has not been configured or verified.
Do not assume a platform report is private or reaches the maintainers.

Do not publish exploit details, credentials or private data in an issue. Ask for
a private security contact using a minimal non-sensitive issue or existing project
discussion, and wait for a verified confidential channel before sharing details.
Establishing that channel is a prerequisite for wider public contributions;
see [hosting](docs/hosting.md).

Include the affected revision, impact, prerequisites and a minimal reproduction.
Never test against other people's systems or submit real secrets as evidence.

## Threat boundaries

Native project plugins execute with their host process's privileges. Neither
Bevy plugins nor an API capability check constitutes a sandbox. Future asset
parsers, scripting and network tooling require explicit threat models and tests.

Dependency advisories and secret scanning are useful detection tools, not a
guarantee that code or released artifacts are free of vulnerabilities.
