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
Bevy plugins nor an API capability check constitutes a sandbox. Opening project
sources grants no permission to execute their code.

Asset/document parsers and model admission process source data under explicit
validation and size limits, not a hostile-filesystem or process sandbox. Luau
execution requires host authorization and trusted, finite host-owned bindings.
Its scopes and budgets bound supported operations and lifetimes; they do not
isolate hostile code or preempt native callbacks.

Application automation is compile-time debug-only and off unless explicitly
enabled. Its authenticated transport binds loopback, publishes a fresh private
credential file and grants application-user capabilities, not OS isolation.
Keep credentials out of reports and shared artifacts; on Windows the containing
directory needs an appropriate ACL. Release excludes the transport and diagnostic
input/capture API. Expanded parser, scripting or network authority requires an
explicit threat model and tests. See
[automation](docs/site/en/contributing/contracts/automation.md) and the
[Luau runtime contract](crates/kedra_script/README.md).

Dependency advisories and secret scanning are useful detection tools, not a
guarantee that code or released artifacts are free of vulnerabilities.
