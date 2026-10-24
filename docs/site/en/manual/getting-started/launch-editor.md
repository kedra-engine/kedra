# Prepare resources and launch the editor

Start from the repository root after completing the [prerequisites](index.md).
Kedra loads its interface from a processed asset delivery, not directly from the authored
`assets/editor` folder. There is no installer step in this source-checkout route.

## 1. Build the editor delivery

The destination **must not exist**, even as an empty directory. Its parent may exist.
These examples use outputs below `target/documentation/examples/getting-started/`, outside
the authoring sources. If an output already exists, choose a new name and use that name
consistently in later commands. Do not delete or overwrite a delivery to rerun the example.

```sh
cargo run --locked -p kedra -- assets build assets/editor target/documentation/examples/getting-started/editor-delivery --namespace editor
```

Expected result: processing progress, native dependency validation and a final
`SUCCESS: … assets delivered` message. Build never writes changes back to these sources.
The editor sources in the repository already have sidecars; do not run `assets init` on them.

## 2. Verify the delivery

```sh
cargo run --locked -p kedra -- assets verify target/documentation/examples/getting-started/editor-delivery
```

Expected result: `SUCCESS: … assets verified`. Verification is read-only and checks the
delivery's manifest, hashes and loader metadata. It does not open a window or test rendering.
Keep this delivery intact while using the editor.

## 3. Open Projects

The following command **launches a native application**; run it when you are ready to use
your desktop. The explicit state directory keeps this walkthrough's local preferences
separate from your usual editor preferences:

```sh
cargo run --locked -p kedra -- --assets target/documentation/examples/getting-started/editor-delivery --state-directory target/documentation/examples/getting-started/editor-state
```

After startup verification, the **Projects** window offers **Open folder…** and
**New project**. There is no splash window. Project preparation can enter the workspace
directly or briefly show a loading window; neither path skips validation.

An installed application needs both its executable and a processed editor delivery; Kedra's
UI assets are not embedded in the executable. Without `--assets`, lookup uses `KEDRA_ASSETS`,
then the platform's bundled location relative to the executable, not the working directory
or authoring checkout. Portable/development builds use adjacent `assets`; macOS app bundles
use `Contents/Resources/assets`, and Linux `usr/bin` layouts use `usr/lib/<executable>/assets`.

1. Select **Open folder…**.
2. Use **Browse…** to choose `tests/fixtures/test_project` inside your checkout.
   Select the directory containing `kedra.toml`, not `assets/` or the manifest file.
3. If typing **Project folder**, enter its absolute native path. Relative text such as
   `tests/fixtures/test_project` is not accepted by this form.
4. Select **Open project**.

Expected result: the **Kedra Test Project** workspace, with Assets, Hierarchy and Inspector
tools and an initially empty document-preview area. Opening a project does not open a
document automatically. Continue to [your first document](first-document.md).

## Optional: launch directly with rendered previews

Close the editor before relaunching. Build and verify a separate project delivery:

```sh
cargo run --locked -p kedra -- assets build tests/fixtures/test_project/assets target/documentation/examples/getting-started/project-delivery --namespace game
cargo run --locked -p kedra -- assets verify target/documentation/examples/getting-started/project-delivery
```

Keep the default **`editor-desktop`** profile for both commands. The namespace is `game`,
but a `game-desktop` profile is not the profile this editor preview binding expects.
The repository's test project already has metadata; do not initialize or edit its canonical sources.

This next command launches the editor directly into the fixture with both deliveries:

```sh
cargo run --locked -p kedra -- tests/fixtures/test_project --assets target/documentation/examples/getting-started/editor-delivery --project-assets target/documentation/examples/getting-started/project-delivery --state-directory target/documentation/examples/getting-started/editor-state
```

The positional project path can be relative to the terminal's repository root, unlike the
Projects form. `--assets` supplies editor resources; `--project-assets` supplies the
project's preview resources and requires an explicit positional project. Neither builds
assets at launch or executes project code. Source/delivery matching happens on document Open.

## If preparation fails

- For compiler/linker or missing-library errors, revisit the [platform prerequisites](index.md).
- For an existing output directory, select a fresh output path; do not reuse a partial build.
- For a missing delivery marker or failed hashes, rebuild from trusted unchanged sources
  into a fresh directory, then verify that directory. `assets/editor` itself is not a delivery.
- For project validation errors, check the selected root and read the startup diagnostic;
  see [project recovery](../projects/create-open.md#preparation-and-failures).
- For preview errors after a successful launch, use the
  [matching-delivery guide](../assets/pipeline.md#editor-previews-need-matching-sources).

For a new empty project, use [Create an empty project][empty-project] instead of copying
the fixture's manifest as a project template.

Contributors changing boot timing, asynchronous preparation or native window lifetime should
read the [startup engineering contract](../../contributing/contracts/startup.md).

[empty-project]: ../projects/create-open.md#create-an-empty-project
