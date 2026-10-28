//! Stage replacement static model forests off the serving view. Old descendants
//! remain intact until all native cloning, ownership, budget and bounds checks pass.
use super::*;
use crate::scene_view::models::Instance;
#[cfg(test)]
mod tests;

pub(super) struct Staged {
    owner: Option<Entity>,
    items: Vec<(Entity, Model, Instance)>,
}
impl Staged {
    pub(super) fn cancel(self, world: &mut World) {
        if let Some(owner) = self.owner
            && let Ok(owner) = world.get_entity_mut(owner)
        {
            owner.despawn();
        }
    }
    pub(super) fn apply(self, world: &mut World, projection: &mut Projection) {
        for (entity, model, instance) in self.items {
            instance.attach(world, entity);
            if let Some(old) = projection.models.insert(entity, instance) {
                old.retire(world);
            }
            world.entity_mut(entity).insert(model);
        }
        if let Some(owner) = self.owner
            && let Ok(owner) = world.get_entity_mut(owner)
        {
            owner.despawn();
        }
    }
}
pub(super) fn prepare(
    world: &mut World,
    projection: &Projection,
    compiled: &CompiledPrefab,
    before: &ResolvedGraph,
) -> Result<Staged, String> {
    let mut changed = Vec::new();
    let mut count = projection.nodes.len();
    for (address, source) in compiled.source_entities() {
        let Some(model) = compiled.authoring_world().get::<Model>(*source) else {
            continue;
        };
        let entity = *projection
            .nodes
            .get(address)
            .ok_or("Projected model node unavailable")?;
        let instance = projection
            .models
            .get(&entity)
            .ok_or("Projected model ownership unavailable")?;
        if !instance.owned_by(world, entity) {
            return Err("Projected model descendants no longer belong to this view".into());
        }
        if before.nodes[address].components.get("scene.model")
            != compiled.graph().nodes[address]
                .components
                .get("scene.model")
        {
            changed.push((entity, model.clone()));
        } else {
            count = count
                .checked_add(instance.len())
                .ok_or("Model node budget exhausted")?;
        }
    }
    let mut staged = Staged {
        owner: None,
        items: Vec::new(),
    };
    if changed.is_empty() {
        return Ok(staged);
    }
    let owner = world.spawn((Transform::IDENTITY, Visibility::Hidden)).id();
    staged.owner = Some(owner);
    let result = (|| {
        for (entity, model) in changed {
            let remaining = 4096usize
                .checked_sub(count)
                .ok_or("Model node budget exhausted")?;
            let instance = crate::scene_view::models::mount(
                world,
                owner,
                &model.scene,
                projection.layer,
                remaining,
            )?;
            count += instance.len();
            staged.items.push((entity, model, instance));
        }
        crate::scene_view::bounds::measure(world, owner)?;
        Ok(())
    })();
    if let Err(error) = result {
        staged.cancel(world);
        return Err(error);
    }
    Ok(staged)
}
