//! Copyable native read-only value surface, independent of its property's schema.
use super::*;
use bevy::text::{EditableText, TextReadWriteMode};

/// Retained native read-only text. The host supplies bounded presentation text.
/// No acceptance callbacks or mutable domain values are owned by this control.
pub struct ValueView {
    root: Entity,
    input: Entity,
    displayed: Option<String>,
}
impl ValueView {
    /// Mount an authored value template containing a native `value` input.
    /// Invalid templates release their complete newly allocated hierarchy.
    pub fn mount(
        world: &mut World,
        parent: Entity,
        templates: &FieldTemplates,
        scope: &str,
    ) -> Result<Self> {
        let view = templates.mount(world, parent, &templates.value, scope)?;
        let result = view.element("value").and_then(|input| {
            if world.get::<EditableText>(input).is_none() {
                return Err(kedra_scene::Error(
                    "Value template requires native text input".into(),
                ));
            }
            world.entity_mut(input).insert((
                FieldAccess::ReadOnly {
                    reason: "Read-only value".into(),
                },
                TextReadWriteMode::ReadOnly,
            ));
            Ok(Self {
                root: view.root,
                input,
                displayed: None,
            })
        });
        if result.is_err() {
            retire(world, view.root);
        }
        result
    }
    /// Native input identity; stable until the value view is retired.
    pub fn input(&self) -> Entity {
        self.input
    }
    /// Root owned by this surface, not its surrounding property row.
    pub fn root(&self) -> Entity {
        self.root
    }
    /// Associate the persistent property label and description with native input.
    /// This sets native accessibility metadata and a shared hover hint, not a
    /// substitute text engine or a claim of screen-reader platform certification.
    pub fn describe(&self, world: &mut World, label: &str, description: &str) -> Result<()> {
        let mut input = world
            .get_entity_mut(self.input)
            .map_err(|_| kedra_scene::Error("Value view was retired".into()))?;
        if let Some(mut node) = input.get_mut::<bevy::a11y::AccessibilityNode>() {
            node.set_label(label);
            node.set_description(description);
        }
        let hint = crate::tooltip::Tooltip(format!("{label}\n{description}"));
        if input.get::<crate::tooltip::Tooltip>() != Some(&hint) {
            input.insert(hint);
        }
        Ok(())
    }
    /// Replace presentation text only when changed, preserving native selection on echo.
    /// Exact source values and truncation policy remain host responsibilities.
    pub fn update(&mut self, world: &mut World, display: &str, access: &FieldAccess) -> Result<()> {
        let mut input = world
            .get_entity_mut(self.input)
            .map_err(|_| kedra_scene::Error("Value view was retired".into()))?;
        if input.get::<FieldAccess>() != Some(access) {
            input.insert(access.clone());
        }
        if self.displayed.as_deref() != Some(display) {
            input
                .get_mut::<EditableText>()
                .ok_or_else(|| kedra_scene::Error("Native value input disappeared".into()))?
                .editor_mut()
                .set_text(display);
            self.displayed = Some(display.into());
        }
        Ok(())
    }
    /// Idempotently release only this value surface.
    pub fn retire(&self, world: &mut World) {
        retire(world, self.root);
    }
}
