//! Validated borrowed-content attachments; no feature reconstruction or ownership transfer.
use super::{BoundHeader, HeaderSlot, under};
use crate::native_input::owner;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// One active header binding to a live window and a consumer-owned authored tree.
///
/// Content is borrowed until detached. The caller must keep the window, header,
/// slots and parking parents live and call `unbind` before destroying the header.
/// Every rejection preserves attachments. No consumer-owned entities are despawned.
pub struct Header {
    /// Explicit target window; unrelated windows cannot participate in this binding.
    pub window: Entity,
    /// Authored header root. The caller owns its lifecycle.
    pub root: Entity,
    slots: BTreeMap<String, Entity>,
    attached: BTreeMap<String, (Entity, Entity)>,
    binding: Option<Entity>,
}
impl Header {
    /// Set a retained full caption and fit it using Bevy's Unicode shaping.
    /// `title` and `bounds` must be live descendants of this binding.
    pub fn set_title(
        &self,
        world: &mut World,
        title: Entity,
        bounds: Entity,
        source: &str,
    ) -> Result<(), String> {
        self.live(world)?;
        super::set_title(world, self.root, title, bounds, source)
    }
    /// Borrow an ephemeral authored destination; it remains owned by this header.
    pub fn slot(&self, name: &str) -> Option<Entity> {
        self.binding.and_then(|_| self.slots.get(name).copied())
    }
    /// Bind distinct named descendants. Reject retired/mismatched owners, duplicate
    /// windows, already-bound slots and empty names without mutating the world.
    pub fn bind(
        world: &mut World,
        window: Entity,
        root: Entity,
        slots: BTreeMap<String, Entity>,
    ) -> Result<Self, String> {
        if world.get::<Window>(window).is_none()
            || world.get::<Node>(root).is_none()
            || owner(world, root) != Some(window)
        {
            return Err("Header requires a live root targeting its window".into());
        }
        if world
            .iter_entities()
            .any(|e| e.get::<BoundHeader>().is_some_and(|b| b.window == window))
        {
            return Err("Window already has a bound header".into());
        }
        let mut seen = BTreeSet::new();
        for (name, slot) in &slots {
            if name.is_empty()
                || !seen.insert(*slot)
                || *slot == root
                || !under(world, *slot, root)
                || world.get::<HeaderSlot>(*slot).is_some()
                || world.get::<Node>(*slot).is_none()
                || owner(world, *slot) != Some(window)
            {
                return Err("Header slots must be distinct unbound named UI descendants".into());
            }
        }
        // An ECS identity distinguishes successive bindings of the same authored
        // root without a global ID allocator or persisted native handles.
        let generation = world.spawn(ChildOf(root)).id();
        world
            .entity_mut(root)
            .insert(BoundHeader { window, generation });
        for slot in slots.values() {
            world.entity_mut(*slot).insert(HeaderSlot { header: root });
        }
        Ok(Self {
            window,
            root,
            slots,
            attached: BTreeMap::new(),
            binding: Some(generation),
        })
    }

    fn live(&self, world: &World) -> Result<(), String> {
        if owner(world, self.root) != Some(self.window)
            || !world.get::<BoundHeader>(self.root).is_some_and(|b| {
                b.window == self.window
                    && Some(b.generation) == self.binding
                    && world.entities().contains(b.generation)
            })
        {
            return Err("Header binding is stale".into());
        }
        Ok(())
    }

    /// Attach a borrowed live root to an empty slot; `parking` must be outside this
    /// header, in the same window, and remain live until detach. Cycles, foreign
    /// content, duplicate attachments and retired bindings are rejected atomically.
    pub fn attach(
        &mut self,
        world: &mut World,
        slot: &str,
        content: Entity,
        parking: Entity,
    ) -> Result<(), String> {
        self.live(world)?;
        let target = *self.slots.get(slot).ok_or("Unknown header slot")?;
        if self.attached.contains_key(slot)
            || self.attached.values().any(|(e, _)| *e == content)
            || !under(world, target, self.root)
            || owner(world, target) != Some(self.window)
            || !world
                .get::<HeaderSlot>(target)
                .is_some_and(|s| s.header == self.root)
            || under(world, target, content)
            || under(world, content, self.root)
            || under(world, parking, self.root)
            || under(world, parking, content)
            || content == parking
            || owner(world, content) != Some(self.window)
            || owner(world, parking) != Some(self.window)
            || world.get::<Node>(content).is_none()
        {
            return Err(
                "Header attachment has occupied, foreign, stale or cyclic ownership".into(),
            );
        }
        world.entity_mut(content).insert(ChildOf(target));
        self.attached.insert(slot.into(), (content, parking));
        Ok(())
    }

    fn detachable(&self, world: &World, content: Entity, parking: Entity) -> Result<(), String> {
        if owner(world, content) != Some(self.window)
            || !under(world, content, self.root)
            || owner(world, parking) != Some(self.window)
            || under(world, parking, self.root)
            || under(world, parking, content)
        {
            return Err("Header attachment lost its live parking owner".into());
        }
        Ok(())
    }

    /// Return a borrowed root to its explicit parking parent. An empty valid slot
    /// returns `None`. A stale binding or parking owner returns an error unchanged.
    pub fn detach(&mut self, world: &mut World, slot: &str) -> Result<Option<Entity>, String> {
        self.live(world)?;
        if !self.slots.contains_key(slot) {
            return Err("Unknown header slot".into());
        }
        let Some((content, parking)) = self.attached.get(slot).copied() else {
            return Ok(None);
        };
        self.detachable(world, content, parking)?;
        world.entity_mut(content).insert(ChildOf(parking));
        self.attached.remove(slot);
        Ok(Some(content))
    }

    /// Park all borrowed content and release the binding, never destroying content.
    /// Validate every parking destination first; on error no attachment is moved.
    pub fn unbind(&mut self, world: &mut World) -> Result<(), String> {
        self.live(world)?;
        for (content, parking) in self.attached.values() {
            self.detachable(world, *content, *parking)?;
        }
        for (content, parking) in self.attached.values() {
            world.entity_mut(*content).insert(ChildOf(*parking));
        }
        self.attached.clear();
        for slot in self.slots.values() {
            if world
                .get::<HeaderSlot>(*slot)
                .is_some_and(|s| s.header == self.root)
                && let Ok(mut entity) = world.get_entity_mut(*slot)
            {
                entity.remove::<HeaderSlot>();
            }
        }
        world.entity_mut(self.root).remove::<BoundHeader>();
        if let Some(generation) = self.binding.take() {
            world.despawn(generation);
        }
        Ok(())
    }
}
