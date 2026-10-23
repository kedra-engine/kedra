# ADR 0022: Compact lossless image payloads

Status: accepted implementation choice for owner-requested asset optimization.

## Decision

Version 2 of `ImageVariantsDelivery` stores encoded PNG bytes as Base64 strings,
using the existing maintained `serde_with` crate's Base64/PickFirst adapters.
The loader accepts both legacy version-1 numeric arrays and version-2 payloads.
UUIDs, native Bevy labels, PNG pixels, dimensions and density ordering do not
change. New deliveries must be rebuilt; older binaries do not read version 2.

Numeric RON byte lists inflate each encoded byte into decimal text and require
per-number parsing. Base64 has bounded 4/3 expansion and a maintained decoder.
Reusing `serde_with` adds its small MIT/Apache-2.0 `base64` dependency rather than
introducing a handwritten parser, custom binary container or serializer. Both
libraries are already established Rust ecosystem tools; the dependency gate
continues to check their resolved versions and license/security constraints.

## GPU and scope boundary

Disk encoding is not GPU texture compression. PNG remains lossless and Bevy
still decodes a loaded package's authored variants into native images. Unused
variants are not yet streamed independently; Base64 does not claim to reduce
their decoded RGBA memory. Startup does not depend on large decorative artwork
or a splash renderer fence.

Bevy KTX2/Basis support is an alternative for texture-heavy content, but requires
an evaluated offline encoder, alpha/color quality and target/GPU-format support.
It is not enabled solely to compress small UI logos/icons or replace a proven
serialization bottleneck. No new GPU codec or format-support claim is made.

## Verification

Test exact compact roundtrip, a payload-size bound against numeric expansion,
legacy numeric packages, malformed payload rejection, and the existing native
import/load density and integrity failure corpus. Keep measured startup/asset
sizes in local evidence; do not weaken visual tolerances or recolor artwork.
