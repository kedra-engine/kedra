# 0053: Scene transform manipulators

Status: accepted for single-node Move, Rotate and Scale. Group manipulation remains
future work; native acceptance must be reported independently of compilation.

## Decision and alternatives

Use Bevy math, camera projection and retained gizmos with a window-aware editor adapter.
`kedra_gizmo` is a runtime-layer library owning constrained transform candidates. It has
one production consumer, the editor; it does not own input, selection, source writes,
history, rendering targets or a second ECS. This independent mathematical responsibility
does not belong in the workspace host. The existing Bevy dependency supplies geometry;
no additional external dependency or Bevy fork is required.

The resolved Bevy 0.20.0 `bevy_gizmos::transform_gizmo` was inspected directly. Its stock
interaction systems read `PrimaryWindow` and global mouse buttons, mutate local Transform
from world-space deltas and have one global gesture state. Stock rendering reserves layer
15 and one overlay camera. Those ownership assumptions do not match texture-backed,
multi-window Scene Views. Public constraint helpers remain reusable without installing
the stock interaction plugin or substituting global input resources.

The MIT/Apache-2.0 `urholaukkarinen/transform-gizmo` project offers an independent core
and a Bevy adapter. The inspected 0.11.0 workspace manifest targets Bevy 0.19; its core
also adds emath/epaint/ecolor and requires a rendering adapter. It is a candidate for
future richer handles, not a justified new dependency for axis translation. Compatibility,
maintenance and security review would be required before admitting a pinned version.
Reference: <https://github.com/urholaukkarinen/transform-gizmo>.

## Ownership and behavior

The editor acquires a gesture against an exact document scope/revision, selected source
address, native window, projection entity and dock layout revision. It freezes the local
transform and invertible parent matrix. World motion converts through the parent's inverse;
translation preserves local rotation and scale, including reflected/nonuniform parents.
Invalid constraints never publish a candidate. Snap distances are relative to the start.

Preview changes only the owned projection. Release stages one ordinary property change
through existing authoring validation and publishes one history entry. Escape, focus loss,
selection/scope/revision changes, projection retirement and layout changes cancel preview.
Save must not persist a live preview. No-op drags create no history. Prefab instance addresses
retain their composition scopes and use the existing override policy.

Move uses world axes. Rotate and Scale use local axes: rotation rings are transformed
by the frozen parent and local rotation, before local scale. Nonuniform parents produce
elliptical rings, evaluated by transforming the ray back into that same frame. Rotation
preserves translation and signed scale; no affine decomposition or shear approximation
is performed. Edge-on rings are not pickable. Scale changes one signed local component,
or multiplies all components for the central uniform handle. Uniform scaling preserves
existing ratios and signs; zero baselines and zero/sign-crossing factors are rejected,
retaining the last valid preview. The user can cancel instead of accepting that preview.

W/E/R select modes outside text entry and camera flight. Icons occupy one compact row
inside the existing viewport control panel, together with Frame selection and Settings.
The hand button cancels an active preview and disables the transform tool while retaining
selection and camera navigation. Settings owns grid visibility. Snap is separate
from grid visibility: one world unit, 15 degrees, or a 0.1 scale-factor increment, relative
to press. Configurable steps, world/local switching, physical-input acceptance,
multi-selection and surface snapping remain separate work.
