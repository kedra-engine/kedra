# Startup design geometry

The design source is UI-KIT `2EZlK40cfx2Mub7GmDyI70`, family `109:9133` on
page `175:45356`. These are design/provenance identities, not screenshot cases.
Startup has no active image references or pixel-comparison suite. Screenshot
coverage is restricted to [kit buttons and text inputs](ui-corpus.md).

| Native surface | State | Figma node |
| --- | --- | --- |
| 1120×720 | Empty project manager | `109:8563` |
| 1120×720 | Recent projects | `109:8489` |
| Manager with 640×400 modal | Create project | `109:8607` |
| Manager with 640×400 modal | Open project | `109:8873` |
| 640×360 | Project failure | `109:8445` |
| Manager with modal | Invalid create | `109:8735` |
| Manager with modal | Invalid open | `109:9003` |

Figma body coordinates start at y=33; the owner-approved borderless client starts
at y=32 and uses the full width. Independent geometry expectations retain that
explicit border removal. Create/Open modals occupy (240,160)–(880,560) in the
1120×720 client and are modal states, not separate native surfaces.

No initial splash is created. Opening is a lightweight opaque adaptation shown
only for unfinished work after 150 ms; success never waits for it. Heavy release
artwork is omitted. Failed uses an opaque canvas and recovery actions. Projects
and modal/empty states share the compact brand row and common editor palette.

`startup-reference.json` retains the manually transcribed geometry contract.
The native behavior scenario checks real pointer/keyboard input, Unicode draft
retention, modal traversal, validation failures, source lifecycle and window
retirement. Renderer readbacks are transient synchronization fences, not retained
screenshots, image references or visual acceptance.

Preparation creates sources; it does not build a Rust host, starter scene or
executable project code. Physical picker/chrome/IME behavior and other platforms
remain separate acceptance work. See [startup](../startup.md),
[automation](../automation.md) and the [Figma source map](figma.md).
