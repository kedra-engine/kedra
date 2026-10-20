//! Shared retained disclosure section; the consumer owns expansion state.
use super::*;

/// Authored disclosure header and retained body. Collapsing never unmounts children.
pub struct SectionView {
    view: UiView,
    title: Entity,
    toggle: Entity,
    body: Entity,
}
impl SectionView {
    /// Mount the supplied section template, releasing allocations on invalid bindings.
    pub fn mount(
        world: &mut World,
        parent: Entity,
        templates: &FieldTemplates,
        scope: &str,
    ) -> Result<Self> {
        let view = templates.mount(world, parent, &templates.section, scope)?;
        let root = view.root;
        let result = (|| {
            Ok(Self {
                title: view.element("title")?,
                toggle: view.element("toggle")?,
                body: view.element("body")?,
                view,
            })
        })();
        if result.is_err() {
            retire(world, root);
        }
        result
    }
    /// Owned root, preserved across placement changes.
    pub fn root(&self) -> Entity {
        self.view.root
    }
    /// Native disclosure action target; hosts bind their own typed actions here.
    pub fn toggle(&self) -> Entity {
        self.toggle
    }
    /// Parent for retained property controls and child sections.
    pub fn body(&self) -> Entity {
        self.body
    }
    /// Update presentation without touching descendant widgets or their state.
    pub fn update(&mut self, world: &mut World, title: &str, expanded: bool) -> Result<()> {
        text(world, self.title, title)?;
        crate::docking::class(world, self.body, "hidden", !expanded);
        crate::docking::class(world, self.root(), "collapsed", !expanded);
        if let Ok(down) = self.view.element("down") {
            crate::docking::class(world, down, "hidden", !expanded);
        }
        if let Ok(right) = self.view.element("right") {
            crate::docking::class(world, right, "hidden", expanded);
        }
        Ok(())
    }
    /// Idempotently release only the section's owned subtree.
    pub fn retire(&self, world: &mut World) {
        retire(world, self.root());
    }
}
