# Create or open a project

Use the Projects window to create a source directory or open an existing Kedra project.
You need a working editor installation with its verified editor asset delivery.
Creating also requires an existing writable parent directory; opening requires a readable
project root containing `kedra.toml` and its configured asset directory.

For build prerequisites and initial setup, follow [Getting started](../getting-started/index.md),
then [prepare resources and launch the editor](../getting-started/launch-editor.md).
For delivery details, see [Process and verify assets](../assets/pipeline.md).

## Create an empty project

1. In Projects, select **New project**.
2. Enter **Project name**, for example `My Game`.
3. Set **Location** to an existing parent directory, using **Browse…** or entering its path.
4. Review the destination shown under **Create new folder**: the parent plus the project name.
5. Select **Create & open**.

For example, selecting an existing `Projects` parent and the name `My Game` creates
`Projects/My Game/kedra.toml` and an empty `Projects/My Game/assets/`.
The editor enters the project workspace; there are no game sources or starter scene to inspect yet.
See [Projects](index.md#the-project-manifest) for the resulting structure and manifest fields.

Names retain Unicode, including names such as `Кедра 日本語`. They must fit within 160 UTF-8 bytes,
not 160 characters. Empty names, surrounding whitespace, trailing dots, control characters,
`.` and `..`, and `< > : " / \ | ? *` are rejected. Windows device names such as `CON` and
`lpt1.txt` are rejected on every platform to keep the directory name portable.

The destination must not already exist, even as an empty directory or a symlink.
Create does not turn an existing folder into a project or overwrite its contents.
Form validation checks availability but does not reserve the name; creation checks again.

## Open an existing project

1. In Projects, select **Open folder…**.
2. Set **Project folder** to the directory containing `kedra.toml`, not to the manifest itself
   or to its `assets/` child. You can use **Browse…**.
3. Select **Open project**.

The entry forms accept an absolute native directory path. They also expand `~` or `~/…`
against the current account's home directory; arbitrary shell expansion is not provided.
Relative directory text is rejected in these forms.

As a populated example from a Kedra checkout, choose `tests/fixtures/test_project` using Browse.
Its manifest names the project **Kedra Test Project** and points to `assets/`.
The resulting workspace can browse the test project's source files, including `Hud.prefab.ron`.
Use a complete copy before making external edits; do not modify the canonical fixture.
Opening the project does not automatically open a document or guarantee its rendered preview.

Continue with [your first document](../getting-started/first-document.md) for fixture inspection.

### Select a project at launch

The editor CLI accepts an existing project root as its positional argument.
From the repository root, with an already prepared editor delivery at `target/editor-assets`:

```sh
cargo run -p kedra -- tests/fixtures/test_project --assets target/editor-assets
```

This requests the same validated project-opening lifecycle as the Projects window.
The command does not build project assets. Without `--project-assets`, do not expect the project
sources to have a render-ready delivery. That option requires an explicit positional project
and a separately prepared matching delivery; it is not a project creation or execution switch.
There is no project-creation CLI subcommand in the current editor executable.

See the [preview binding requirements](../assets/pipeline.md#editor-previews-need-matching-sources)
before expecting rendered content.

## Preparation and failures

Fast preparation can enter the workspace without a separate loading window.
Longer work can show a loading window. Do not interpret a short or absent loading view as a
skipped validation step.

If validation fails in the entry form, correct the name or directory and submit again.
If preparation fails later, read the diagnostic and startup log. A failed Open can be retried
after fixing the cause. A failed Create has no automatic retry: return to Projects and inspect
the destination before submitting another creation request.

Common causes include:

- A missing, unreadable or malformed `kedra.toml`, unknown fields or an invalid project UUID.
- An unsupported `format`; opening does not migrate or rewrite the project.
- A missing asset directory, an absolute asset path or an asset root escaping the project.
- An existing creation target, missing parent or filesystem permission failure.

Opening is read-only with respect to project sources, including a rejected Open.
Creation publishes the manifest last and never clobbers an existing target.
An interruption can nevertheless leave an incomplete directory. Do not assume cancellation or
closing a window rolls back published files; preserve and inspect unexpected content yourself.
The filesystem checks are not a sandbox against another process replacing paths concurrently.

## Recent projects

Successful preparation remembers up to 20 projects, newest first, deduplicated by canonical root.
Select an available row and use its Open control. Missing locations stay in the list with
recovery controls rather than being silently deleted. Browse to a moved project's new root;
the list does not discover moves by UUID.

History is stored separately from sources. Corrupt history is reported and left untouched;
history saving is disabled for that session. A history-write failure warns but does not invalidate
the successfully opened project. Concurrent history writers use last-writer-wins behavior.

## Verification limits

Test the system folder picker, keyboard/IME, window chrome and mixed-DPI behavior on your
target desktop. Cross-platform testing is incomplete; source validation and documentation
builds do not check these interactions.

## Related pages

- [Projects](index.md)
- [Paths and identity](paths-and-identity.md)
