//! Explicitly authorized Luau authoring over a host-owned transaction backend.
//! No VM, file discovery, World exposure, implicit Save or second history lives here.
use bevy::prelude::World;
use kedra_authoring::editing::SourceNode;
use kedra_authoring::workspace::HistoryId;
use kedra_script::{
    Scope,
    lua::{self, Lua, LuaSerdeExt, UserData, UserDataFields, UserDataMethods},
};
use std::sync::Arc;

/// Native edit lease. The editor owns scope/revision/settlement validation and
/// complete typed compilation. Methods must stage atomically on error; commit must
/// publish once or leave sources/history unchanged. Drop/cancel never writes sources.
pub trait AuthoringEdit: Send {
    /// Resolve bounded nodes from an exact source-qualified logical document path.
    fn nodes(&mut self, world: &mut World, path: &str) -> Result<Vec<SourceNode>, String>;
    /// Stage one native reflected field using the existing typed property machinery.
    /// JSON is a transport value, not a source schema or RON fragment.
    fn set_property(
        &mut self,
        world: &mut World,
        node: &SourceNode,
        component: &str,
        path: &str,
        value: serde_json::Value,
    ) -> Result<(), String>;
    /// Stage a domain node rename in the receiving document.
    fn rename(&mut self, world: &mut World, node: &SourceNode, name: &str) -> Result<(), String>;
    /// Stage removal of a non-root node and its descendants in the receiver.
    fn remove_node(&mut self, world: &mut World, node: &SourceNode) -> Result<(), String>;
    /// Stage reparenting within a single receiving document/composition boundary.
    fn reparent(
        &mut self,
        world: &mut World,
        node: &SourceNode,
        parent: &SourceNode,
    ) -> Result<(), String>;
    /// Stage removal of an existing authored component; full compilation still applies.
    fn remove_component(
        &mut self,
        world: &mut World,
        node: &SourceNode,
        component: &str,
    ) -> Result<(), String>;
    /// Publish one validated named transaction. Return None for a validated no-op.
    fn commit(&mut self, world: &mut World) -> Result<Option<HistoryId>, String>;
    /// Discard staged work only, without needing access to the serving World.
    fn cancel(&mut self);
}
/// Install `ctx.edit.begin(label)` for this explicitly authorized runtime scope.
/// Transactions expose nodes(path), set_property(node, component, path, value),
/// rename(node, name), commit() and cancel(). No project code is loaded or run here.
pub fn install(
    lua: &Lua,
    scope: Scope,
    ctx: &lua::Table,
    begin: impl Fn(&mut World, &str) -> Result<Box<dyn AuthoringEdit>, String> + Send + Sync + 'static,
) -> lua::Result<()> {
    let api = lua.create_table()?;
    api.set(
        "begin",
        lua.create_function(move |lua, label: String| {
            let edit =
                scope.with_world(|world| begin(world, &label).map_err(lua::Error::external))?;
            lua.create_userdata(Lease {
                scope: scope.clone(),
                edit: Some(edit),
                identity: Arc::new(()),
            })
        })?,
    )?;
    api.set_readonly(true);
    ctx.set("edit", api)
}
struct NodeHandle {
    node: SourceNode,
    identity: Arc<()>,
}
impl UserData for NodeHandle {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("name", |_, this| Ok(this.node.name.clone()));
    }
}
struct Lease {
    scope: Scope,
    edit: Option<Box<dyn AuthoringEdit>>,
    identity: Arc<()>,
}
impl Lease {
    fn node(&self, node: &lua::AnyUserData) -> lua::Result<SourceNode> {
        let node = node.borrow::<NodeHandle>()?;
        if !Arc::ptr_eq(&node.identity, &self.identity) {
            return Err(lua::Error::external(
                "source node belongs to another edit transaction",
            ));
        }
        Ok(node.node.clone())
    }
    fn with<T>(
        &mut self,
        action: impl FnOnce(&mut dyn AuthoringEdit, &mut World) -> Result<T, String>,
    ) -> lua::Result<T> {
        let scope = self.scope.clone();
        let edit = self
            .edit
            .as_mut()
            .ok_or_else(|| lua::Error::external("edit transaction is closed"))?;
        scope.with_world(|world| action(edit.as_mut(), world).map_err(lua::Error::external))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(edit) = &mut self.edit {
            edit.cancel();
        }
    }
}
impl UserData for Lease {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("nodes", |lua, this, path: String| {
            let nodes = this.with(|edit, world| edit.nodes(world, &path))?;
            if nodes.len() > 4096 {
                return Err(lua::Error::external(
                    "edit node enumeration exceeds 4096 nodes",
                ));
            }
            let result = lua.create_table()?;
            for (index, node) in nodes.into_iter().enumerate() {
                result.set(
                    index + 1,
                    lua.create_userdata(NodeHandle {
                        node,
                        identity: this.identity.clone(),
                    })?,
                )?;
            }
            Ok(result)
        });
        methods.add_method_mut("set_property", |lua, this, (node, component, path, value): (lua::AnyUserData, String, String, lua::Value)| {
            let node = this.node(&node)?;
            let value: serde_json::Value = lua.from_value(value)?;
            this.with(|edit, world| edit.set_property(world, &node, &component, &path, value))
        });
        methods.add_method_mut(
            "rename",
            |_, this, (node, name): (lua::AnyUserData, String)| {
                let node = this.node(&node)?;
                this.with(|edit, world| edit.rename(world, &node, &name))
            },
        );
        methods.add_method_mut("commit", |_, this, ()| {
            let result = this.with(|edit, world| edit.commit(world))?;
            this.edit = None;
            // IDs are opaque strings at the Lua boundary, never lossy doubles.
            Ok(result.map(|id| id.0.to_string()))
        });
        methods.add_method_mut("remove_node", |_, this, node: lua::AnyUserData| {
            let node = this.node(&node)?;
            this.with(|edit, world| edit.remove_node(world, &node))
        });
        methods.add_method_mut(
            "reparent",
            |_, this, (node, parent): (lua::AnyUserData, lua::AnyUserData)| {
                let node = this.node(&node)?;
                let parent = this.node(&parent)?;
                this.with(|edit, world| edit.reparent(world, &node, &parent))
            },
        );
        methods.add_method_mut(
            "remove_component",
            |_, this, (node, component): (lua::AnyUserData, String)| {
                let node = this.node(&node)?;
                this.with(|edit, world| edit.remove_component(world, &node, &component))
            },
        );
        methods.add_method_mut("cancel", |_, this, ()| {
            this.with(|edit, _| {
                edit.cancel();
                Ok(())
            })?;
            this.edit = None;
            Ok(())
        });
    }
}
