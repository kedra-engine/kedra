//! Bevy adapters only. Feature rules and drafts are ordinary Rust in `domain`.
use crate::domain::{EntryDraft, StartupModel};
pub(crate) use crate::domain::{EntryMode, Intent, Phase};
use bevy::prelude::*;

/// Bevy schedule projection of the application's ordinary Rust phase.
#[derive(States, Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppPhase {
    /// Windowless delivery verification and editor source preparation.
    #[default]
    Boot,
    /// Project selection and entry forms.
    Projects,
    /// Project validation or actionable failure.
    Loading,
    /// Validated dockable project workspace, without scene-editing tools.
    Editor,
}

#[derive(Resource, Deref, DerefMut)]
pub(crate) struct ProjectManager {
    #[deref]
    pub model: StartupModel,
    // Retained view ownership, never persisted or used by domain operations.
    pub list_dirty: bool,
    pub modal: Option<Entity>,
    pub native_created_at: Option<u32>,
    pub loading_started: Option<std::time::Duration>,
}
impl Default for ProjectManager {
    fn default() -> Self {
        let history_path = directories::ProjectDirs::from("org", "Kedra", "Kedra")
            .map(|dirs| dirs.data_local_dir().join("recent-projects.toml"));
        let mut model = StartupModel::default();
        model.history_writable = history_path.is_some();
        model.history_path = history_path;
        Self {
            model,
            list_dirty: true,
            modal: None,
            native_created_at: None,
            loading_started: None,
        }
    }
}

#[derive(Resource, Deref, DerefMut)]
pub(crate) struct EntryForm {
    #[deref]
    pub draft: EntryDraft,
    pub restore_focus: Option<Entity>,
}
impl Default for EntryForm {
    fn default() -> Self {
        let directory = directories::UserDirs::new()
            .map(|dirs| dirs.home_dir().to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            draft: EntryDraft {
                name: "New project".into(),
                directory,
                ..Default::default()
            },
            restore_focus: None,
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    Name,
    Directory,
    Search,
}
#[derive(Component, Clone, Copy)]
pub(crate) enum StatusLabel {
    Notice,
    EntryError,
    Destination,
    Loading,
}
#[derive(Component, Clone)]
pub(crate) enum Action {
    New,
    Open,
    Submit,
    Cancel,
    Browse,
    Select(std::path::PathBuf),
    OpenRecent(std::path::PathBuf),
    Locate(std::path::PathBuf),
    Back,
    Retry,
    Projects,
    Search,
    LoadingLog,
}
#[derive(Component, Clone, Default)]
pub(crate) struct FailureActions;

#[derive(Component, Clone, Default)]
pub(crate) struct RetryAction;

pub(crate) fn entry_intent(form: &EntryDraft) -> Result<Intent, String> {
    let dirs = directories::UserDirs::new();
    crate::domain::entry_intent(form, dirs.as_ref().map(|dirs| dirs.home_dir()))
}

pub(crate) fn project_phase(
    manager: Res<ProjectManager>,
    state: Res<State<AppPhase>>,
    mut next: ResMut<NextState<AppPhase>>,
    boot: Res<crate::boot::Boot>,
    pending: Option<Res<crate::jobs::PendingJob>>,
) {
    if !boot.complete {
        return;
    }
    // Resolve initial history/CLI intent before choosing the first real window.
    if *state.get() == AppPhase::Boot && pending.is_some() {
        return;
    }
    let phase = match manager.phase() {
        Phase::Projects => AppPhase::Projects,
        Phase::Loading => AppPhase::Loading,
        Phase::Editor => AppPhase::Editor,
    };
    if *state.get() != phase {
        next.set(phase);
    }
}
