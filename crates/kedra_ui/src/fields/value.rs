//! Copyable native read-only value surface, independent of its property's schema.
use super::*;
use bevy::text::{EditableText, TextReadWriteMode};

/// Semantic native resource icon from the host's authored field kit. This is a
/// presentation category, never a loader/type or source-identity discriminator.
#[derive(Clone, Copy)]
pub enum AssetIcon {
    /// An image resource.
    Image,
    /// A model or native scene resource.
    Model,
    /// A scene environment resource.
    Environment,
    /// Another resource, including a stylesheet.
    File,
}

/// Retained native read-only text. The host supplies bounded presentation text.
/// No acceptance callbacks or mutable domain values are owned by this control.
pub struct ValueView {
    root: Entity,
    input: Entity,
    swatch: Option<Entity>,
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
                swatch: view.element("swatch").ok(),
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
    /// Select an authored type icon without changing the native path buffer or
    /// granting text editing. Older kits without these optional slots keep the
    /// path well usable; current editor acceptance requires the authored icons.
    pub fn asset_icon(&self, world: &mut World, icon: AssetIcon) {
        for (class, enabled) in [
            ("asset-image", matches!(icon, AssetIcon::Image)),
            ("asset-model", matches!(icon, AssetIcon::Model)),
            ("asset-environment", matches!(icon, AssetIcon::Environment)),
            ("asset-file", matches!(icon, AssetIcon::File)),
        ] {
            crate::docking::class(world, self.root, class, enabled);
        }
    }
    /// Optional authored color-sample target. Consumers may attach native behavior;
    /// retirement remains owned by this value surface, never the surrounding row.
    pub fn swatch(&self) -> Option<Entity> {
        self.swatch
    }
    /// Show a native color sample in the authored swatch slot. The value text and
    /// color-space semantics remain the consumer's responsibility. Missing slots
    /// reject without allocating replacement UI or changing the input buffer.
    pub fn color_sample(&self, world: &mut World, color: Color) -> Result<()> {
        let swatch = self.swatch.ok_or_else(|| {
            kedra_scene::Error("Value template has no authored color swatch".into())
        })?;
        world
            .get_entity_mut(swatch)
            .map_err(|_| kedra_scene::Error("Color swatch was retired".into()))?
            .insert(BackgroundColor(color));
        crate::docking::class(world, swatch, "hidden", false);
        crate::docking::class(world, self.root, "color", true);
        Ok(())
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
        let managed = input.contains::<crate::fields::TextField>();
        if !managed && self.displayed.as_deref() != Some(display) {
            input
                .get_mut::<EditableText>()
                .ok_or_else(|| kedra_scene::Error("Native value input disappeared".into()))?
                .editor_mut()
                .set_text(display);
            self.displayed = Some(display.into());
        }
        crate::docking::class(world, self.root, "editable", !access.is_read_only());
        Ok(())
    }
    /// Idempotently release only this value surface.
    pub fn retire(&self, world: &mut World) {
        retire(world, self.root);
    }
}
