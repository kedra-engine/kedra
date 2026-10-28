//! Shared editor context over existing source authority. Selection and drag payloads
//! contain persistent logical references, not UI entities, source writes or Open effects.
mod library;
mod selection;
pub use library::{AssetKind, AssetLibrary, AssetRef, NodeRef, ScopeId};
pub use selection::{DragPayload, Selection, SelectionItem};
