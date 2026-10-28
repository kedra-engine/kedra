use super::*;
use bevy::input_focus::FocusedInput;
pub(super) fn install(app: &mut App) {
    app.add_observer(activate).add_observer(keyboard);
}
fn activate(
    event: On<bevy::ui_widgets::Activate>,
    bindings: Query<&Binding>,
    mut commands: Commands,
) {
    if let Ok(binding) = bindings.get(event.entity)
        && !matches!(binding.action, Action::FocusSearch)
    {
        let binding = binding.clone();
        commands.queue(move |world: &mut World| crate::workspace::hierarchy_action(world, binding));
    }
}
fn keyboard(
    mut event: On<FocusedInput<bevy::input::keyboard::KeyboardInput>>,
    bindings: Query<&Binding>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if !event.input.state.is_pressed() {
        return;
    }
    let Ok(binding) = bindings.get(event.event_target()) else {
        return;
    };
    let key = event.input.key_code;
    let command = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    let action = if command && key == KeyCode::KeyF {
        Some(Action::FocusSearch)
    } else if key == KeyCode::Escape && matches!(binding.action, Action::FocusSearch) {
        Some(Action::CloseSearch)
    } else if let Action::Select(node) | Action::Toggle(node) = &binding.action {
        matches!(
            key,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Home
                | KeyCode::End
        )
        .then(|| Action::Key(node.clone(), key))
    } else {
        None
    };
    if let Some(action) = action {
        let mut binding = binding.clone();
        binding.action = action;
        event.propagate(false);
        commands.queue(move |world: &mut World| crate::workspace::hierarchy_action(world, binding));
    }
}
impl Panel {
    pub fn dispatch(
        &mut self,
        world: &mut World,
        binding: Binding,
        documents: &mut Documents,
    ) -> Result<(), String> {
        // Deferred row input must never select a node in a different document.
        if binding.document != documents.active || binding.document != self.document {
            return Ok(());
        }
        let area = self.element("entries")?;
        if matches!(binding.action, Action::FocusSearch) {
            self.search_open = true;
            self.dirty = true;
            kedra_ui::docking::class(world, self.element("toolbar")?, "hidden", false);
            world.resource_mut::<bevy::input_focus::InputFocus>().set(
                self.element("search")?,
                bevy::input_focus::FocusCause::Navigated,
            );
            return Ok(());
        }
        if matches!(binding.action, Action::Details) {
            self.details_open = !self.details_open;
            self.dirty = true;
            return Ok(());
        }
        if matches!(binding.action, Action::CloseSearch) {
            self.search_open = false;
            self.search(world, "")?;
            self.focus = self.selected.clone();
            world
                .resource_mut::<bevy::input_focus::InputFocus>()
                .set(self.view.root, bevy::input_focus::FocusCause::Navigated);
            self.dirty = true;
            return Ok(());
        }
        let Some(document) = self.document else {
            return Ok(());
        };
        let Some(tree) = &self.tree else {
            return Ok(());
        };
        let navigation = self
            .navigation
            .get_mut(&document)
            .ok_or("Hierarchy navigation missing")?;
        let pointer_selection = matches!(binding.action, Action::Select(_));
        let target = match binding.action {
            Action::Select(node) => Some(node),
            Action::Toggle(node) => {
                if !navigation.collapsed.remove(&node) {
                    navigation.collapsed.insert(node);
                }
                None
            }
            Action::Reveal => {
                if let Some(node) = self.selected.clone() {
                    tree.reveal(navigation, &node);
                    self.search(world, "")?;
                    Some(node)
                } else {
                    None
                }
            }
            Action::Key(node, key) => {
                let visible = tree.visible(navigation);
                let Some(current) = visible
                    .iter()
                    .position(|(i, _)| tree.rows[*i].address == node)
                else {
                    return Ok(());
                };
                let index = visible[current].0;
                let row = &tree.rows[index];
                let next = match key {
                    KeyCode::Home => Some(visible[0].0),
                    KeyCode::End => visible.last().map(|(i, _)| *i),
                    KeyCode::ArrowUp => Some(visible[current.saturating_sub(1)].0),
                    KeyCode::ArrowDown => Some(visible[(current + 1).min(visible.len() - 1)].0),
                    KeyCode::ArrowLeft => {
                        if row.children
                            && !navigation.collapsed.contains(&node)
                            && navigation.filter.is_empty()
                        {
                            navigation.collapsed.insert(node.clone());
                            Some(index)
                        } else {
                            row.parent
                        }
                    }
                    KeyCode::ArrowRight => {
                        if navigation.collapsed.remove(&node) {
                            Some(index)
                        } else {
                            visible
                                .get(current + 1)
                                .filter(|(i, _)| tree.rows[*i].parent == Some(index))
                                .map(|(i, _)| *i)
                        }
                    }
                    _ => None,
                };
                next.map(|i| tree.rows[i].address.clone())
            }
            Action::FocusSearch | Action::Details | Action::CloseSearch => None,
        };
        if let Some(node) = target {
            let keys = world.get_resource::<ButtonInput<KeyCode>>();
            let additive = pointer_selection
                && keys.is_some_and(|keys| {
                    [
                        KeyCode::SuperLeft,
                        KeyCode::SuperRight,
                        KeyCode::ControlLeft,
                        KeyCode::ControlRight,
                    ]
                    .iter()
                    .any(|key| keys.pressed(*key))
                });
            let range = pointer_selection
                && keys.is_some_and(|keys| {
                    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
                });
            let accepted = if range {
                let tree = self.tree.as_ref().ok_or("Hierarchy graph missing")?;
                let visible = tree.visible(&self.navigation[&document]);
                let anchor = self
                    .range_anchor
                    .as_ref()
                    .or(self.selected.as_ref())
                    .unwrap_or(&node);
                let start = visible
                    .iter()
                    .position(|(index, _)| tree.rows[*index].address == *anchor);
                let end = visible
                    .iter()
                    .position(|(index, _)| tree.rows[*index].address == node);
                match (start, end) {
                    (Some(start), Some(end)) => {
                        let mut nodes: Vec<_> = visible[start.min(end)..=start.max(end)]
                            .iter()
                            .map(|(index, _)| tree.rows[*index].address.clone())
                            .collect();
                        if end < start {
                            nodes.reverse();
                        }
                        documents.select_nodes(document, nodes)
                    }
                    _ => false,
                }
            } else if additive {
                documents.toggle_node(document, node.clone())
            } else {
                documents.select(document, node.clone())
            };
            if !accepted {
                return Ok(());
            }
            if !range {
                self.range_anchor = Some(node.clone());
            } else if self.range_anchor.is_none() {
                self.range_anchor = self.selected.clone();
            }
            if pointer_selection {
                if let Some(view) = self.rows.get(&node) {
                    world.resource_mut::<bevy::input_focus::InputFocus>().set(
                        view.element("select").map_err(|e| e.to_string())?,
                        bevy::input_focus::FocusCause::Pressed,
                    );
                }
            } else {
                self.focus = Some(node.clone());
            }
            let visible = self
                .tree
                .as_ref()
                .ok_or("Hierarchy graph missing")?
                .visible(&self.navigation[&document]);
            if let Some(position) = visible.iter().position(|(i, _)| {
                self.tree
                    .as_ref()
                    .is_some_and(|t| t.rows[*i].address == node)
            }) {
                let top = 4. + position as f32 * 22.;
                let height = kedra_ui::docking::rect(world, area).map_or(260., |r| r.height());
                if let Some(mut scroll) = world.get_mut::<ScrollPosition>(area) {
                    if position == 0 {
                        scroll.y = 0.;
                    } else if top < scroll.y {
                        scroll.y = top;
                    } else if top + 22. > scroll.y + height {
                        scroll.y = (top + 22. - height).max(0.);
                    }
                }
            }
        }
        self.dirty = true;
        Ok(())
    }
}
