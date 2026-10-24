# Troubleshoot asset delivery

Asset failures can come from invalid sources, a corrupted delivery or a preview that no
longer matches its sources. Keep the original sources and sidecars intact while diagnosing.
Do not regenerate identities or patch manifest hashes just to make an error disappear.

## Start with the delivery you actually use

For the project delivery from the [walkthrough](pipeline.md), run:

```sh
cargo run -p kedra -- assets verify target/documentation/examples/assets/project-delivery
```

Check the complete diagnostic and the final `SUCCESS` or `FAILED` message.
A source directory is not a delivery: verify expects its `kedra-assets.json` marker.
Use an explicit root to avoid accidentally verifying a bundled or environment-selected
delivery. If build used a different `--profile`, pass that same profile to verify.

## Build fails before publication

| Symptom | Meaning and recovery |
| --- | --- |
| `initialize asset metadata first` | A source has no readable adjacent `.meta`. On your own source copy, run `assets init SOURCE`; review the new sidecars. |
| Invalid or duplicate UUID | Inspect the named sidecar and document identity. Restore intended metadata from version control; copying sidecars is not a safe way to create distinct assets. |
| Document UUID differs from metadata | The scene/prefab and its sidecar disagree. Restore the intended matching pair rather than assigning an unrelated UUID. |
| Output must be a new directory | Choose a new path. Even an empty or incomplete old output is not reused. |
| Source and delivery roots must be disjoint | Move the output outside the source tree and ensure the source is not inside the output. |
| Invalid source asset path | Remove traversal/URI delimiters from physical names and update references deliberately; a literal `#` is not a label in a filename. |
| Invalid metadata or unknown processor | Check the authored RON and exact processor type path. Initialization deliberately does not replace existing settings. |
| Processing or dependency load failed | Inspect the named resource and its imports/references. Check the [admitted format and settings](supported-formats.md), not only its extension. |
| Sources or inventory changed | Stop concurrent edits/generation and retry into a fresh output from a stable source snapshot. |
| Manifest exceeds 64 KiB | Reduce the delivered inventory, for example by explicitly ignoring genuinely source-only data in your own tree. Do not raise reader limits or edit the generated marker. |

Initialization may have written some missing sidecars before another source fails. Those
successful assignments remain. Build failures do not rewrite authoring sources or existing
deliveries. A final I/O/publication failure can leave an incomplete new directory; retain
it for diagnosis and choose another destination rather than treating it as valid output.

## Verify fails or runtime loading rejects a resource

Missing members, changed bytes, unexpected loader metadata and format/profile mismatch
reject the delivery. Restore the complete trusted delivery or rebuild from sources into
a new directory. Mixing members from different builds is not a repair.
An extra file copied into the directory is not automatically loadable: runtime lookup only
admits assets declared by the manifest.

For an unresolved logical reference, check source registration, exact case/Unicode spelling,
the delivered path, asset identity and any `#Label`. An ignored source has no delivered
entry. A variant image package only provides its authored density labels. Neither an
absolute path nor a traversal path bypasses the verified reader.

For model rejection, inspect topology and expansion as well as file validity. Cycles,
multiple parents, excessive depth and scene expansion fail even when hashes match.
Renaming a GLB or marking it ignored does not bypass import admission.

Checksums detect corruption relative to the manifest; they do not establish who produced
it. Only use deliveries from a trusted origin. Keep a registered delivery immutable;
portable containment checks are not an OS sandbox against concurrent file replacement.

## Verify succeeds, but the editor cannot preview

Standalone verify checks delivery integrity, not correspondence with your open project.
See the [matching rules](pipeline.md#editor-previews-need-matching-sources).
Confirm that `--project-assets` was supplied with an explicit project, the delivery was
built with namespace `game`, and its profile is `editor-desktop`.
The editor's `--assets` option supplies the shell resources, not the project's preview data.

Rebuild after changing document bytes, resource bytes, paths or sidecar identities.
Relaunch with the new delivery; switching to a different project's source root cannot reuse
an earlier binding. Source navigation can remain usable while rendering reports a mismatch.

If source admission itself fails, review
[project source admission](../projects/paths-and-identity.md).

Processed resources are a separate limitation: a valid rasterized SVG delivery does not
match source SVG bytes under the current preview policy. Repeatedly rebuilding that same
input does not add the missing source-provenance support. Direct asset Open currently
supports PNG and static GLB, not every format that the runtime can load.

Loaded dependencies do not establish that the renderer produced a frame. Check GPU readiness
and preview interaction in the running editor, not through headless build/verify commands.
These commands do not implement editor editing, Save or Play.

Review the [asset mental model](index.md), [pipeline guarantees](pipeline.md) and
[format-specific limits](supported-formats.md) before changing source settings.
