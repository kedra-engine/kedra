# Startup visual references

The startup reference is UI-KIT `2EZlK40cfx2Mub7GmDyI70`, page `69:2425`.
The [Figma source map](figma.md) describes current library navigation; the page
and frame identities here record the pinned export source.
Nine independent complete client-frame PNGs are stored in
`references/startup-2026-10-18-v2/`. They include the 32px custom titlebar and have
no external shadow, crop, rescaling or repainting. JSON policies pin source links,
SHA-256, dimensions and critical regions.

| Native surface | State | Figma node | Comparison policy |
| --- | --- | --- | --- |
| 640×360 | Engine splash | `71:4556` | [boot](references/boot-visual.json) |
| 640×360 | Project loading | `71:4575` | [loading](references/loading-locked-visual.json) |
| 1120×720 | Empty project manager | `71:4775` | [empty manager](references/manager-empty-visual.json) |
| 1120×720 | Recent projects | `71:4622` | [manager](references/manager-visual.json) |
| Manager with 640×400 modal | Create project | `71:4819` | [create](references/create-visual.json) |
| Manager with 640×400 modal | Open project | `71:5038` | [open](references/open-visual.json) |
| 640×360 | Project failure | `71:4596` | [failure](references/failure-visual.json) |
| Manager with modal | Invalid create | `71:4930` | [invalid create](references/create-invalid-visual.json) |
| Manager with modal | Invalid open | `71:5150` | [invalid open](references/open-invalid-visual.json) |

Manager/modal body coordinates start at y=33. Create and Open modals occupy
(240,160)–(880,560) in the 1120×720 client. Create/Open are modal states of the
manager window, not separate native surfaces. Continue without project is not a
supported action.

## Acceptance boundary

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
