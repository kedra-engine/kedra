//! Authored chrome handles and derived visual states, never document authority.
use bevy::prelude::*;
use kedra_ui::authored::view::UiView;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Controls {
    pub(super) buttons: Vec<Entity>,
    pub(super) nodes: BTreeMap<&'static str, Entity>,
    orientation: Option<super::orientation::Widget>,
}
impl Controls {
    pub(super) fn mount(world: &mut World, view: &UiView) -> Result<Self, String> {
        let names = [
            "hand",
            "move",
            "rotate",
            "scale",
            "snap",
            "settings",
            "orbit",
            "frame",
            "projection",
        ];
        let mut nodes = BTreeMap::new();
        for name in names.into_iter().chain(["navigation", "transform-tools"]) {
            nodes.insert(name, view.element(name).map_err(|error| error.to_string())?);
        }
        let mut buttons = names.map(|name| nodes[name]).to_vec();
        for name in ["projection-label"] {
            if let Ok(entity) = view.element(name) {
                nodes.insert(name, entity);
            }
        }
        for axis in super::orientation::Axis::ALL {
            let entity = view.element(axis.name()).map_err(|e| e.to_string())?;
            nodes.insert(axis.name(), entity);
            buttons.push(entity);
        }
        let orientation = Some(super::orientation::Widget::mount(world, view)?);
        Ok(Self {
            buttons,
            nodes,
            orientation,
        })
    }
    pub(super) fn entity(&self, name: &str) -> Option<Entity> {
        self.nodes.get(name).copied()
    }
    pub(super) fn enabled(&self, world: &mut World, enabled: bool) {
        for button in &self.buttons {
            if enabled {
                world
                    .entity_mut(*button)
                    .remove::<bevy::ui::InteractionDisabled>();
            } else {
                world
                    .entity_mut(*button)
                    .insert(bevy::ui::InteractionDisabled);
            }
        }
    }
    pub(super) fn selected(&self, world: &mut World, name: &str, selected: bool) {
        if let Some(entity) = self.entity(name) {
            kedra_ui::docking::class(world, entity, "selected", selected);
        }
    }
    pub(super) fn spatial_enabled(&self, world: &mut World, enabled: bool) {
        for (name, entity) in &self.nodes {
            if ![
                "settings",
                "navigation",
                "transform-tools",
                "projection-label",
            ]
            .contains(name)
            {
                if enabled {
                    world
                        .entity_mut(*entity)
                        .remove::<bevy::ui::InteractionDisabled>();
                } else {
                    world
                        .entity_mut(*entity)
                        .insert(bevy::ui::InteractionDisabled);
                }
            }
        }
    }
    pub(super) fn orientation(&self, world: &mut World, camera: Entity) {
        if let Some(rotation) = world.get::<Transform>(camera).map(|pose| pose.rotation)
            && let Some(widget) = &self.orientation
        {
            widget.update(world, rotation);
        }
    }
    pub(super) fn layout(&self, world: &mut World, _root: Entity, viewport: Entity, spatial: bool) {
        let size = kedra_ui::docking::rect(world, viewport).map_or(Vec2::ZERO, |rect| rect.size());
        for (name, hidden) in [
            ("navigation", size.x < 46. || size.y < 38.),
            (
                "transform-tools",
                !spatial || size.x < 190. || size.y < 120.,
            ),
            ("frame", !spatial || size.x < 190. || size.y < 120.),
            ("orbit", !spatial || size.x < 160. || size.y < 282.),
            ("projection", !spatial || size.x < 160. || size.y < 282.),
        ] {
            if let Some(entity) = self.entity(name) {
                kedra_ui::docking::class(world, entity, "hidden", hidden);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn narrow_and_short_layouts_hide_unreachable_tools_and_restore_without_rebinding()
    -> Result<(), String> {
        let mut world = World::new();
        let root = world
            .spawn((
                Node::default(),
                bevy_flair::style::components::ClassList::default(),
            ))
            .id();
        let viewport = world
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::new(480., 290.),
                    inverse_scale_factor: 1.,
                    ..default()
                },
                UiGlobalTransform::default(),
            ))
            .id();
        let nav = world
            .spawn(bevy_flair::style::components::ClassList::default())
            .id();
        let orbit = world
            .spawn(bevy_flair::style::components::ClassList::default())
            .id();
        let projection = world
            .spawn(bevy_flair::style::components::ClassList::default())
            .id();
        let controls = Controls {
            nodes: BTreeMap::from([
                ("navigation", nav),
                ("orbit", orbit),
                ("projection", projection),
                (
                    "transform-tools",
                    world
                        .spawn(bevy_flair::style::components::ClassList::default())
                        .id(),
                ),
            ]),
            ..default()
        };
        controls.layout(&mut world, root, viewport, true);
        let hidden = |world: &World, entity| {
            world
                .get::<bevy_flair::style::components::ClassList>(entity)
                .is_some_and(|list| list.contains("hidden"))
        };
        assert!(!hidden(&world, nav));
        world
            .get_mut::<ComputedNode>(viewport)
            .ok_or("Viewport")?
            .size = Vec2::new(240., 180.);
        controls.layout(&mut world, root, viewport, true);
        assert!(!hidden(&world, nav));
        assert!(hidden(&world, orbit));
        assert!(hidden(&world, projection));
        world
            .get_mut::<ComputedNode>(viewport)
            .ok_or("Viewport")?
            .size = Vec2::new(240., 100.);
        controls.layout(&mut world, root, viewport, true);
        assert!(!hidden(&world, nav));
        assert!(hidden(&world, controls.nodes["transform-tools"]));
        world
            .get_mut::<ComputedNode>(viewport)
            .ok_or("Viewport")?
            .size = Vec2::new(1000., 550.);
        controls.layout(&mut world, root, viewport, true);
        assert!(!hidden(&world, nav));
        assert!(!hidden(&world, orbit));
        assert!(!hidden(&world, projection));
        controls.layout(&mut world, root, viewport, false);
        assert!(
            !hidden(&world, nav),
            "Settings remains available on nonspatial views"
        );
        assert!(hidden(&world, orbit));
        assert!(hidden(&world, projection));
        assert!(hidden(&world, controls.nodes["transform-tools"]));
        Ok(())
    }
}
