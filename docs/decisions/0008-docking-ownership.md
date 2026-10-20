# ADR 0008: Docking ownership and transactions

Status: architecture in use. Native platform acceptance is bounded separately.

## Context and constraints

Tools and document views may occupy any group. There is no reserved central region
or egui dependency. Figma defines presentation, not the model's algorithm or source
ownership. Feature content must survive tab/group moves and native host transfer.

## Decision

`kedra_docking` owns a bounded conventional binary split/tab model. Authoritative
state, commands, candidates and sessions are ordinary Rust values. The runtime
crate has no Bevy, editor, authoring-writer or desktop-integration dependency.
No second ECS, UI language, scheduler or general layout framework is introduced.

Stable IDs identify hosts, nodes and view instances, not entities or native handles.
Feature-owned documents and undo histories remain outside docking; several views
can share one document. Snapshots are layout preferences, not authoring Save.

Commands prepare validated candidates against a specific revision. Commit rejects
stale or foreign candidates. Rejection/cancellation leaves topology and revision
unchanged. Bevy UI/Taffy owns geometry; candidate and committed projection share
resolved CSS metrics, measured allocations and rounding. The model has no pixel
layout authority.

`kedra_ui::docking` retains content roots keyed by view identity and reparents them
without reconstruction. Bevy owns focus, picking and navigation; Flair owns visual
properties and the declared inline-geometry path. The editor owns gesture policy,
feature bindings, menus and preferences.

Desktop integration stages a native window/camera/root, acknowledges readiness,
then commits a still-current transfer and moves content before empty-host
retirement. OS allocation is asynchronous, not an atomic model transaction.
Failure/staleness retires staged resources, not source content.

Main exit, floating Close, tool hiding and document-close approval are distinct.
Hidden tools retain reopening bookmarks. Future document features must supply
close approval; docking never saves a document or approves discarding edits.
Restoration validates format, bounds, identity, reachability and ownership before
publication. Unknown kinds retain unavailable records. Invalid input is reported
and preserved, not silently reset.

## Reuse rationale

Reuse requires a renderer-independent core that preserves content and tab identity
through membership/order changes and edge drops, and supports native multi-window
lifecycle. Rebuilding content, replacing tab identity or coupling the model to a
single presentation host does not satisfy those contracts. Adapting such an
implementation requires replacing its retention/native contracts, not merely
changing CSS.

A bounded project-owned model therefore composes the existing Bevy UI/Taffy,
scene/Flair, UUID/serde and native-host mechanisms. It does not reuse an external
renderer under a second authoritative tree. A maintained renderer-independent
upstream core can be reconsidered if it satisfies these contracts. No prototype
source import or license permission follows from architectural similarity.

## Verification and platform limits

Tests require retained tab/group transfers, recursive minima, resize cancellation,
maximize/restore, overflow/navigation, acknowledged float/redock/close, validated
preferences and an independent native production-mechanics fixture with editable
content. Model tests alone are insufficient.

Current Winit Wayland support lacks portable global desktop coordinates. Without
global positions or destination-local events, cross-host dragging must report its
limitation instead of silently changing the operation. Injected pointer tests do
not establish physical OS capture, mixed-DPI behavior or platform certification.
See [docking](../ui/docking-implementation.md) and [tests](../ui/docking-tests.md).
