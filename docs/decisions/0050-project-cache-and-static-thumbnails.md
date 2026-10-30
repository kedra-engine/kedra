# ADR 0050: Shared project cache and static thumbnails

Status: implementation; owner review pending.

## Decision

Core-layer `kedra_cache` owns disposable project storage independently of Bevy,
editor UI and source writers. Its independent responsibility and real thumbnail
consumer justify a separate crate. Project cache providers share namespaced,
versioned keys and quotas under `.kedra/cache`. The maintained Apache-2.0
`cacache` library supplies storage/index/integrity mechanisms. See the
[cache contract](../../crates/kedra_cache/README.md) for dependency configuration
and physical-storage limitations.

`kedra_preview::thumbnails` owns visible-demand coalescing, bounded snapshot
residency, serial native loading/rendering, background image/cache I/O and
retirement. It reuses audited model materialization and finite bounds. Shared
native pipeline readiness lives in `kedra_preview::gpu`, consumed by thumbnails
and Scene View. Bevy owns loading, remapping and readback; previews run no scripts.

The editor supplies source/delivery authorization, a reserved render layer and
visible tile requests. The existing document admission task supplies the source
catalog without opening a tab. Keys include project identity, source/dependency
hashes, delivery member hashes, provider version/kind, logical path and output
size. Source/delivery mismatch withdraws demand; cache hits cannot bypass admission.
Unchanged UI image/geometry state is retained rather than rewritten every frame.

The compact grid follows UI-KIT `163:37852`: 80×96 tiles, 72×72 previews and 4-pixel
gaps. PNG and passive GLTF previews show real content; other types retain truthful
type fallbacks. Full filenames remain in existing hints; shaped single-line
ellipsis is not implemented. Native GLTF scene handles come from the loader,
never guessed subasset labels. Skins/morphs retain the static-preview refusal.

## Bounds and limitations

Demand and residency share a 128-request bound so pinned visible snapshots cannot
prevent completion of other admitted visible requests. Output is at most
288×288 RGBA8, at most 81 MiB for resident CPU/GPU copies. One native job and one
disk/codec task bound concurrency. Only current demand receives completion;
cancelled native work retains its slot until completion. A stalled readback
cannot admit an allocation loop. Active frame demand ends after 30 seconds while
the retained job can still be polled. Finished jobs retire temporary scene entities
and strong native source handles.

The host admits sources up to 16 MiB. Decoded images exceeding 4096 pixels per
axis are refused before reduction; model materialization retains its 4096-node
bound. Native dependency decoding precedes these checks: this is **not** a hard
source/dependency decoder-memory bound. Cached PNG decoding has separate size and
allocation limits. Pre-decode transient-memory budgets and SVG/document providers
remain future work.

Persistent hits skip model loading/rendering. Per-revision failures use bounded
in-memory retention; disk failures are logged once per changed diagnostic, with
uncached fallback. Thumbnails do not rebuild or hot-swap runtime deliveries.

## Verification ownership

The cache crate owns filesystem scope/quota/missing-content guarantees; preview
tests own pinned capacity, revision separation and late-result/forest retirement.
Native `document-creation` checks generated PNG/model tiles, frames, disk reuse
across project close/reopen, source creation and model placement through Hierarchy
and Scene View. Large-project frame/memory measurement is a separate acceptance
boundary, not implied by compilation or these functional checks.
