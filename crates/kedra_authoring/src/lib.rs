//! Authoring sources, working revisions and save transactions; never live gameplay state.
//!
//! Runtime scene loading does not depend on this crate. A host executes explicit
//! save requests and returns their receipts; history cannot acknowledge unrelated
//! or newer edits. No filesystem writer or remote transport is implicitly installed.
pub mod overrides;
pub mod persistence;
pub mod unpack;
pub mod workspace;

fn failure(message: impl Into<String>) -> kedra_scene::Error {
    kedra_scene::Error(message.into())
}
