//! Feature-owned hint text; shared kedra_ui owns all tooltip presentation.
use super::*;
#[cfg(test)]
thread_local! {
    pub(super) static GENERATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
impl Panel {
    pub(super) fn hint(&self, action: &Action) -> String {
        #[cfg(test)]
        GENERATIONS.with(|count| count.set(count.get() + 1));
        match action {
            Action::Select(name) | Action::Enter(name) => self
                .model
                .entries
                .iter()
                .find(|e| &e.name == name)
                .map(|entry| entry.name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Action::Navigate(path) | Action::TreeNavigate(path) | Action::Toggle(path) => format!(
                "{}{}",
                path.file_name()
                    .map_or_else(|| "Assets".into(), |name| name.to_string_lossy()),
                self.tree
                    .error(path)
                    .map_or_else(String::new, |error| format!("\n{error}"))
            ),
            Action::Back => "Back to the previous source folder".into(),
            Action::Forward => "Forward to the next source folder".into(),
            Action::Up | Action::Root => "Navigate within the project source root".into(),
            Action::Grid => "Grid view".into(),
            Action::List => "List view".into(),
            Action::Documents => "Filter: scene/prefab documents and folders".into(),
            Action::Clear => "Clear this folder's search".into(),
            _ => String::new(),
        }
    }
}
