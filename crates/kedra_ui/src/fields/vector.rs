//! Shared inline compound presentation; native value surfaces own selection/copy.
use super::*;

/// Two to four labeled, retained native value surfaces in one authored row.
/// This control owns no reflection, rotation math or source editing authority.
pub struct VectorView {
    view: UiView,
    values: Vec<ValueView>,
}

impl VectorView {
    /// Mount X/Y[/Z/W] from explicit templates. Invalid dimensions or bindings
    /// reclaim every allocation and leave siblings unchanged.
    pub fn mount(
        world: &mut World,
        parent: Entity,
        templates: &FieldTemplates,
        scope: &str,
        dimensions: usize,
    ) -> Result<Self> {
        if !(2..=4).contains(&dimensions) {
            return Err(kedra_scene::Error(
                "Vector fields require two to four axes".into(),
            ));
        }
        let view = templates.mount(world, parent, &templates.vector, scope)?;
        let root = view.root;
        let result = (|| {
            let mut values = Vec::new();
            for (index, axis) in ["x", "y", "z", "w"].iter().enumerate() {
                let row = view.element(&format!("axis-{axis}"))?;
                let slot = view.element(&format!("value-{axis}"))?;
                crate::docking::class(world, row, "hidden", index >= dimensions);
                if index < dimensions {
                    values.push(ValueView::mount(
                        world,
                        slot,
                        templates,
                        &format!("{scope}/{axis}"),
                    )?);
                }
            }
            Ok(Self { view, values })
        })();
        if result.is_err() {
            retire(world, root);
        }
        result
    }

    /// Root owned by this compound control, retained through refresh and host moves.
    pub fn root(&self) -> Entity {
        self.view.root
    }

    /// Native input handles in X/Y/Z/W order; these are UI handles, not property identities.
    pub fn inputs(&self) -> impl Iterator<Item = Entity> + '_ {
        self.values.iter().map(ValueView::input)
    }

    /// Associate axis-specific accessibility/provenance text with an existing
    /// native surface. An out-of-range axis leaves every surface unchanged.
    pub fn describe_axis(
        &self,
        world: &mut World,
        axis: usize,
        label: &str,
        description: &str,
    ) -> Result<()> {
        self.values
            .get(axis)
            .ok_or_else(|| kedra_scene::Error("Vector axis is unavailable".into()))?
            .describe(world, label, description)
    }

    /// Refresh exact display strings without rebuilding controls. A different axis
    /// count is an error, not an implicit remount. Access applies to every axis.
    pub fn update(
        &mut self,
        world: &mut World,
        values: &[String],
        label: &str,
        description: &str,
        access: &FieldAccess,
    ) -> Result<()> {
        if values.len() != self.values.len() {
            return Err(kedra_scene::Error("Vector field dimension changed".into()));
        }
        let description = format!("{description}\n{}", access.description());
        for ((view, value), axis) in self.values.iter_mut().zip(values).zip(["X", "Y", "Z", "W"]) {
            view.update(world, value, access)?;
            view.describe(world, &format!("{label} {axis}"), &description)?;
        }
        Ok(())
    }

    /// Release this control and its inputs, never the surrounding row or siblings.
    pub fn retire(&self, world: &mut World) {
        retire(world, self.root());
    }
}
