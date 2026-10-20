//! Consumer-neutral retained property controls over authored native UI.
//!
//! Hosts supply compiled templates, labels and display values. These controls own
//! only presentation, never reflection, documents, validation or persistence.

use crate::authored::view::UiView;
use bevy::prelude::*;
use kedra_scene::{Result, instance::CompiledPrefab};
use std::{collections::BTreeMap, sync::Arc};
mod row;
mod section;
mod value;
pub use row::PropertyRow;
pub(crate) use row::responsive_rows;
pub use section::SectionView;
pub use value::ValueView;

/// Host-supplied authored kit templates. No asset identity is hard-coded in controls.
#[derive(Clone)]
pub struct FieldTemplates {
    /// Row with `label`, `editor` and `diagnostic` local bindings.
    pub row: Arc<CompiledPrefab>,
    /// Section with `title`, `toggle` and `body` local bindings.
    pub section: Arc<CompiledPrefab>,
    /// Native text surface with a `value` local binding.
    pub value: Arc<CompiledPrefab>,
    /// Explicit native rendering density; the template must support it.
    pub density: f32,
}
impl FieldTemplates {
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        template: &Arc<CompiledPrefab>,
        scope: &str,
    ) -> Result<UiView> {
        if !world.entities().contains(parent) {
            return Err(kedra_scene::Error("Field parent was retired".into()));
        }
        UiView::mount(
            world,
            template,
            Some(parent),
            &BTreeMap::new(),
            scope,
            self.density,
        )
    }
}
fn text(world: &mut World, entity: Entity, value: &str) -> Result<()> {
    let mut text = world
        .get_mut::<Text>(entity)
        .ok_or_else(|| kedra_scene::Error("Field text binding disappeared".into()))?;
    if text.0 != value {
        text.0 = value.into();
    }
    Ok(())
}
fn retire(world: &mut World, root: Entity) {
    if let Ok(entity) = world.get_entity_mut(root) {
        entity.despawn();
    }
}

/// Access declared by a field host, independently of temporary interaction disablement.
/// Absence of this component preserves existing authored input behavior.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum FieldAccess {
    /// Native selection and copying remain available; mutation is prohibited.
    ReadOnly {
        /// Human-readable explanation associated with the value presentation.
        reason: String,
    },
}
