//! Consumer-neutral retained property controls over authored native UI.
//!
//! Hosts supply compiled templates, labels and display values. Shared controls own
//! presentation and native text acknowledgment, never reflection, documents,
//! domain validation or persistence.

use crate::authored::view::UiView;
use bevy::prelude::*;
use kedra_scene::{Result, instance::CompiledPrefab};
use std::{collections::BTreeMap, sync::Arc};
mod acceptance;
mod command;
mod native_text;
mod row;
mod section;
mod value;
mod vector;
pub use acceptance::{AcceptanceError, AcceptanceRequest, TextAcceptance};
pub use command::CommandView;
pub use native_text::LiveTextField;
pub use native_text::{TextField, TextFieldEvent, TextFieldOutcome};
pub(crate) use native_text::{field_keyboard, install_text_fields};
pub use row::PropertyRow;
pub use section::SectionView;
pub use value::{AssetIcon, ValueView};
pub use vector::VectorView;

/// Mount a non-interactive insertion line in a retained property row. The authored
/// stylesheet owns geometry/color and the parent `drop-target` state controls its
/// visibility. Retiring the row retires this child; it owns no drag or edit authority.
pub fn insertion_marker(world: &mut World, parent: Entity) -> Entity {
    world
        .spawn((
            Node::default(),
            Pickable::IGNORE,
            bevy_flair::style::components::ClassList::new_with_classes([
                "property-insertion-marker",
            ]),
            ChildOf(parent),
        ))
        .id()
}

/// Host-supplied authored kit templates. No asset identity is hard-coded in controls.
#[derive(Clone)]
pub struct FieldTemplates {
    /// Row with `label`, `editor` and `diagnostic` local bindings.
    pub row: Arc<CompiledPrefab>,
    /// Section with `title`, `toggle` and `body` local bindings.
    pub section: Arc<CompiledPrefab>,
    /// Native text surface with a `value` local binding.
    pub value: Arc<CompiledPrefab>,
    /// Inline compound surface with `axis-x`/`value-x` through `axis-w`/`value-w` slots.
    pub vector: Arc<CompiledPrefab>,
    /// Native command with exactly one direct caption, used for choices and collection actions.
    pub command: Arc<CompiledPrefab>,
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
    /// The host supplies a validated editing adapter; this alone grants no source authority.
    Editable,
    /// Native selection and copying remain available; mutation is prohibited.
    ReadOnly {
        /// Human-readable explanation associated with the value presentation.
        reason: String,
    },
}
impl FieldAccess {
    /// Human-readable interaction/access explanation for the native control.
    pub fn description(&self) -> &str {
        match self {
            Self::Editable => "Enter accepts; Escape cancels. Save is a separate host action.",
            Self::ReadOnly { reason } => reason,
        }
    }
    /// Whether native mutation is prohibited independently of pending state.
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::ReadOnly { .. })
    }
}
