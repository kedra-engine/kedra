//! Adapter-owned holds outlive receipts. Bevy's aggregate input resources cannot
//! represent independent holds of the same key/button in different windows, so
//! injected cross-window holds are rejected. Hardware messages are never owned.
use super::*;
use bevy::ecs::message::MessageCursor;
use std::collections::{HashMap, HashSet};

#[derive(Resource, Default)]
pub(super) struct HeldInputs {
    owner: Option<Entity>,
    mouse: bool,
    keys: Vec<KeyboardInput>,
    external_mouse: HashSet<(Entity, MouseButton)>,
    external_keys: HashMap<(Entity, KeyCode), Key>,
    mouse_cursor: MessageCursor<MouseButtonInput>,
    key_cursor: MessageCursor<KeyboardInput>,
    // A combined key-release message would release both aggregate identities.
    // When hardware shares only one identity, release the unshared one after
    // InputSystems instead, without sending a misleading combined event.
    release_codes: Vec<KeyCode>,
    release_keys: Vec<Key>,
}
impl HeldInputs {
    pub(super) fn validate_window(&self, window: Entity) -> Result<(), String> {
        if self.owner.is_some_and(|owner| owner != window) {
            Err("Release injected holds before targeting another window".into())
        } else {
            Ok(())
        }
    }
    pub(super) fn mouse_held(&self) -> bool {
        self.mouse
    }
    pub(super) fn key_held(&self, code: KeyCode) -> bool {
        self.keys.iter().any(|key| key.key_code == code)
    }
    pub(super) fn observe_external(&mut self, world: &World) {
        if let Some(messages) = world.get_resource::<Messages<MouseButtonInput>>() {
            for event in self.mouse_cursor.read(messages) {
                let key = (event.window, event.button);
                if event.state.is_pressed() {
                    self.external_mouse.insert(key);
                } else {
                    self.external_mouse.remove(&key);
                }
            }
        }
        if let Some(messages) = world.get_resource::<Messages<KeyboardInput>>() {
            for event in self.key_cursor.read(messages) {
                let key = (event.window, event.key_code);
                if event.state.is_pressed() {
                    self.external_keys.insert(key, event.logical_key.clone());
                } else {
                    self.external_keys.remove(&key);
                }
            }
        }
        self.external_mouse
            .retain(|(window, _)| world.get::<Window>(*window).is_some());
        self.external_keys
            .retain(|(window, _), _| world.get::<Window>(*window).is_some());
    }
    pub(super) fn skip_injected(&mut self, world: &World) {
        if let Some(messages) = world.get_resource::<Messages<MouseButtonInput>>() {
            self.mouse_cursor.clear(messages);
        }
        if let Some(messages) = world.get_resource::<Messages<KeyboardInput>>() {
            self.key_cursor.clear(messages);
        }
    }
    pub(super) fn retire(&mut self, world: &mut World) {
        if let Some(owner) = self
            .owner
            .filter(|owner| world.get::<Window>(*owner).is_none())
        {
            let _ = self.button(world, owner, false);
            for mut key in self.keys.clone() {
                key.state = ButtonState::Released;
                self.key(world, key);
            }
            self.owner = None;
        }
    }
    fn clear_owner(&mut self) {
        if !self.mouse && self.keys.is_empty() {
            self.owner = None;
        }
    }
    pub(super) fn button(
        &mut self,
        world: &mut World,
        window: Entity,
        down: bool,
    ) -> Result<(), String> {
        if down {
            self.validate_window(window)?;
            if self.mouse {
                return Ok(());
            }
            if self
                .external_mouse
                .iter()
                .any(|(_, button)| *button == MouseButton::Left)
                || world
                    .get_resource::<ButtonInput<MouseButton>>()
                    .is_some_and(|input| input.pressed(MouseButton::Left))
            {
                return Err("Left pointer button is already held outside the input adapter".into());
            }
            self.owner = Some(window);
            self.mouse = true;
        } else {
            if !self.mouse {
                return Ok(());
            }
            self.mouse = false;
            self.clear_owner();
            if self
                .external_mouse
                .iter()
                .any(|(_, button)| *button == MouseButton::Left)
            {
                return Ok(());
            }
        }
        let event = MouseButtonInput {
            button: MouseButton::Left,
            state: if down {
                ButtonState::Pressed
            } else {
                ButtonState::Released
            },
            window,
        };
        world.write_message(event);
        world.write_message(WindowEvent::MouseButtonInput(event));
        Ok(())
    }
    pub(super) fn validate_keys(
        &self,
        world: &World,
        events: &[KeyboardInput],
    ) -> Result<(), String> {
        for event in events.iter().filter(|event| event.state.is_pressed()) {
            self.validate_window(event.window)?;
            if !self.key_held(event.key_code)
                && (self
                    .external_keys
                    .iter()
                    .any(|((_, code), key)| *code == event.key_code || *key == event.logical_key)
                    || world
                        .get_resource::<ButtonInput<KeyCode>>()
                        .is_some_and(|input| input.pressed(event.key_code))
                    || world
                        .get_resource::<ButtonInput<Key>>()
                        .is_some_and(|input| input.pressed(event.logical_key.clone())))
            {
                return Err("Key is already held outside the input adapter".into());
            }
        }
        Ok(())
    }
    // Key batches are validated atomically before any press is published.
    pub(super) fn key(&mut self, world: &mut World, mut event: KeyboardInput) {
        if event.state.is_pressed() {
            if self.key_held(event.key_code) {
                return;
            }
            self.owner = Some(event.window);
            self.keys.push(event.clone());
        } else {
            let Some(index) = self
                .keys
                .iter()
                .position(|key| key.key_code == event.key_code)
            else {
                return;
            };
            let owned = self.keys.remove(index);
            self.clear_owner();
            event.logical_key = owned.logical_key;
            event.text = None;
            let physical_shared = self
                .external_keys
                .keys()
                .any(|(_, code)| *code == event.key_code);
            let logical_shared = self
                .external_keys
                .values()
                .any(|key| *key == event.logical_key);
            if physical_shared || logical_shared {
                if !physical_shared {
                    self.release_codes.push(event.key_code);
                }
                if !logical_shared {
                    self.release_keys.push(event.logical_key);
                }
                return;
            }
        }
        world.write_message(event.clone());
        world.write_message(WindowEvent::KeyboardInput(event));
    }
}

pub(super) fn release_unshared(
    mut held: ResMut<HeldInputs>,
    codes: Option<ResMut<ButtonInput<KeyCode>>>,
    keys: Option<ResMut<ButtonInput<Key>>>,
) {
    if let Some(mut input) = codes {
        for code in held.release_codes.drain(..) {
            input.release(code);
        }
    }
    if let Some(mut input) = keys {
        for key in held.release_keys.drain(..) {
            input.release(key);
        }
    }
}
