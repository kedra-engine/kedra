# Build and distribution

For exact builder and runtime APIs, use the [Rust API guide](../../api/index.md)
alongside this chapter's CLI workflows and repository distribution contracts.

A standalone application is an ordinary Rust executable with the resources it needs
outside the source checkout. Kedra's project-local `cargo kedra` command compiles that
application, prepares a verified asset delivery and assembles a runnable build. A separate
command creates native installable and portable packages.

The build tooling does not require an installed editor. It is not an editor Build button,
a game generator or an implementation of Play.

## Choose the right starting point

There are three different kinds of input in the repository:

| Input | What it provides | What it does not provide |
| --- | --- | --- |
| A new editor project with `kedra.toml` and empty `assets/` | Authoring identity and source location | A Cargo application, startup code or runnable game |
| An ordinary Rust application with Cargo metadata | Executable code and explicit distribution inputs | Automatic integration of every Bevy/Kedra capability |
| A checked acceptance fixture | A narrow example used to verify a contract | A published game SDK or supported game template |

Creating or opening an editor project does not authorize project-defined code execution.
The host application still chooses its plugins, resource loading, startup scene and any
script execution explicitly. The editor's read-only Scene View is not a running game.

To try the builder, use the repository's configured `apps/kedra` package. Building it
produces the editor, not a game from the currently open project. Run the commands in this
chapter from the repository root and use new output paths
under `target/documentation/examples/build-and-distribution/`.

The examples use that package's source configuration. Builds, packages and launches still
need testing on your machine.

## The workflow

1. Review and trust the application, its dependencies, build scripts and Cargo configuration.
2. Install Rust and the native platform prerequisites.
3. Check the application's Cargo metadata, source assets, sidecars, icon and notices.
4. Use `build` to create a new verified runnable assembly.
5. Use `run` only when you intend to launch a newly built application.
6. Use `package` when you also need native and portable distribution artifacts.
7. Validate installed behavior, licensing and platform acceptance before publication.

Do not run builds on untrusted projects. Cargo compiles and executes project build scripts
and procedural macros. Native packaging can download and execute upstream helper tools.
Output validation does not sandbox either activity.

## What each command means

| Command | Result | Launches the application? |
| --- | --- | --- |
| `cargo kedra build` | Compiled executable, delivery, notices and `build-result.json` | No |
| `cargo kedra run` | The same fresh build, then an explicit launch request | Yes |
| `cargo kedra package` | Release build plus inspected native/portable packages and inventory | No |
| `cargo xtask check` | Repository Rust validation | No; not a distribution build |

`build` and `run` use a development build unless `--release` is supplied. `package`
always requests Cargo's release profile. Independent applications retain their own Cargo
profile policy; the tool does not copy the editor's optimization settings into them.

Every output directory must be new. A failed operation can retain partial staging for
diagnosis, but it is not a successful distribution. Choose a different path for a retry;
do not reuse or overwrite an earlier result.

## Follow the section

- [Configuration](configuration.md): launcher, Cargo metadata and input restrictions.
- [Build and run](build-and-run.md): assembly, verified handoff, explicit launch and recovery.
- [Packaging and licenses](packaging-and-licenses.md): formats, platform tools, notices,
  source obligations and publication limits.
- [Projects](../projects/index.md): authoring projects and persistent identities.
- [Assets](../assets/index.md): sources, sidecars and processed deliveries.

## Acceptance is a separate step

A valid executable and a matching delivery establish useful structural guarantees.
They do not establish a rendered frame, working input, a successful application exit,
installation/uninstallation behavior or operating-system trust.

The configured desktop targets are Windows x64 MSVC, Linux x64 GNU, macOS ARM64 and
macOS Intel. The target must match the native host. Web/mobile builds, cross-platform SDK
bootstrapping and a certified platform matrix are not provided by these commands.

Use the [builder CLI reference](../cli/build-tool.md) for exact options and
[Testing](../testing/index.md) for the evidence needed beyond a successful build.
