# ADR 0048: Native source import and file-drop ownership

## Status

Accepted source-import boundary. Wayland backend integration is pending a reviewed,
published fork revision and physical-input acceptance; the committed Winit pin remains
`948d27f0007602d2536a8974fd483c6fd33d19d3`.

## Decision

Assets Import uses the existing rfd native dialog dependency. The editor owns the
invoking window, retained browser identity, project/directory capture, pending work,
diagnostics and source settlement. Native Bevy file-drop messages use the same operation.
Only a visible Assets panel in the event's window at the native drag position is eligible.
No drop position may be inferred from a pointer location predating the OS drag.

`kedra_import::ingest` owns external-file copying and fresh metadata initialization.
WalkDir and tempfile provide traversal, staging and no-clobber publication; existing
import metadata initialization remains the only constructor. This is a cohesive import
responsibility, not a second editor filesystem writer. Adding the editor's production
dependency on the existing import crate is allowed by the checked dependency DAG.

Source ingestion copies rather than moves. Destination conflicts refuse without replacing
existing files or identities. Preparation is complete before publication; publication is
per-file and partial I/O failure is reported, not misrepresented as a rollback. Incoming
sidecars are excluded. Source reconciliation preserves authoring history and refuses
unrelated external document changes. Import is not document Save and has no document Undo.

## Native backend requirement

Winit 0.30.13's Wayland backend does not implement file-drop reception; its X11 backend
does not emit drag coordinates. The compatible fork implementation uses the already
selected SCTK 0.19 data-device protocol and calloop 0.13 event loop. It must preserve the
existing Winit/Bevy path-event API, advertise Copy only, use bounded asynchronous transfer,
retain the drop position while data arrives, and retire cancelled/timed-out transfers.
The maintained `url` crate supplies local file-URI decoding; no handwritten URI decoder
or alternate window/event loop is warranted. This dependency is already in Kedra's lock.
No additional unsafe workspace code is authorized.

Winit's existing Apache-2.0 notices and macOS titlebar changes remain applicable.
Upstream Winit's newer data-transfer API is not a compatible replacement for the Bevy
0.20 adapter. A future upstream migration must replace this compatibility implementation
and verify both the editor and independent native consumers.

## Limits and verification

Imported files and sidecars are authoring sources, not a verified runtime delivery.
Automatic build/publication and live delivery replacement are separate lifecycle work.
This import boundary does not enable PNG/GLB preview without a matching delivery.

Real filesystem tests cover conflicts, unchanged originals and metadata, Unicode and
linked-file refusal. Editor tests cover routing and source-history preservation. Fork
tests cover local URI decoding and invalid inputs. Native dialogs, compositor transfer,
clipboard coexistence and cross-window/DPI behavior require real desktop verification.
