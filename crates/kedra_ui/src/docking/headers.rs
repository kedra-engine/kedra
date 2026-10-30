//! External header destinations are part of projection publication, never caller surgery.
use super::*;
use std::collections::BTreeSet;

/// Consumer-owned slots for one group's existing tab rail and common menu.
/// They must remain live through sync, and outside this projection's owned chrome.
#[derive(Clone, Copy, Debug)]
pub struct HeaderDestination {
    /// Laid-out client slot for the retained tab rail, excluding native-drag space.
    pub row: Entity,
    /// Laid-out client slot for the retained common menu button.
    pub menu: Entity,
}
impl Projection {
    /// Prepare external destinations for the next successful sync. This does not
    /// move anything; invalid/retired/duplicate/cyclic targets preserve old policy.
    /// An empty map restores normal authored group headers on the next sync.
    pub fn set_header_destinations(
        &mut self,
        world: &mut World,
        destinations: BTreeMap<NodeId, HeaderDestination>,
    ) -> Result<(), String> {
        self.check_destinations(world, &destinations)?;
        self.header_destinations = destinations;
        Ok(())
    }
    /// The retained common menu control; identity survives header reattachment.
    pub fn menu(&self, group: NodeId) -> Option<Entity> {
        match self.nodes.get(&group) {
            Some(Chrome::Group { menu, .. }) => Some(*menu),
            _ => None,
        }
    }
    /// Current configured header policy, for native measurement and hit testing.
    pub fn header_destination(&self, group: NodeId) -> Option<HeaderDestination> {
        self.header_destinations.get(&group).copied()
    }
    fn check_destinations(
        &self,
        world: &World,
        destinations: &BTreeMap<NodeId, HeaderDestination>,
    ) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for destination in destinations.values() {
            for target in [destination.row, destination.menu] {
                if world.get::<bevy::ui::Node>(target).is_none() || !seen.insert(target)
                    || self.nodes.values().any(|chrome| crate::window_chrome::under(world, target, chrome.root()))
                    || self.nodes.values().any(|chrome| matches!(chrome, Chrome::Group { tabs, menu, .. } if crate::window_chrome::under(world, target, *tabs) || crate::window_chrome::under(world, target, *menu)))
                    || self.tabs.values().any(|tab| crate::window_chrome::under(world, target, tab.view.root)) {
                    return Err("Dock header destination is retired, duplicated or owned by the projection".into());
                }
            }
        }
        Ok(())
    }
    pub(super) fn validate_headers(
        &self,
        world: &World,
        state: &Snapshot,
        hosts: &BTreeMap<HostId, Entity>,
    ) -> Result<(), String> {
        self.check_destinations(world, &self.header_destinations)?;
        for (group, destination) in &self.header_destinations {
            if !matches!(state.nodes.get(group), Some(Node::Group { .. })) {
                return Err("Dock header destination has no group".into());
            }
            let host = state
                .host_of(*group)
                .ok_or("Dock header group has no host")?;
            let window = crate::native_input::owner(world, hosts[&host]);
            if [destination.row, destination.menu]
                .into_iter()
                .any(|e| crate::native_input::owner(world, e) != window)
            {
                return Err("Dock header destination belongs to another window".into());
            }
        }
        Ok(())
    }
    pub(super) fn restore_headers(&self, world: &mut World, state: &Snapshot) {
        // First restore borrowed chrome to its own group. It can now be retired
        // safely even when an obsolete external native host is about to disappear.
        for (id, chrome) in &self.nodes {
            if state.nodes.contains_key(id) && self.header_destinations.contains_key(id) {
                continue;
            }
            if let Chrome::Group {
                tabs,
                menu,
                tabs_parent,
                menu_parent,
                ..
            } = chrome
            {
                if world
                    .get::<Children>(*tabs_parent)
                    .and_then(|children| children.first())
                    != Some(tabs)
                {
                    world.entity_mut(*tabs_parent).insert_children(0, &[*tabs]);
                }
                reparent(world, *menu, *menu_parent);
            }
        }
    }
}
