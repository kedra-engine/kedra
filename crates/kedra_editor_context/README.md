# kedra_editor_context

Editor-layer logical context shared by native windows and extensions. The real
consumer is `kedra_editor::{context,drag}`; no runtime package depends on this crate.

`AssetLibrary` borrows an admitted `SourceSnapshot` and its `WorkingSet`. It does
not scan files, build a second registry, load runtime assets or open document tabs.
Exact source-qualified paths preserve case and Unicode. Document references retain
their document identity; tracked files retain their asset UUID. Folders are inferred
from admitted members, so empty filesystem folders are not library members.

`ScopeId` belongs to one host project/source lifetime. Asset/node references from
retired scopes must fail validation even when paths and document IDs are reused.
Nodes retain the receiving document and complete composed `NodeAddress`.

`Selection` holds up to 4096 deduplicated items in user order, with the final item
active. Replace/toggle validate all members and refuse atomically. Reconciliation
removes missing/foreign members without authoring effects. Selection revisions
describe context changes, not source history. Windows can keep a pinned snapshot.

`DragPayload` captures all members of a homogeneous asset or node selection; empty
and mixed selections refuse. Receivers revalidate membership and scope at use time.
Payloads grant no source writes, document activation, Save or filesystem permission.
The editor separately owns visible producer bindings, cancellation and draft settlement.

Inspector currently displays the active item rather than common-field multi-editing.
Hierarchy supports modifier/range selection; Scene View marks one selected node.
Raw non-document asset inspection currently exposes only identity/path presentation.
Native multi-selection and context-drop interaction acceptance is not yet established.

Run `cargo test -p kedra_editor_context --test contracts --locked` for pure identity,
selection/refusal, reconciliation and drag-lifetime contracts. Editor integration and
real input/frame acceptance remain separate checks.
