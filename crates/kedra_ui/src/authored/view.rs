//! Instance-owned UI hydration. Source layouts are not reconstructed by editor screens.
use super::components::*;
use bevy::{
    input_focus::{
        AutoFocus,
        tab_navigation::{TabGroup, TabIndex},
    },
    prelude::*,
    text::{EditableText, TextCursorStyle},
    ui_widgets::{Button, ModalDialog, ModalDialogBarrier, SelectAllOnFocus, TextInput},
};
use kedra_scene::{
    document::NodeAddress,
    instance::{CompiledPrefab, PrefabInstance},
};
use std::{collections::BTreeMap, sync::Arc};

/// Ownership of one hydrated UI instance and its local semantic bindings.
pub struct UiView {
    /// Root owns its complete authored hierarchy and immutable source compilation.
    pub root: Entity,
    elements: BTreeMap<String, Entity>,
}

/// Disposable passive presentation. Native UI roots are detached from spatial
/// parents: Bevy layout and Flair require a UI tree, not a Transform hierarchy.
/// Persistent source addresses remain in the source root's PrefabInstance mapping.
pub struct PassiveView {
    /// Source instance root, which may be spatial or one of the UI roots.
    pub root: Entity,
    /// Native offscreen UI roots. Assign explicit UiTargetCamera before rendering.
    pub ui_roots: Vec<Entity>,
}
impl PassiveView {
    /// Idempotently retire detached UI trees and the source instance. Never
    /// touches external bindings, cameras or another presentation's entities.
    pub fn retire(&self, world: &mut World) {
        for entity in self
            .ui_roots
            .iter()
            .copied()
            .chain(std::iter::once(self.root))
        {
            if let Ok(entity) = world.get_entity_mut(entity) {
                entity.despawn();
            }
        }
    }
}
impl UiView {
    /// Mount a complete authored UI entry scene with source-owned observation IDs.
    /// The host supplies its window/camera; no scripts or extra plugins are loaded.
    /// The returned root owns the full hydrated instance and can be despawned to retire it.
    pub fn mount_scene(
        world: &mut World,
        scene: &Arc<CompiledPrefab>,
    ) -> kedra_scene::Result<Entity> {
        Ok(Self::mount(world, scene, None, &BTreeMap::new(), "", 1.)?.root)
    }

    /// Materialize a validated source using the shared scene runtime, then install
    /// native UI components. All source/resource failures are checked before allocation.
    /// `scope` explicitly namespaces runtime observation IDs; lookups remain local.
    pub fn mount(
        world: &mut World,
        snapshot: &Arc<CompiledPrefab>,
        parent: Option<Entity>,
        bindings: &BTreeMap<String, Option<Entity>>,
        scope: &str,
        density: f32,
    ) -> kedra_scene::Result<Self> {
        Self::mount_mode(world, snapshot, parent, bindings, scope, density, false)
    }

    /// Materialize a passive offscreen presentation, including UI subtrees of a
    /// mixed spatial graph. No buttons, text input, autofocus, modal ownership or
    /// public observation IDs are installed. The host supplies an explicit target
    /// camera for every UI layout node and owns rendering/retirement separately.
    /// Sources remain unchanged. Dependency/layout failures allocate no live graph.
    /// Text must have a stylesheet within its resulting detached UI tree; sheets
    /// on external parents or across non-layout ancestors are not inherited.
    pub fn mount_passive(
        world: &mut World,
        snapshot: &Arc<CompiledPrefab>,
        parent: Option<Entity>,
        density: f32,
    ) -> kedra_scene::Result<PassiveView> {
        let view = Self::mount_mode(world, snapshot, parent, &BTreeMap::new(), "", density, true)?;
        let nodes = world
            .get::<PrefabInstance>(view.root)
            .ok_or_else(|| kedra_scene::Error("passive instance mapping missing".into()))?
            .nodes
            .clone();
        let ui_roots = nodes
            .values()
            .copied()
            .filter(|entity| {
                world.get::<UiLayout>(*entity).is_some()
                    && world
                        .get::<ChildOf>(*entity)
                        .is_none_or(|parent| world.get::<UiLayout>(parent.parent()).is_none())
            })
            .collect::<Vec<_>>();
        for root in &ui_roots {
            world.entity_mut(*root).remove::<ChildOf>();
        }
        Ok(PassiveView {
            root: view.root,
            ui_roots,
        })
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "shared hydration keeps live and passive projection on one validated path"
    )]
    fn mount_mode(
        world: &mut World,
        snapshot: &Arc<CompiledPrefab>,
        parent: Option<Entity>,
        bindings: &BTreeMap<String, Option<Entity>>,
        scope: &str,
        density: f32,
        passive: bool,
    ) -> kedra_scene::Result<Self> {
        if !density.is_finite() || density <= 0. {
            return Err(kedra_scene::Error(
                "UI density must be finite and positive".into(),
            ));
        }
        let runtime = world
            .get_resource::<UiFailure>()
            .ok_or_else(|| kedra_scene::Error("AuthoredUiPlugin is not installed".into()))?;
        if let Some(error) = &runtime.0 {
            return Err(kedra_scene::Error(format!(
                "UI runtime is unavailable: {error}"
            )));
        }
        let source = snapshot.authoring_world();
        let root_source = snapshot
            .source_entities()
            .get(&snapshot.graph().root)
            .ok_or_else(|| kedra_scene::Error("compiled UI source has no root".into()))?;
        if !passive && source.get::<UiLayout>(*root_source).is_none() {
            return Err(kedra_scene::Error(
                "authored UI root requires a native layout".into(),
            ));
        }
        let mut keys = BTreeMap::<String, NodeAddress>::new();
        for (address, entity) in snapshot.source_entities() {
            if let Some(key) = source.get::<crate::ElementId>(*entity)
                && keys.insert(key.0.clone(), address.clone()).is_some()
            {
                return Err(kedra_scene::Error(format!(
                    "duplicate UI identity in instance: {}",
                    key.0
                )));
            }
            if let Some(sheet) = source.get::<UiStylesheet>(*entity)
                && !world
                    .get_resource::<Assets<bevy_flair::style::StyleSheet>>()
                    .is_some_and(|sheets| sheets.contains(sheet.asset.id()))
            {
                return Err(kedra_scene::Error(format!(
                    "UI stylesheet dependency is not materialized: {address:?}"
                )));
            }
            if let Some(image) = source.get::<UiImage>(*entity) {
                select_image(world, image, density)?;
            }
            if source.get::<UiCaption>(*entity).is_some()
                && !has_stylesheet(source, Some(*entity), passive)
                && (passive || !has_stylesheet(world, parent, false))
            {
                return Err(kedra_scene::Error(
                    "authored text requires an explicit stylesheet scope".into(),
                ));
            }
            if source.get::<UiEditable>(*entity).is_some()
                && source.get::<UiCaption>(*entity).is_none()
            {
                return Err(kedra_scene::Error(
                    "editable UI requires authored text content".into(),
                ));
            }
            if (source.get::<UiCaption>(*entity).is_some()
                || source.get::<UiButton>(*entity).is_some()
                || source.get::<UiImage>(*entity).is_some())
                && source.get::<UiLayout>(*entity).is_none()
            {
                return Err(kedra_scene::Error(
                    "native text/buttons require an authored UI layout".into(),
                ));
            }
        }
        let root = snapshot.instantiate(world, parent, bindings)?;
        let nodes = world
            .get::<PrefabInstance>(root)
            .ok_or_else(|| {
                kedra_scene::Error("scene runtime did not publish instance ownership".into())
            })?
            .nodes
            .clone();
        let mut elements = BTreeMap::new();
        for (key, address) in keys {
            let entity = nodes[&address];
            let runtime_key = if scope.is_empty() {
                key.clone()
            } else {
                format!("{scope}/{key}")
            };
            if passive {
                world.entity_mut(entity).remove::<crate::ElementId>();
            } else {
                world
                    .entity_mut(entity)
                    .insert(crate::ElementId(runtime_key));
            }
            elements.insert(key, entity);
        }
        for entity in nodes.values().copied() {
            if passive {
                world.entity_mut(entity).remove::<crate::ScreenId>();
                world
                    .entity_mut(entity)
                    .insert((bevy::picking::Pickable::IGNORE, PassiveUi::default()));
            }
            let layout = world.get::<UiLayout>(entity).cloned();
            if let Some(layout) = layout {
                world.entity_mut(entity).insert(layout.node);
            }
            if let Some(classes) = world.get::<UiClasses>(entity).cloned() {
                world.entity_mut(entity).insert(
                    bevy_flair::style::components::ClassList::new_with_classes(classes.names),
                );
            }
            if let Some(sheet) = world.get::<UiStylesheet>(entity).cloned() {
                world
                    .entity_mut(entity)
                    .insert(bevy_flair::style::components::Styled::new(sheet.asset));
            }
            if let Some(caption) = world.get::<UiCaption>(entity).cloned() {
                let editable = if passive {
                    None
                } else {
                    world.get::<UiEditable>(entity).cloned()
                };
                world.entity_mut(entity).insert((
                    TextFont::default(),
                    TextLayout {
                        justify: caption.justify,
                        linebreak: if caption.wrap {
                            bevy::text::LineBreak::WordBoundary
                        } else {
                            bevy::text::LineBreak::NoWrap
                        },
                    },
                ));
                if let Some(editable) = editable {
                    world.entity_mut(entity).insert((
                        TextInput,
                        EditableText::new(caption.text),
                        TabIndex(0),
                        TextCursorStyle::default(),
                    ));
                    if editable.autofocus {
                        world.entity_mut(entity).insert(AutoFocus);
                    }
                    if editable.select_all {
                        world.entity_mut(entity).insert(SelectAllOnFocus);
                    }
                } else {
                    world.entity_mut(entity).insert(Text::new(caption.text));
                }
            }
            if let Some(image) = world.get::<UiImage>(entity).cloned() {
                let image_handle = select_image(world, &image, density)?;
                world.entity_mut(entity).insert(ImageNode {
                    image: image_handle,
                    color: image.color,
                    ..default()
                });
            }
            if !passive && world.get::<UiButton>(entity).is_some() {
                world.entity_mut(entity).insert((
                    Button,
                    TabIndex(0),
                    bevy::picking::hover::Hovered::default(),
                ));
            }
            if !passive && let Some(modal) = world.get::<UiModal>(entity).cloned() {
                match modal {
                    UiModal::Dialog => {
                        world.entity_mut(entity).insert((
                            ModalDialog,
                            TabGroup {
                                modal: true,
                                order: 1,
                            },
                        ));
                    }
                    UiModal::Barrier => {
                        world.entity_mut(entity).insert(ModalDialogBarrier);
                    }
                }
            }
        }
        world.entity_mut(root).insert(UiReady);
        Ok(Self { root, elements })
    }

    /// Resolve a required local binding; missing keys are errors, never ordinal guesses.
    pub fn element(&self, key: &str) -> kedra_scene::Result<Entity> {
        self.elements.get(key).copied().ok_or_else(|| {
            kedra_scene::Error(format!("required authored UI element is absent: {key}"))
        })
    }

    /// Inspect local bindings without traversing or rebuilding the rendered tree.
    pub fn elements(&self) -> &BTreeMap<String, Entity> {
        &self.elements
    }

    /// Resolve a text binding on the named node or its single immediate label.
    /// Ambiguous/missing labels are source contract errors, never guessed by order.
    pub fn text(&self, world: &World, key: &str) -> kedra_scene::Result<Entity> {
        let root = self.element(key)?;
        if world.get::<Text>(root).is_some() {
            return Ok(root);
        }
        let labels = world
            .get::<Children>(root)
            .into_iter()
            .flatten()
            .copied()
            .filter(|entity| world.get::<Text>(*entity).is_some())
            .collect::<Vec<_>>();
        if let [label] = labels.as_slice() {
            Ok(*label)
        } else {
            Err(kedra_scene::Error(format!(
                "authored control requires exactly one direct text label: {key}"
            )))
        }
    }
}

fn has_stylesheet(world: &World, mut entity: Option<Entity>, detached_ui: bool) -> bool {
    while let Some(current) = entity {
        // Passive layout roots detach at precisely this boundary. Validate the
        // resulting forest, without moving stylesheet ownership onto new roots.
        if detached_ui && world.get::<UiLayout>(current).is_none() {
            return false;
        }
        if world.get::<UiStylesheet>(current).is_some() {
            return true;
        }
        match world.get::<bevy_flair::style::components::Styled>(current) {
            Some(bevy_flair::style::components::Styled::StyleSheet(_)) => return true,
            Some(bevy_flair::style::components::Styled::Block) => return false,
            _ => {}
        }
        entity = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
    false
}

pub(super) fn select_image(
    world: &World,
    image: &UiImage,
    density: f32,
) -> kedra_scene::Result<Handle<Image>> {
    let variant = image
        .variants
        .iter()
        .find(|v| v.density >= density)
        .ok_or_else(|| {
            kedra_scene::Error(format!(
                "authored image has no variant for density {density}"
            ))
        })?;
    if !world
        .get_resource::<Assets<Image>>()
        .is_some_and(|images| images.contains(variant.image.id()))
    {
        return Err(kedra_scene::Error(
            "authored image dependency is not materialized".into(),
        ));
    }
    Ok(variant.image.clone())
}
