# Luau API reference

Kedra's host-injected API supports game scripts, diagnostics and automation. Its interfaces
are unstable. For the standard Lua/Luau library, use the language's own reference.
English is canonical.

## Choose a task

| I want to… | Start here | What is available now |
| --- | --- | --- |
| Write game behavior | [Game scripting](game-host.md) | Behaviors, finite Sequences and opt-in live-object operations |
| Automate the editor | [Editor diagnostics](editor.md) | Read-only editor observations in named debug programs, not project plugins |
| Drive a debug application | [Application automation](application.md) | UI lookup, input, resize and renderer captures |
| Work on a test scenario | [Test fixtures](testing.md) | Named editor diagnostics and the test project's custom projection API |
| Integrate Luau in Rust | [Host integration](integration.md) | Admission, capability installation and declaration ownership |

## Common foundation

All these tasks use the same [runtime reference](runtime.md): entrypoints, parameters,
waits, logs, events and child Sequences. Read [context and value conventions](conventions.md)
when interpreting a signature. Available capabilities depend on the host.

For a first explanation and the complete switch example, use the
[programming manual](../../manual/programming/luau.md). Use this reference to look up
signatures, results and errors; use the manual for setup and walkthroughs.

## Availability and verification

Shared runtime and authorized live-object bindings are available in release. Application
automation and editor diagnostic registrations require `debug_assertions`; the native
scenario named `luau` is a test selection, not a Cargo feature boundary.

There is no universal `ctx.game`, editor plugin loader, automatic project-code execution
or in-editor Play. See [implementation limitations](limitations.md) for optional targets,
fixture geometry and resource limits.

Examples link to their checked-in sources. Native input and rendered frames require
separate native tests.
