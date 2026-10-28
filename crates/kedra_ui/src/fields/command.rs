//! Shared authored command surface; domain actions belong to the consumer.
use super::*;

/// A retained native command. Retiring the command releases only its own subtree.
pub struct CommandView {
    view: UiView,
    caption: Entity,
}
impl CommandView {
    /// Mount an explicit command template. Missing native behavior/caption fails
    /// without leaking a partial subtree. The scope is its observation identity.
    pub fn mount(
        world: &mut World,
        parent: Entity,
        templates: &FieldTemplates,
        scope: &str,
    ) -> Result<Self> {
        let view = templates.mount(world, parent, &templates.command, scope)?;
        let root = view.root;
        let result = (|| {
            let labels = world
                .get::<Children>(root)
                .into_iter()
                .flatten()
                .copied()
                .filter(|entity| world.get::<Text>(*entity).is_some())
                .collect::<Vec<_>>();
            let [caption] = labels.as_slice() else {
                return Err(kedra_scene::Error(
                    "Command template requires exactly one direct caption".into(),
                ));
            };
            let caption = *caption;
            if world.get::<bevy::ui_widgets::Button>(view.root).is_none()
                || world.get::<Text>(caption).is_none()
            {
                return Err(kedra_scene::Error(
                    "Command template requires a native button and caption".into(),
                ));
            }
            world
                .entity_mut(view.root)
                .insert(crate::ElementId::new(scope));
            world
                .entity_mut(caption)
                .insert(crate::ElementId::new(format!("{scope}/caption")));
            Ok(Self { caption, view })
        })();
        if result.is_err() {
            retire(world, root);
        }
        result
    }
    /// Native action target, not a source or gameplay entity.
    pub fn root(&self) -> Entity {
        self.view.root
    }
    /// Add non-interactive native choice/checked affordances owned by this command.
    /// The shared property stylesheet selects the relevant marker; actions and
    /// checked state remain consumer-controlled. Call once after mounting.
    pub fn choice_markers(&self, world: &mut World) {
        use bevy_flair::style::components::ClassList;
        let indicator = world
            .spawn((
                Node::default(),
                Pickable::IGNORE,
                ClassList::new_with_classes(["property-indicator"]),
                ChildOf(self.root()),
            ))
            .id();
        world.spawn((
            Node::default(),
            Pickable::IGNORE,
            ClassList::new_with_classes(["property-checkmark"]),
            ChildOf(indicator),
        ));
        world.spawn((
            Node::default(),
            Pickable::IGNORE,
            ClassList::new_with_classes(["property-chevron"]),
            ChildOf(self.root()),
        ));
    }
    /// Append a non-interactive native color sample to a palette command. The
    /// native Color is presentation data, never a source acceptance callback.
    pub fn color_marker(&self, world: &mut World, color: Color) {
        world.spawn((
            Node::default(),
            Pickable::IGNORE,
            BackgroundColor(color),
            bevy_flair::style::components::ClassList::new_with_classes(["property-swatch"]),
            ChildOf(self.root()),
        ));
    }
    /// Update visible text and native eligibility without reconstructing focus.
    pub fn update(&self, world: &mut World, label: &str, enabled: bool) -> Result<()> {
        text(world, self.caption, label)?;
        let mut root = world
            .get_entity_mut(self.view.root)
            .map_err(|_| kedra_scene::Error("Command retired".into()))?;
        if let Some(mut node) = root.get_mut::<bevy::a11y::AccessibilityNode>() {
            node.set_label(label);
        }
        if enabled {
            root.remove::<bevy::ui::InteractionDisabled>();
        } else {
            root.insert(bevy::ui::InteractionDisabled);
        }
        Ok(())
    }
    /// Idempotently release the retained native command.
    pub fn retire(&self, world: &mut World) {
        retire(world, self.view.root);
    }
}
