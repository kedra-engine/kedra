//! A read-only native UI border on the same offscreen canvas as its source.
//! Bevy owns layout, affine transforms, clipping and image fitting. Only this
//! disposable presentation node's computed values are replaced after layout.
use bevy::{camera::visibility::VisibilitySystems, picking::Pickable, prelude::*};

pub(super) const SELECTION_COLOR: Color = Color::srgb_u8(177, 210, 255);

#[derive(Component)]
struct UiMarker(Entity);

pub(super) fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        sync.after(bevy::ui::UiSystems::PostLayout)
            .after(bevy::ui::UiSystems::Stack)
            .after(VisibilitySystems::VisibilityPropagate),
    );
}

pub(super) fn spawn(world: &mut World, source: Entity) -> Option<Entity> {
    let camera = world.get::<UiTargetCamera>(source)?.0;
    Some(
        world
            .spawn((
                UiMarker(source),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BorderColor::all(SELECTION_COLOR),
                GlobalZIndex(i32::MAX),
                UiTargetCamera(camera),
                // The camera owns lifetime, not the passive source or its style tree.
                ChildOf(camera),
                Pickable::IGNORE,
                Visibility::Hidden,
            ))
            .id(),
    )
}

pub(super) fn sync(world: &mut World) {
    // Native stacking assigns ordinary nodes indices 0..len. Reserve the next
    // render index for our decorative overlay instead of competing with valid
    // authored global/local Z values or their change/history tie-breakers.
    // Leave UiStack itself untouched: input order belongs to native UI and this
    // overlay passes input through. Each view owns one marker on its own camera.
    let overlay_index = world
        .get_resource::<bevy::ui::UiStack>()
        .and_then(|stack| u32::try_from(stack.uinodes.len()).ok());
    let markers = world
        .query::<(Entity, &UiMarker)>()
        .iter(world)
        .map(|(entity, marker)| (entity, marker.0))
        .collect::<Vec<_>>();
    for (entity, source) in markers {
        if !world.entities().contains(source) {
            world.entity_mut(entity).despawn();
            continue;
        }
        let geometry = (|| {
            let stack_index = bevy::ui::ComputedStackIndex(overlay_index?);
            let target = *world.get::<ComputedUiTargetCamera>(source)?;
            let camera = target.get()?;
            if !world.get::<Camera>(camera)?.is_active
                || !world.get::<InheritedVisibility>(source)?.get()
            {
                return None;
            }
            let node = *world.get::<ComputedNode>(source)?;
            let transform = *world.get::<UiGlobalTransform>(source)?;
            if node.is_empty()
                || !node.size.is_finite()
                || !transform.affine().is_finite()
                || transform.matrix2.determinant() == 0.
            {
                return None;
            }
            let info = *world.get::<ComputedUiRenderTargetInfo>(source)?;
            let clip = world.get::<bevy::ui::CalculatedClip>(source).cloned();
            Some((node, transform, target, info, clip, stack_index))
        })();
        let Some((source_node, transform, target, info, clip, stack_index)) = geometry else {
            world
                .entity_mut(entity)
                .insert((Visibility::Hidden, InheritedVisibility::HIDDEN));
            continue;
        };
        let mut node = ComputedNode {
            size: source_node.size,
            unrounded_size: source_node.unrounded_size,
            inverse_scale_factor: source_node.inverse_scale_factor,
            ..default()
        };
        // Inside border preserves full-canvas/root bounds without clipping it off.
        node.border = bevy::sprite::BorderRect::all(2.);
        let mut overlay = world.entity_mut(entity);
        overlay.insert((
            node,
            transform,
            target,
            info,
            stack_index,
            Visibility::Inherited,
            InheritedVisibility::VISIBLE,
        ));
        if let Some(clip) = clip {
            overlay.insert(clip);
        } else {
            overlay.remove::<bevy::ui::CalculatedClip>();
        }
    }
}
