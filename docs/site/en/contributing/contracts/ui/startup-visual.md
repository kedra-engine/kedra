# Startup geometry contracts

Startup design provenance uses UI-KIT `2EZlK40cfx2Mub7GmDyI70`, family `109:9133`, page `175:45356`
as an independent design reference, not a functional-test geometry baseline.
There are no startup image references/pixel-comparison cases; only [kit controls](ui-corpus.md)
have screenshot coverage.

## Source states and adaptation

| Native surface | State | Figma node |
| --- | --- | --- |
| 1120×720 | Empty project manager | `109:8563` |
| 1120×720 | Recent projects | `109:8489` |
| Manager with 640×400 modal | Create project | `109:8607` |
| Manager with 640×400 modal | Open project | `109:8873` |
| 640×360 | Project failure | `109:8445` |
| Manager with modal | Invalid create | `109:8735` |
| Manager with modal | Invalid open | `109:9003` |

Figma body starts at y=33; owner-approved borderless client starts at y=32, full width.
Independent expectations preserve this explicit border removal. Create/Open occupy
(240,160)–(880,560) within the 1120×720 client; they are modal states, not extra native surfaces.

[ADR 0021](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0021-windowless-startup.md)
accepts windowless startup: no initial splash. Opening is a lightweight opaque adaptation shown
only for unfinished work after 150 ms; success never waits. Heavy release artwork is omitted.
Failed has opaque canvas/recovery actions. Projects/modal/empty states share compact branding
and palette. Preparation creates sources, not a Rust host, starter scene or executable project code.

## Independent geometry and evidence

[`docs/ui/startup-reference.json`](https://github.com/kedra-engine/kedra/blob/main/docs/ui/startup-reference.json)
is a manually transcribed design reference. Its rectangles use logical endpoint coordinates,
independent of renderer DPR, with viewport `[1120, 720]` and tolerance `1.0`. It preserves boot,
manager, create, open and loading node provenance and declared manager/create/loading regions.
The `pixels` entries are retained point/color declarations, not PNG hashes or an active pixel
comparison suite. The presence of boot/loading design identities does not authorize a splash or
heavy artwork. Native startup scenarios check user workflows at DPR 1/2, not exact reference
coordinates, padding or colors. The geometry and `pixels` declarations are design provenance,
not active functional assertions.

The native startup scenario checks pointer/keyboard input, Unicode draft retention, modal traversal,
validation failures, source lifecycle and window retirement. Renderer readbacks are transient GPU
fences; the scenario retains no screenshots and compares no reference pixels. Physical
picker/chrome/IME behavior and other platforms need separate tests. Native verification of this
revision remains pending.
See [Create and open](../../../manual/projects/create-open.md),
[Native scenarios](../../../manual/testing/native-scenarios.md),
[Capture boundaries](../../../manual/automation/input-and-captures.md) and
[Design provenance](design-provenance.md#startup-identity-and-palette).
