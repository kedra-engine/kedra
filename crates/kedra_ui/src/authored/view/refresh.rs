//! Staged passive value refresh. No interactive behavior or source authority.
use super::*;

/// Preflighted passive presentation update. Holds its source snapshot and original
/// instance mapping; publication refuses retired/replaced ownership before mutation.
pub struct PassiveRefresh {
    root: Entity,
    nodes: BTreeMap<NodeAddress, Entity>,
    source: Arc<CompiledPrefab>,
    images: BTreeMap<Entity, Handle<Image>>,
    baseline: Arc<CompiledPrefab>,
}
impl PassiveView {
    /// Stage values for unchanged passive topology. Native focus, modal ownership,
    /// observation IDs, cameras and layout-derived components are never copied.
    /// Missing layout/text/image/sheet dependencies reject without live mutation.
    pub fn prepare_refresh(
        &self,
        world: &World,
        source: Arc<CompiledPrefab>,
        density: f32,
    ) -> kedra_scene::Result<PassiveRefresh> {
        if !density.is_finite() || density <= 0. {
            return Err(kedra_scene::Error(
                "Passive refresh requires positive finite density".into(),
            ));
        }
        let instance = world
            .get::<PrefabInstance>(self.root)
            .ok_or_else(|| kedra_scene::Error("Passive instance retired".into()))?;
        let nodes = instance.nodes.clone();
        let baseline = instance.source.clone();
        if nodes.keys().ne(source.source_entities().keys()) {
            return Err(kedra_scene::Error(
                "Passive refresh requires unchanged topology".into(),
            ));
        }
        let mut images = BTreeMap::new();
        for (address, native) in &nodes {
            let before = &baseline.graph().nodes[address];
            let after = &source.graph().nodes[address];
            if before.parent != after.parent
                || before.children != after.children
                || before.components.keys().ne(after.components.keys())
            {
                return Err(kedra_scene::Error(
                    "Passive refresh requires unchanged component membership and hierarchy".into(),
                ));
            }
            let entity = source.source_entities()[address];
            let authored = source.authoring_world();
            if world.get::<PassiveUi>(*native).is_none()
                || authored.get::<UiLayout>(entity).is_some()
                    != world.get::<Node>(*native).is_some()
            {
                return Err(kedra_scene::Error(
                    "Passive layout ownership changed".into(),
                ));
            }
            if authored.get::<UiCaption>(entity).is_some()
                && (world.get::<Text>(*native).is_none()
                    || world.get::<TextLayout>(*native).is_none()
                    || !has_stylesheet(authored, Some(entity), true))
            {
                return Err(kedra_scene::Error(
                    "Passive caption presentation unavailable".into(),
                ));
            }
            if let Some(sheet) = authored.get::<UiStylesheet>(entity)
                && !world
                    .get_resource::<Assets<bevy_flair::style::StyleSheet>>()
                    .is_some_and(|sheets| sheets.contains(sheet.asset.id()))
            {
                return Err(kedra_scene::Error(
                    "Passive stylesheet dependency unavailable".into(),
                ));
            }
            if let Some(image) = authored.get::<UiImage>(entity) {
                if world.get::<ImageNode>(*native).is_none() {
                    return Err(kedra_scene::Error(
                        "Passive image presentation unavailable".into(),
                    ));
                }
                images.insert(*native, select_image(world, image, density)?);
            }
        }
        Ok(PassiveRefresh {
            root: self.root,
            nodes,
            source,
            images,
            baseline,
        })
    }
}
impl PassiveRefresh {
    /// Publish synchronously after all other presentation preflight. A replaced
    /// instance or retired node rejects before changing any surviving node.
    pub fn apply(self, world: &mut World) -> kedra_scene::Result<()> {
        if world
            .get::<PrefabInstance>(self.root)
            .is_none_or(|instance| {
                instance.nodes != self.nodes || !Arc::ptr_eq(&instance.source, &self.baseline)
            })
            || self
                .nodes
                .values()
                .any(|entity| world.get::<PassiveUi>(*entity).is_none())
        {
            return Err(kedra_scene::Error(
                "Passive refresh owner retired or replaced".into(),
            ));
        }
        for (address, entity) in &self.nodes {
            let source = self.source.authoring_world();
            let authored = self.source.source_entities()[address];
            if source.get::<UiLayout>(authored).is_some() && world.get::<Node>(*entity).is_none()
                || source.get::<UiCaption>(authored).is_some()
                    && (world.get::<Text>(*entity).is_none()
                        || world.get::<TextLayout>(*entity).is_none())
                || self.images.get(entity).is_some_and(|image| {
                    world.get::<ImageNode>(*entity).is_none()
                        || !world
                            .get_resource::<Assets<Image>>()
                            .is_some_and(|images| images.contains(image.id()))
                })
                || source.get::<UiStylesheet>(authored).is_some_and(|sheet| {
                    !world
                        .get_resource::<Assets<bevy_flair::style::StyleSheet>>()
                        .is_some_and(|sheets| sheets.contains(sheet.asset.id()))
                })
            {
                return Err(kedra_scene::Error(
                    "Passive refresh presentation or dependency retired".into(),
                ));
            }
        }
        let source = self.source.authoring_world();
        for (address, entity) in self.nodes {
            let authored = self.source.source_entities()[&address];
            let changed = |key: &str| {
                self.baseline.graph().nodes[&address].components.get(key)
                    != self.source.graph().nodes[&address].components.get(key)
            };
            macro_rules! copy {
                ($($ty:ty => $key:literal),* $(,)?) => { $(if changed($key) && let Some(value) = source.get::<$ty>(authored) { world.entity_mut(entity).insert(value.clone()); })* };
            }
            // Inert authored data remains available for density/animation/style systems.
            copy!(UiLayout => "ui.layout", UiCaption => "ui.caption", UiClasses => "ui.classes", UiStylesheet => "ui.stylesheet", UiImage => "ui.image", UiSpin => "ui.spin",
                BackgroundColor => "ui.background", BorderColor => "ui.border", BoxShadow => "ui.shadow", ZIndex => "ui.z", GlobalZIndex => "ui.global_z", Outline => "ui.outline");
            if changed("ui.layout")
                && let Some(layout) = source.get::<UiLayout>(authored)
            {
                world.entity_mut(entity).insert(layout.node.clone());
            }
            if changed("ui.classes")
                && let Some(classes) = source.get::<UiClasses>(authored)
            {
                world.entity_mut(entity).insert(
                    bevy_flair::style::components::ClassList::new_with_classes(
                        classes.names.clone(),
                    ),
                );
            }
            if changed("ui.stylesheet")
                && let Some(sheet) = source.get::<UiStylesheet>(authored)
            {
                world
                    .entity_mut(entity)
                    .insert(bevy_flair::style::components::Styled::new(
                        sheet.asset.clone(),
                    ));
            }
            if changed("ui.caption")
                && let Some(caption) = source.get::<UiCaption>(authored)
            {
                if let Some(mut text) = world.get_mut::<Text>(entity) {
                    text.0.clone_from(&caption.text);
                }
                if let Some(mut layout) = world.get_mut::<TextLayout>(entity) {
                    layout.justify = caption.justify;
                    layout.linebreak = if caption.wrap {
                        bevy::text::LineBreak::WordBoundary
                    } else {
                        bevy::text::LineBreak::NoWrap
                    };
                }
            }
            if changed("ui.image")
                && let Some(image) = source.get::<UiImage>(authored)
                && let Some(mut node) = world.get_mut::<ImageNode>(entity)
            {
                node.image.clone_from(&self.images[&entity]);
                node.color = image.color;
            }
            // Authored picking/behavior flags never enable passive input.
            world
                .entity_mut(entity)
                .insert(bevy::picking::Pickable::IGNORE);
        }
        if let Some(mut instance) = world.get_mut::<PrefabInstance>(self.root) {
            instance.source = self.source;
        }
        Ok(())
    }
}
