//! Retained labeled editor slots; widgets placed in the slot own their values.
use super::*;

#[derive(Component)]
pub(crate) struct ResponsivePropertyRow;

type ResponsiveRows<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedNode,
        &'static mut bevy_flair::style::components::ClassList,
    ),
    (With<ResponsivePropertyRow>, Changed<ComputedNode>),
>;

pub(crate) fn responsive_rows(mut rows: ResponsiveRows<'_, '_>) {
    for (node, mut classes) in &mut rows {
        let width = node.size().x * node.inverse_scale_factor();
        if width.is_finite() && width > 0. && classes.contains("compact") != (width < 296.) {
            classes.toggle("compact");
        }
    }
}

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
        world.entity_mut(root).insert(ResponsivePropertyRow);
        let result = (|| {
            Ok(Self {
                label: view.element("label")?,
                editor: view.element("editor")?,
                diagnostic: view.element("diagnostic")?,
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
        let width = world
            .get::<ComputedNode>(self.root())
            .map(|n| n.size().x * n.inverse_scale_factor());
        if let Some(width) = width.filter(|w| *w > 0.) {
            crate::docking::class(world, self.root(), "compact", width < 296.);
        }
        Ok(())
    }
    /// Idempotently release this row and descendants, never siblings or its parent.
    pub fn retire(&self, world: &mut World) {
        retire(world, self.root());
    }
}
