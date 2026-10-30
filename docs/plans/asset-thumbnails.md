# Bounded asset thumbnails

Status: partially implemented. Current contracts and limits are in
[ADR 0050](../decisions/0050-project-cache-and-static-thumbnails.md). PNG/static-GLTF
thumbnails and a centralized persistent project cache are implemented. The target
below includes unfinished SVG/document providers, shaped filename ellipsis,
pre-decode memory budgets and large-project performance acceptance.

## Presentation target

Use UI-KIT composition `163:37852` in file `2EZlK40cfx2Mub7GmDyI70`:
80×96 logical-pixel tiles, a 72×72 preview, one 16-pixel filename line,
4-pixel gaps, and the existing selected/hover tokens. Content imagery is dynamic;
the specimen's SVG thumbnails are not bundled replacement previews. Keep full
Unicode names available through the existing tooltip and selection path. Retain
the independent folder/content scroll regions and virtualized retained rows.

## Ownership and reuse

- Extend `kedra_preview` with the reusable thumbnail lifetime and static rendering
  mechanism. Asset Browser supplies demand; future pickers can use the same API.
- Keep project/source authorization in the host and existing
  `kedra_authoring_assets` admission. A suffix is not load authorization. Resolve
  source-qualified logical paths through the existing admitted inventory.
- Reuse Bevy asset loading, offscreen render targets and screenshot completion,
  `kedra_preview::models` and `bounds`, and `kedra_wakeup` for worker completion.
  Do not add another asset registry, executor, model parser or rendering engine.
- Extract any shared Scene View GPU-readiness/readback implementation before
  reusing it. Host camera navigation, scene selection and authorization stay in
  the editor. Thumbnail jobs never execute project scripts or activate gameplay.
- Use existing pinned `image`/SVG processing facilities for image reduction;
  evaluate their allocation limits before accepting large sources. Cache policy
  is thumbnail-specific, not a new generic caching framework.

## Demand, identity and bounds

Visible tiles have priority over a small overscan window. Hidden panels, list
mode and retired/rebound tiles relinquish demand. Duplicated consumers share one
job and one cached image. Do not eagerly load a directory's models or retain a
camera per tile. A thumbnail request identifies source scope, published revision,
full logical path/subasset, dependency revision, output size and rendering profile.
Do not hash source files on the UI thread. Invalidate on source/delivery changes,
including referenced textures; rename and project replacement cannot reuse a
result solely because its filename or document UUID matches.

Initial limits to verify before acceptance:

- One active 3D job and at most one native capture, including cancelled captures
  still awaiting completion. Keep their native allocations charged until retired.
- At most 128 pending distinct requests; rebuild priority from current demand,
  rather than retaining an unlimited stale scroll queue.
- Output sizes in a small quantized set, capped at 288×288 physical pixels for
  the compact tile at 4× density. Account resident images by allocated bytes,
  including GPU/CPU copies and images pinned by visible consumers.
- A 32 MiB thumbnail residency target with eviction of unreferenced least-recently
  used entries. If pinned demand fills the budget, defer additional work instead
  of exceeding it. Model dependencies and transient render allocations need a
  separate accounting limit; output-image size alone is not a memory bound.
- Bound admitted source bytes, decoded image dimensions, model node/geometry
  cost and preparation work. Native loading is asynchronous but cancellation of
  a handle is not proof that decoding has stopped or memory has been reclaimed.

Completed thumbnails are static images. Release temporary models, lights,
cameras and strong source handles after renderer completion. A stalled render
must not admit another job behind an unaccounted capture. Timeout publishes an
unavailable result, while native retirement remains separately accounted.
Generation tokens reject late completion after rebinding, cancellation or project
closure. Failures are cached per revision with bounded diagnostics, so scrolling
does not retry a broken asset every frame. Unsupported content retains a truthful
type fallback; it must not look like a successfully generated preview.

Persistent project storage uses `kedra_cache` and versioned content keys. Further
work includes accounting/collection of index overhead and unindexed crash remnants;
the current quota covers indexed payload, not total physical filesystem usage.

## Providers and acceptance

Support reduced raster/SVG images and passive static GLTF models first. Select a
real native GLTF scene/default scene, never guess `Scene0`. Preserve the current
explicit refusal of unsupported skins/morphs rather than silently showing broken
geometry. Document/prefab previews require their own passive projection provider;
an extension badge is not document-preview support.

Test the shared owner for deduplication, priority/capacity, retained-byte bounds,
late completion, failed preparation, cancellation and exact retirement. Negative
cases must preserve unrelated entities/assets and source bytes. Test the editor
for source/delivery invalidation, row reuse and project replacement.

Native acceptance uses actual generated image/model pixels, real scrolling and
input, repeated folder changes and multiple consumers. Verify compact tile
geometry, filename ellipsis, selection, drag and the blank-space context menu.
Measure cold/warm frame intervals and memory during scrolling through the KayKit
set; report actual observations rather than infer responsiveness from unit tests.
Verify idle camera/job counts after completion and cancellation. A successful
compile or RPC receipt alone does not establish visual or performance acceptance.
