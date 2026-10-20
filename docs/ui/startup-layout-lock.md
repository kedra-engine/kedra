# Startup visual references

The startup reference is UI-KIT `2EZlK40cfx2Mub7GmDyI70`, component family
`109:9133` on page `175:45356`. The [Figma source map](figma.md) describes current
library navigation; frame identities here record the pinned export source.
Seven independent current client-frame PNGs are stored in
`references/startup-2026-10-20/`. They include the 32px custom titlebar and have
no external shadow, crop, rescaling or repainting. JSON policies pin source links,
SHA-256, dimensions and critical regions.

| Native surface | State | Figma node | Comparison policy |
| --- | --- | --- | --- |
| 1120×720 | Empty project manager | `109:8563` | [empty manager](references/manager-empty-visual.json) |
| 1120×720 | Recent projects | `109:8489` | [manager](references/manager-visual.json) |
| Manager with 640×400 modal | Create project | `109:8607` | [create](references/create-visual.json) |
| Manager with 640×400 modal | Open project | `109:8873` | [open](references/open-visual.json) |
| 640×360 | Project failure | `109:8445` | [failure](references/failure-visual.json) |
| Manager with modal | Invalid create | `109:8735` | [invalid create](references/create-invalid-visual.json) |
| Manager with modal | Invalid open | `109:9003` | [invalid open](references/open-invalid-visual.json) |

Figma manager body coordinates start at y=33; the owner-approved borderless
implementation starts at y=32 and uses the full client width. Its independent
geometry expectations apply that explicit border removal to the design's layout.
Create and Open modals occupy
(240,160)–(880,560) in the 1120×720 client. Create/Open are modal states of the
manager window, not separate native surfaces. Continue without project is not a
supported action.

The saved Boot export/policy is design provenance only: no initial splash surface
is created. Opening is a lightweight opaque adaptation, shown only for unfinished
work after 150 ms; success never waits for it. Heavy release artwork is omitted.
Failed uses an opaque canvas and recovery actions. Projects and modal/empty states
share a compact brand row above the
heading/actions. The common editor palette uses purple accent, selection and focus
roles; neutral surfaces remain unchanged. Earlier exports are retained separately,
not used as the active policy's expected pixels.

The previous [Boot](references/boot-visual.json) and
[Loading](references/loading-locked-visual.json) policies retain their original
bounded exports as historical provenance, not acceptance targets for windowless
startup or the new lightweight loading adaptation. Current Loading geometry is
pinned separately in `startup-reference.json`; native presentation is reviewed
explicitly rather than manufacturing a screenshot baseline from the application.

## Acceptance boundary

Owner direction intentionally omits the initial splash and loading illustration.
The client frame also removes Figma's perimeter stroke and its 1px layout inset,
at the owner's request. Strict image
comparisons expose this difference; no exclusion or tolerance change hides it.

Policies use normalized YIQ tolerance 0.05 and independent 2% full-frame and
critical-region budgets. There are no masks. They are design-reference audits,
not certified cross-platform application goldens.

Absolute project paths, selected/missing recent-row presentation, text focus and
truthful preparation copy can differ from static references. Opening prepares
sources; it does not build a Rust host or scaffold a starter scene. Such
differences remain visible in reports, not hidden by alignment or new baselines.

Native startup geometry/input and renderer captures have macOS evidence.
Deterministic startup fixture content, complete control-state pixel equivalence,
physical picker/chrome/IME behavior and other platforms remain acceptance work.
See [visual comparison](visual-acceptance.md), [startup](../startup.md) and
[automation](../automation.md).
