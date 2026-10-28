//! Base-owned chrome for every custom presentation, independent of drawer language.
use super::Disclosure;
use crate::editors::EditorLayout;
use bevy::prelude::*;
use kedra_ui::{
    docking::class,
    fields::{FieldTemplates, PropertyRow, SectionView},
};

pub(super) enum EditorSlot {
    Field(PropertyRow),
    Block(SectionView, PropertyRow),
}
impl EditorSlot {
    pub(super) fn mount(
        world: &mut World,
        parent: Entity,
        templates: &FieldTemplates,
        scope: &str,
        layout: EditorLayout,
        root: bool,
    ) -> Result<Self, String> {
        if layout == EditorLayout::Field {
            return PropertyRow::mount(world, parent, templates, scope)
                .map(Self::Field)
                .map_err(|error| error.to_string());
        }
        let section = SectionView::mount(world, parent, templates, &format!("{scope}/chrome"))
            .map_err(|error| error.to_string())?;
        // A root already has its TreeView's component header. Nested block
        // values get the exact same section/disclosure primitive as default groups.
        class(world, section.root(), "property-composition", root);
        match PropertyRow::mount(
            world,
            section.body(),
            templates,
            &format!("{scope}/diagnostic"),
        ) {
            Ok(diagnostic) => {
                class(world, diagnostic.root(), "property-diagnostic-row", true);
                class(world, diagnostic.root(), "hidden", true);
                Ok(Self::Block(section, diagnostic))
            }
            Err(error) => {
                section.retire(world);
                Err(error.to_string())
            }
        }
    }
    pub(super) fn root(&self) -> Entity {
        match self {
            Self::Field(row) => row.root(),
            Self::Block(section, _) => section.root(),
        }
    }
    pub(super) fn editor(&self) -> Entity {
        match self {
            Self::Field(row) => row.editor(),
            Self::Block(section, _) => section.body(),
        }
    }
    pub(super) fn update(
        &mut self,
        world: &mut World,
        label: &str,
        diagnostic: Option<&str>,
        expanded: bool,
        disclosure: Disclosure,
    ) -> Result<(), String> {
        match self {
            Self::Field(row) => row
                .update(world, label, diagnostic)
                .map_err(|error| error.to_string()),
            Self::Block(section, message) => {
                let title = diagnostic
                    .map_or_else(|| label.to_owned(), |reason| format!("{label} · {reason}"));
                section
                    .update(world, &title, expanded)
                    .map_err(|error| error.to_string())?;
                world.entity_mut(section.toggle()).insert(disclosure);
                message
                    .update(world, "", diagnostic)
                    .map_err(|error| error.to_string())?;
                class(world, message.root(), "hidden", diagnostic.is_none());
                Ok(())
            }
        }
    }
    pub(super) fn retire(&self, world: &mut World) {
        match self {
            Self::Field(row) => row.retire(world),
            Self::Block(section, _) => section.retire(world),
        }
    }
}
