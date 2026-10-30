# Project cache

`kedra_cache` is the shared, Bevy-independent owner of disposable project-derived
bytes. Consumers use `Store::project` and a versioned, namespaced `Key`, rather
than private cache directories/indexes. Storage is `.kedra/cache/v1` below the
project root. Its generated `.gitignore` excludes it in external projects.
Deleting this directory with the editor closed loses only derived data; sources,
Save recovery and asset deliveries are not stored here.

Operations are blocking and belong on an existing worker pool. Apache-2.0
`cacache` supplies verified content addressing, index encoding and atomic content
publication. Kedra owns placement, nonblocking interprocess locking, entry-byte
admission and oldest-written eviction across namespaces. No async runtime, mmap
or source-symlink feature is enabled. Version 13.0.0 is pinned for its supported
synchronous-only configuration; 13.1.0 has an unguarded async `State` reference in
that configuration. This avoids a custom disk format, parser or executor.

Defaults are 4 MiB per value, 1 GiB indexed payload and 4096 entries. These are
payload quotas, not a hard filesystem quota: index records, temporary files and
unindexed crash remnants are additional. There is no background garbage collector.
Missing/corrupt content is a miss; storage errors remain explicit diagnostics and
consumers must support uncached fallback. Cache availability cannot authorize
source mutation or become a prerequisite for opening a project.

Keys include provider namespace/version, content/dependency revisions and output
options. Full source-qualified logical paths remain the external lookup authority;
a cache key is not an asset identity. The first consumer is
`kedra_preview::thumbnails`. Runtime delivery and build outputs retain their
existing contracts, rather than becoming disposable cache data implicitly.

Public filesystem contracts in `tests/contracts.rs` cover persistence, scope and
namespace separation, quotas, missing-content regeneration, rejected configuration
and preservation of source/foreign files.
