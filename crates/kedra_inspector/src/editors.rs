//! Deterministic ordinary-Rust editor registration over native reflection identities.
use crate::binding::ValueBinding;
use bevy::{
    prelude::*,
    reflect::{ParsedPath, TypeRegistry},
};
use kedra_ui::fields::FieldTemplates;
use std::{any::TypeId, collections::BTreeMap, sync::Arc};

/// A trusted built-in custom value presentation. It never receives mutable source data.
pub trait ValueEditor: Send + Sync {
    /// Mount an owned presentation subtree using shared kit controls.
    /// Errors must clean up allocations made by the callback before returning.
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String>;
}

/// Lifecycle of a mounted custom presentation. Native entities are UI handles only.
pub trait ValueEditorView: Send + Sync {
    /// Owned subtree root, attached underneath the supplied presentation parent.
    fn root(&self) -> Entity;
    /// Refresh against the same binding without reconstructing native controls.
    fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String>;
    /// Idempotently release the presentation subtree, not sources or sibling views.
    fn retire(&self, world: &mut World) {
        if let Ok(entity) = world.get_entity_mut(self.root()) {
            entity.despawn();
        }
    }
}

/// Shared presentation allowance, including recursive custom composition.
pub struct RenderBudget {
    pub(crate) remaining: usize,
    pub(crate) depth: usize,
}
impl Default for RenderBudget {
    fn default() -> Self {
        Self {
            remaining: 2048,
            depth: 0,
        }
    }
}
impl RenderBudget {
    /// Whether the shared reflected-discovery allowance has been consumed.
    /// Hosts show a limit indication rather than mounting further sibling trees.
    pub fn exhausted(&self) -> bool {
        self.remaining == 0
    }
}

/// Native kit and host registry used by both default and custom presentations.
/// `world` parameters elsewhere always refer to UI presentation, not inspected data.
pub struct EditorContext<'a> {
    /// Explicit reusable kit templates and rendering density.
    pub templates: &'a FieldTemplates,
    /// Host's authoritative Bevy registry.
    pub types: &'a TypeRegistry,
    /// Shared deterministic editor selection.
    pub editors: &'a EditorRegistry,
    /// Shared rebuild allowance; recursive consumers cannot reset this context's budget.
    pub budget: &'a mut RenderBudget,
    /// Host-assigned UI observation namespace.
    pub scope: &'a str,
}

/// Registry-local registration identity; duplicate keys never replace this identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorId(usize);

/// Selected registered custom editor. Default/unsupported selection is represented by None.
pub struct Selection<'a> {
    /// Stable identity within the registry's lifetime.
    pub id: EditorId,
    /// Concrete presentation implementation.
    pub editor: &'a Arc<dyn ValueEditor>,
}

#[derive(Default)]
/// Editor-owned presentation registrations; not a second type/schema registry.
pub struct EditorRegistry {
    types: BTreeMap<TypeId, (EditorId, Arc<dyn ValueEditor>)>,
    fields: BTreeMap<(TypeId, ParsedPath), (EditorId, Arc<dyn ValueEditor>)>,
    next: usize,
}
impl EditorRegistry {
    /// Register one exact native type. Duplicate registration leaves the registry unchanged.
    pub fn register_type(
        &mut self,
        ty: TypeId,
        editor: Arc<dyn ValueEditor>,
    ) -> Result<EditorId, String> {
        if self.types.contains_key(&ty) {
            return Err("Duplicate exact-type editor registration".into());
        }
        let id = self.allocate()?;
        self.types.insert(ty, (id, editor));
        Ok(id)
    }
    /// Register an explicit root-type/native-path specialization. Parser offsets do
    /// not form identity; `.field` and `field` refer to the same access sequence.
    pub fn register_field(
        &mut self,
        owner: TypeId,
        path: ParsedPath,
        editor: Arc<dyn ValueEditor>,
    ) -> Result<EditorId, String> {
        let key = (owner, canonical(&path));
        if self.fields.contains_key(&key) {
            return Err("Duplicate field editor registration".into());
        }
        let id = self.allocate()?;
        self.fields.insert(key, (id, editor));
        Ok(id)
    }
    fn allocate(&mut self) -> Result<EditorId, String> {
        let id = EditorId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or("Editor registration counter exhausted")?;
        Ok(id)
    }
    /// Resolve field, then exact-type specialization. Explicit default mode bypasses
    /// all specializations, including descendants when propagated by the caller.
    pub fn resolve(
        &self,
        owner: Option<TypeId>,
        path: Option<&ParsedPath>,
        ty: Option<TypeId>,
        default_mode: bool,
    ) -> Option<Selection<'_>> {
        if default_mode {
            return None;
        }
        owner
            .zip(path)
            .and_then(|(owner, path)| self.fields.get(&(owner, canonical(path))))
            .or_else(|| ty.and_then(|ty| self.types.get(&ty)))
            .map(|(id, editor)| Selection { id: *id, editor })
    }
}

fn canonical(path: &ParsedPath) -> ParsedPath {
    ParsedPath::from(path.0.iter().map(|p| p.access.clone()).collect::<Vec<_>>())
}
