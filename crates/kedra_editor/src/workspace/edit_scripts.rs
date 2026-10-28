//! Host-owned settlement and lifetime authorization over real Documents leases.
use super::*;
use kedra_authoring::workspace::HistoryId;
use kedra_authoring::{
    editing::{SourceEdit, SourceNode, SourceProperty},
    persistence::Workspace,
};
use kedra_authoring_luau::AuthoringEdit;
use kedra_script::{
    Scope,
    lua::{self, Lua},
};

pub(crate) fn install(
    lua: &Lua,
    scope: Scope,
    ctx: &lua::Table,
    grants: Vec<String>,
) -> lua::Result<()> {
    // Bind this installed capability to its first admitted source session, not
    // merely each transaction. Retiring a project must not regrant matching paths.
    let source_scope = std::sync::Mutex::new(None);
    kedra_authoring_luau::install(lua, scope, ctx, move |world, label| {
        with_workspace(world, |world, state| {
            settled(world, state)?;
            let mut admitted = source_scope
                .lock()
                .map_err(|_| "Script source grant unavailable")?;
            let current = state.documents.edit_scope();
            if admitted.is_some_and(|scope| scope != current) {
                return Err(
                    "Script source grant retired; explicit reauthorization required".into(),
                );
            }
            let edit = state.documents.begin_edit(label, &grants)?;
            admitted.get_or_insert(current);
            Ok(Box::new(Backend(edit)) as Box<dyn AuthoringEdit>)
        })
    })
}
fn with_workspace<T>(
    world: &mut World,
    action: impl FnOnce(&mut World, &mut EditorWorkspace) -> Result<T, String>,
) -> Result<T, String> {
    if !world.contains_resource::<EditorWorkspace>() {
        return Err("Editor workspace unavailable".into());
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| action(world, &mut state))
}
fn settled(world: &World, state: &EditorWorkspace) -> Result<(), String> {
    if super::settlement::source_edits(world, state) {
        return Err(
            "Resolve pending editor gestures and property drafts before script source edits".into(),
        );
    }
    Ok(())
}
struct Backend(SourceEdit);
impl Backend {
    fn access<T>(
        &mut self,
        world: &mut World,
        action: impl FnOnce(
            &mut SourceEdit,
            &mut Workspace,
            &bevy::reflect::TypeRegistry,
            &kedra_scene::reflection::ComponentPolicies,
        ) -> Result<T, String>,
    ) -> Result<T, String> {
        with_workspace(world, |world, state| {
            settled(world, state)?;
            let registry = world.resource::<AppTypeRegistry>().clone();
            let types = registry.read();
            let policies = world.resource::<kedra_scene::reflection::ComponentPolicies>();
            action(
                &mut self.0,
                state.documents.edit_workspace()?,
                &types,
                policies,
            )
        })
    }
}
impl AuthoringEdit for Backend {
    fn nodes(&mut self, world: &mut World, path: &str) -> Result<Vec<SourceNode>, String> {
        self.access(world, |edit, documents, _, _| edit.nodes(documents, path))
    }
    fn set_property(
        &mut self,
        world: &mut World,
        node: &SourceNode,
        component: &str,
        path: &str,
        value: serde_json::Value,
    ) -> Result<(), String> {
        self.access(world, |edit, documents, types, policies| {
            edit.set_property(
                documents,
                SourceProperty {
                    node,
                    component,
                    path,
                    value,
                },
                types,
                policies,
            )
        })
    }
    fn rename(&mut self, world: &mut World, node: &SourceNode, name: &str) -> Result<(), String> {
        self.access(world, |edit, documents, types, policies| {
            edit.rename(documents, node, name, types, policies)
        })
    }
    fn commit(&mut self, world: &mut World) -> Result<Option<HistoryId>, String> {
        with_workspace(world, |world, state| {
            settled(world, state)?;
            let capture = super::history_context::capture(world, state);
            let registry = world.resource::<AppTypeRegistry>().clone();
            let types = registry.read();
            let policies = world.resource::<kedra_scene::reflection::ComponentPolicies>();
            let result = state.documents.commit_edit(&mut self.0, &types, policies)?;
            super::history_context::accepted(world, state, capture);
            Ok(result)
        })
    }
    fn remove_node(&mut self, world: &mut World, node: &SourceNode) -> Result<(), String> {
        self.access(world, |edit, documents, types, policies| {
            edit.remove_node(documents, node, types, policies)
        })
    }
    fn reparent(
        &mut self,
        world: &mut World,
        node: &SourceNode,
        parent: &SourceNode,
    ) -> Result<(), String> {
        self.access(world, |edit, documents, types, policies| {
            edit.reparent(documents, node, parent, types, policies)
        })
    }
    fn remove_component(
        &mut self,
        world: &mut World,
        node: &SourceNode,
        component: &str,
    ) -> Result<(), String> {
        self.access(world, |edit, documents, types, policies| {
            edit.remove_component(documents, node, component, types, policies)
        })
    }
    fn cancel(&mut self) {
        self.0.cancel();
    }
}
