//! Native input/focus adapter. The model still owns selection and navigation.
use super::*;
use bevy::input_focus::{FocusedInput, InputFocus};
use kedra_ui::docking::rect;
pub(super) fn install(app: &mut App) {
    app.add_observer(activate)
        .add_observer(double_click)
        .add_observer(keyboard);
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
        commands.queue(move |world: &mut World| dispatch(world, binding));
    }
}
fn double_click(
    mut event: On<bevy::picking::events::PointerPress>,
    bindings: Query<&Binding>,
    mut commands: Commands,
) {
    if event.count != 2 || event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    if let Ok(binding) = bindings.get(event.entity)
        && let Action::Select(name) = &binding.action
    {
        let mut binding = binding.clone();
        binding.action = Action::Enter(name.clone());
        event.propagate(false);
        commands.queue(move |world: &mut World| dispatch(world, binding));
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
    let action = if key == KeyCode::F5 {
        Some(Action::Refresh)
    } else if command && key == KeyCode::KeyF {
        Some(Action::FocusSearch)
    } else if let Action::Select(name) = &binding.action {
        match key {
            KeyCode::Enter => Some(Action::Enter(name.clone())),
            KeyCode::ArrowLeft
            | KeyCode::ArrowRight
            | KeyCode::ArrowUp
            | KeyCode::ArrowDown
            | KeyCode::Home
            | KeyCode::End => Some(Action::Move(name.clone(), key)),
            KeyCode::Backspace => Some(Action::Up),
            _ => None,
        }
    } else if let Action::TreeNavigate(path) = &binding.action {
        matches!(
            key,
            KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Home
                | KeyCode::End
        )
        .then(|| Action::TreeKey(path.clone(), key))
    } else {
        None
    };
    if let Some(action) = action {
        let mut binding = binding.clone();
        binding.action = action;
        event.propagate(false);
        commands.queue(move |world: &mut World| dispatch(world, binding));
    }
}
impl Panel {
    pub(super) fn tree_key(
        &mut self,
        world: &mut World,
        path: PathBuf,
        key: KeyCode,
    ) -> Result<(), String> {
        let rows = self.tree.rows();
        let index = rows.iter().position(|(p, _)| p == &path).unwrap_or(0);
        if (key == KeyCode::ArrowRight && !self.tree.expanded(&path))
            || (key == KeyCode::ArrowLeft && self.tree.expanded(&path))
        {
            self.tree.toggle(path.clone());
            self.dirty = true;
            return self.focus_folder(world, &path);
        }
        let next = match key {
            KeyCode::Home => rows.first(),
            KeyCode::End => rows.last(),
            KeyCode::ArrowUp => rows.get(index.saturating_sub(1)),
            KeyCode::ArrowDown => rows.get((index + 1).min(rows.len().saturating_sub(1))),
            KeyCode::ArrowLeft => path
                .parent()
                .and_then(|parent| rows.iter().find(|(p, _)| p == parent)),
            _ => rows
                .get(index + 1)
                .filter(|(_, depth)| *depth > rows[index].1),
        };
        if let Some((path, _)) = next {
            self.focus_folder(world, path)?;
        }
        Ok(())
    }
    fn focus_folder(&self, world: &mut World, path: &std::path::Path) -> Result<(), String> {
        if let Some((_, row)) = self.folders.iter().find(|(p, _)| p == path) {
            let entity = row.element("navigate").map_err(|e| e.to_string())?;
            world
                .resource_mut::<InputFocus>()
                .set(entity, bevy::input_focus::FocusCause::Navigated);
            let area = self.element("folders")?;
            // Native rows already include the current scroll translation. Convert
            // their logical viewport bounds back into content-space coordinates.
            if let Some(viewport) = rect(world, area)
                && let Some(target) = rect(world, row.root)
                && let Some(node) = world.get::<ComputedNode>(area)
            {
                let height = viewport.height();
                let extent =
                    ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.);
                if height.is_finite()
                    && height > 0.
                    && extent.is_finite()
                    && target.min.y.is_finite()
                    && target.height() > 0.
                    && let Some(mut scroll) = world.get_mut::<ScrollPosition>(area)
                {
                    let top = target.min.y - viewport.min.y + scroll.y;
                    let bottom = top + target.height();
                    let next = if top < scroll.y || target.height() > height {
                        top
                    } else if bottom > scroll.y + height {
                        bottom - height
                    } else {
                        scroll.y
                    };
                    scroll.y = next.clamp(0., extent);
                }
            }
        }
        Ok(())
    }
    pub(super) fn move_selection(&mut self, world: &mut World, key: KeyCode) -> Result<(), String> {
        let names: Vec<_> = self
            .model
            .visible()
            .map(|entry| entry.name.clone())
            .collect();
        if names.is_empty() {
            return Ok(());
        }
        let entries = self.view.element("entries").map_err(|e| e.to_string())?;
        let columns = if self.model.mode == model::Mode::List {
            1
        } else {
            rect(world, entries).map_or(1, |r| ((r.width() - 16.) / 120.).floor().max(1.) as usize)
        };
        let current = names
            .iter()
            .position(|name| Some(name) == self.model.selected.as_ref())
            .unwrap_or(0);
        let next = match key {
            KeyCode::Home => 0,
            KeyCode::End => names.len() - 1,
            KeyCode::ArrowLeft => current.saturating_sub(1),
            KeyCode::ArrowRight => (current + 1).min(names.len() - 1),
            KeyCode::ArrowUp => current.saturating_sub(columns),
            KeyCode::ArrowDown => (current + columns).min(names.len() - 1),
            _ => current,
        };
        if self.model.select(&names[next]) {
            self.dirty = true;
            self.focus_selection = true;
            let height = if self.model.mode == model::Mode::List {
                22.
            } else {
                104.
            };
            let offset = 4. + (next / columns) as f32 * height;
            let area_height = rect(world, entries).map_or(104., |r| r.height());
            if let Some(mut scroll) = world.get_mut::<ScrollPosition>(entries) {
                if offset < scroll.y {
                    scroll.y = offset;
                } else if offset + height > scroll.y + area_height {
                    scroll.y = (offset + height - area_height).max(0.);
                }
            }
        }
        Ok(())
    }
}
