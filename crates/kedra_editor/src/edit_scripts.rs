//! Explicit project editor-script admission/execution; Project Open alone never runs code.
mod launch;
mod source;
use kedra_script::{
    Scope,
    lua::{self, Lua},
};
pub(crate) use launch::configure;
pub use launch::{ExecutionStatus, Launch, cancel, status};
pub use source::{ScriptInfo, discover, load};

/// Grant an explicitly authorized run access to exact admitted source-qualified
/// document paths. Call from its native Program bindings, never from script args.
/// The first successful begin pins this capability to its source session; later
/// begins cannot reauthorize it after session retirement. Every operation refuses pending drafts,
/// retired projects, pending Save and changed source revisions. This installs only
/// `ctx.edit`, not code loading, execution, filesystem Save or mutable World access.
pub fn install(lua: &Lua, scope: Scope, ctx: &lua::Table, grants: Vec<String>) -> lua::Result<()> {
    crate::workspace::edit_scripts::install(lua, scope, ctx, grants)
}
