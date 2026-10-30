//! Borrowed spatial projection access for host-owned transform previews.
use super::*;
use kedra_scene::document::NodeAddress;

impl View {
    pub(crate) fn manipulation_target(
        &self,
        world: &World,
        address: &NodeAddress,
    ) -> Option<(Entity, Transform, Mat4, usize)> {
        self.navigation(world)?;
        let projection = self.projection.as_ref()?;
        let entity = *projection.nodes.get(address)?;
        if world.get::<Node>(entity).is_some() {
            return None;
        }
        let local = *world.get::<Transform>(entity)?;
        let parent = world
            .get::<ChildOf>(entity)
            .and_then(|parent| world.get::<GlobalTransform>(parent.parent()))
            .map_or(Mat4::IDENTITY, GlobalTransform::to_matrix);
        Some((entity, local, parent, projection.layer()))
    }
    pub(crate) fn manipulation_ray(&self, world: &World, point: Vec2) -> Result<Ray3d, String> {
        selection::ray(
            world,
            self.projection.as_ref().ok_or("Projection missing")?,
            point,
            self.area(world).ok_or("Viewport geometry missing")?,
        )
    }
}
