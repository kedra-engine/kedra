//! Publish only changed docking edges; unchanged branches keep Bevy/CSS ticks.
use super::*;

pub(super) fn reparent(world: &mut World, entity: Entity, parent: Entity) {
    if world.get::<ChildOf>(entity).map(ChildOf::parent) != Some(parent) {
        world.entity_mut(entity).insert(ChildOf(parent));
    }
}

pub(super) fn order_children(world: &mut World, entity: Entity, order: &[Entity]) {
    let unchanged = world
        .get::<Children>(entity)
        .map_or(order.is_empty(), |children| &children[..] == order);
    if !unchanged {
        world.entity_mut(entity).replace_children(order);
    }
}

impl Projection {
    pub(super) fn park_transfers(
        &self,
        world: &mut World,
        state: &Snapshot,
        hosts: &BTreeMap<HostId, Entity>,
        stash: Entity,
    ) {
        // This is one publication's edge plan, not a retained invalidation cache.
        // Detach every changing edge before attachment: a surviving descendant
        // can become an ancestor, and stale chrome must never retire live content.
        let mut destinations = BTreeMap::new();
        for (id, host) in &state.hosts {
            if let Some(root) = host.root {
                destinations.insert(self.nodes[&root].root(), hosts[id]);
            }
        }
        for (id, node) in &state.nodes {
            match node {
                Node::Split { first, second, .. } => {
                    let parent = self.nodes[id].root();
                    for child in [first, second] {
                        destinations.insert(self.nodes[child].root(), parent);
                    }
                }
                Node::Group {
                    tabs: order,
                    active,
                } => {
                    let Chrome::Group { tabs, body, .. } = &self.nodes[id] else {
                        unreachable!("Chrome kinds were checked before publication")
                    };
                    for view in order {
                        destinations.insert(self.tabs[view].view.root, *tabs);
                    }
                    if let Some(content) = self.content.get(active) {
                        destinations.insert(*content, *body);
                    }
                }
            }
        }
        for entity in self
            .content
            .values()
            .copied()
            .chain(self.tabs.values().map(|tab| tab.view.root))
            .chain(self.nodes.values().map(Chrome::root))
        {
            let destination = destinations.get(&entity).copied().unwrap_or(stash);
            if world.get::<ChildOf>(entity).map(ChildOf::parent) != Some(destination) {
                reparent(world, entity, stash);
            }
        }
    }
}
