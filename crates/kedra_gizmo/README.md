# kedra_gizmo

Runtime-layer constrained transform candidates using Bevy's public geometry helpers.
`MoveDrag` freezes the local baseline, parent inverse and world-space axis on press.
Each pointer ray returns an independent candidate; no world, input, document or file
is mutated. Dropping the gesture cancels it. Translation preserves local rotation and
scale under rotated, nonuniform and reflected parents. Invalid parents, constraints
and snapping steps fail explicitly.

`RotateDrag` evaluates local-axis ring angles in the frozen parent/orientation frame,
preserving translation and signed scale without decomposing a potentially sheared world
matrix. `ScaleDrag` changes one local scale or scales uniformly while preserving signs
and ratios; zero/sign-crossing factors fail without mutating the gesture. Both reuse the
same host preview/history lifecycle. Their snapping units are radians and relative factors.

The editor is the production consumer and owns rendered handles, window/viewport
eligibility, source revision guards, preview restoration and one-shot history publication.
The stock Bevy global-input transform plugin is not installed by this library.
See [ADR 0053](../../docs/decisions/0053-scene-transform-manipulators.md).

`cargo test -p kedra_gizmo --locked` checks coordinate conversion and failed-constraint
isolation without editor feature unification. `scene-transform` exercises assembled native
input and source Save separately; neither establishes physical-input quality.
