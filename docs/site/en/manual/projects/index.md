# Projects

A project is a directory of Kedra authoring sources, identified by `kedra.toml`.
It gives the editor a persistent project identity, a display name and an asset source directory.
Open a project when you want to browse its sources and inspect supported scenes, prefabs or assets.

Creating a project does not create a runnable game. The current editor does not compile or execute
project code, provide Play, edit scene documents visually or save document changes.

## What belongs to a project?

A newly created project contains only this structure:

```text
My Game/
├── kedra.toml
└── assets/
```

The asset directory starts empty. Creation does not add a scene, prefab, Cargo package,
script entry point or processed asset delivery. Additional files can belong to a project,
but their presence does not make them executable by the editor.

Keep authored files and their existing `.meta` sidecars together when copying a populated project.
Sidecars contain asset identities and processing settings; they are not disposable preferences.
Processed deliveries and local editor preferences have different roles from these sources.

## The project manifest

This is the complete manifest of the repository's test project at
`tests/fixtures/test_project/kedra.toml`:

```toml
id = "7e570000-0000-4000-8000-000000000000"
format = 1
name = "Kedra Test Project"
editor_version = "0.1.3"
assets = "assets"
```

This is an existing project's identity, not a template UUID to reuse for new projects.
Use **New project** to obtain a fresh identity.

| Field | Meaning |
| --- | --- |
| `id` | Non-nil persistent project UUID; moving the directory does not change it. |
| `format` | On-disk project schema version. Only `1` is currently supported. |
| `name` | Human-readable project name, not a Cargo package or executable name. |
| `editor_version` | Informational writer version; must not be blank. Opening does not update it. |
| `assets` | Existing asset directory, relative to the project root. |

All five fields are required, and unknown fields are rejected. The manifest must be valid UTF-8
TOML and no larger than 64 KiB. Compatibility depends on `format`, not `editor_version`:
a different writer release alone does not prevent opening a format-1 project.
There is no automatic migration for unsupported formats.

The `assets` value can name a nested directory, but cannot be empty, absolute or contain parent
traversal. After resolving filesystem links, the directory must remain inside the project root.
Relative paths in this manifest are not relative to the terminal's working directory.

## Opening is not full content validation

Project opening validates the manifest and the asset directory's location. It does not prove that
every source document is valid or every asset is ready to render.

When you open a supported source from the Asset Browser, the editor admits a source inventory
into one project-owned source workspace. Document validation and reference checks happen at that
stage. A malformed document elsewhere in the same inventory can therefore prevent admission,
even after the project itself opened successfully.

Rendering is another boundary: Scene View and supported asset previews require an explicit,
matching verified project asset delivery. Opening neither builds that delivery nor grants
permission to execute code. Successful source inspection is not evidence that Play is available.

## Moving and sharing

Move the complete project directory, keeping the manifest, relative asset directory and sidecars.
The manifest's project UUID remains unchanged; the native location changes.
Use **Open folder** at the new location if a recent-project entry still points to the old one.
Recent-project history is local preference data, not a portable project index or backup.

Copying a directory also copies its identities. This is useful for an unchanged working copy,
but is not an independent-project creation operation or an automatic identity-remapping tool.

## Next steps

- [Create or open a project](create-open.md) for the supported workflow and recovery steps.
- [Paths and identity](paths-and-identity.md) to distinguish source names, locations and UUIDs.
- [Assets](../assets/index.md) to distinguish authored sources from prepared deliveries.
- [Build and distribution](../build-and-distribution/index.md) for a separately configured
  Cargo application, not a runnable template created by the editor.
