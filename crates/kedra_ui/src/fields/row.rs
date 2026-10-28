//! Retained labeled editor slots; widgets placed in the slot own their values.
use super::*;

#[derive(Component)]
struct DiagnosticTarget(Entity);

/// A property label, arbitrary editor slot and associated diagnostic.
/// Retiring the row also retires controls mounted beneath its slot.
pub struct PropertyRow {
    view: UiView,
    label: Entity,
    editor: Entity,
    diagnostic: Entity,
}
impl PropertyRow {
    /// Mount the supplied authored row. Invalid bindings reclaim all allocations.
    pub fn mount(
        world: &mut World,
        parent: Entity,
        templates: &FieldTemplates,
        scope: &str,
    ) -> Result<Self> {
        let view = templates.mount(world, parent, &templates.row, scope)?;
        let root = view.root;
        let result = (|| {
            let editor = view.element("editor")?;
            let diagnostic = view.element("diagnostic")?;
            world
                .entity_mut(editor)
                .insert(DiagnosticTarget(diagnostic));
            Ok(Self {
                label: view.element("label")?,
                editor,
                diagnostic,
                view,
            })
        })();
        if result.is_err() {
            retire(world, root);
        }
        result
    }
    /// Owned retained root, for attachment and host movement only.
    pub fn root(&self) -> Entity {
        self.view.root
    }
    /// Parent for arbitrary native value editors, including compound controls.
    pub fn editor(&self) -> Entity {
        self.editor
    }
    /// Retained diagnostic text target for an input adapter's bounded feedback.
    pub fn diagnostic(&self) -> Entity {
        self.diagnostic
    }
    /// Diagnostic owned by this exact editor slot, when it belongs to a property row.
    /// Compound controls may share the target without discovering ancestors or IDs.
    /// The returned presentation entity is invalid after its owning row is retired.
    pub fn diagnostic_for(world: &World, editor: Entity) -> Option<Entity> {
        world.get::<DiagnosticTarget>(editor).map(|target| target.0)
    }
    /// Update labels without replacing value widgets, native focus or selection.
    pub fn update(
        &mut self,
        world: &mut World,
        label: &str,
        diagnostic: Option<&str>,
    ) -> Result<()> {
        text(world, self.label, label)?;
        text(world, self.diagnostic, diagnostic.unwrap_or(""))?;
        crate::docking::class(world, self.diagnostic, "hidden", diagnostic.is_none());
        Ok(())
    }
    /// Idempotently release this row and descendants, never siblings or its parent.
    pub fn retire(&self, world: &mut World) {
        retire(world, self.root());
    }
}
