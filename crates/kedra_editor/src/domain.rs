//! Ordinary Rust application model. No Bevy, entities, windows or renderer.
//!
//! The host executes filesystem effects; completions carry a generation and
//! cannot publish into a later session. UI trees are a projection, not authority.
use kedra_project::{Project, recent::RecentProjects};
use std::path::PathBuf;

#[derive(Default)]
enum Preparation {
    #[default]
    Idle,
    Working,
    Ready,
    Failed(String),
}

/// Application phase, independent of Bevy's scheduling state.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// No project is open.
    #[default]
    Projects,
    /// One preparation request owns the loading surface.
    Loading,
    /// Empty editor host, optionally with a validated project.
    Editor,
}

/// Project preparation intent; never contains an ECS handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Read-only open of an existing project root.
    Open(PathBuf),
    /// Create a new child directory without overwriting existing content.
    Create {
        /// Existing parent directory.
        parent: PathBuf,
        /// Validated portable directory component.
        name: String,
    },
}
impl Intent {
    /// Provisional name while the manifest is being read.
    pub fn display_name(&self) -> String {
        match self {
            Self::Create { name, .. } => name.clone(),
            Self::Open(path) => path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
        }
    }
    /// Filesystem target of this intent.
    pub fn root(&self) -> PathBuf {
        match self {
            Self::Open(path) => path.clone(),
            Self::Create { parent, name } => parent.join(name),
        }
    }
}

/// Feature-local model, owned as one value by the native application adapter.
/// Methods enforce lifecycle invariants; effects and UI ownership stay outside.
#[derive(Default)]
pub struct StartupModel {
    pub(crate) phase: Phase,
    pub(crate) project: Option<Project>,
    pub(crate) history: RecentProjects,
    pub(crate) history_path: Option<PathBuf>,
    pub(crate) history_writable: bool,
    pub(crate) notice: String,
    pub(crate) filter: String,
    pub(crate) initial_project: Option<PathBuf>,
    pub(crate) intent: Option<Intent>,
    preparation: Preparation,
    pub(crate) generation: u64,
}
impl StartupModel {
    /// Current application phase; the Bevy state mirrors this value.
    pub fn phase(&self) -> Phase {
        self.phase
    }
    /// Validated open project, never a partially parsed manifest.
    pub fn project(&self) -> Option<&Project> {
        self.project.as_ref()
    }
    /// Failure diagnostic for the current request, if preparation has finished.
    pub fn loading_error(&self) -> Option<&str> {
        match &self.preparation {
            Preparation::Failed(error) => Some(error),
            _ => None,
        }
    }
    /// Whether preparation succeeded and is waiting for native surface readiness.
    pub fn is_ready(&self) -> bool {
        matches!(self.preparation, Preparation::Ready)
    }

    /// Accept one request. Repeated activation while loading cannot replace it.
    /// Returns the generation to attach to an asynchronous completion.
    pub fn begin(&mut self, intent: Intent) -> Option<u64> {
        if self.phase != Phase::Projects {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.intent = Some(intent);
        self.project = None;
        self.preparation = Preparation::Working;
        self.phase = Phase::Loading;
        Some(self.generation)
    }

    /// Publish only a matching completion. A failure retains its request and
    /// error, but never exposes a project to the editor surface.
    pub fn complete(&mut self, generation: u64, result: Result<Project, String>) -> bool {
        if self.phase != Phase::Loading
            || generation != self.generation
            || !matches!(self.preparation, Preparation::Working)
        {
            return false;
        }
        match result {
            Ok(project) => {
                self.project = Some(project);
                self.preparation = Preparation::Ready;
            }
            Err(error) => self.preparation = Preparation::Failed(error),
        }
        true
    }

    /// Enter the editor only after the host confirms native loading readiness.
    pub fn enter_editor(&mut self) -> bool {
        if !self.is_ready() || self.project.is_none() || self.phase != Phase::Loading {
            return false;
        }
        self.phase = Phase::Editor;
        true
    }

    /// Return from an opened project or completed failure. In-flight creation
    /// cannot be cancelled with a false promise of filesystem rollback.
    pub fn back_to_projects(&mut self) -> bool {
        if self.phase != Phase::Editor
            && !(self.phase == Phase::Loading && self.loading_error().is_some())
        {
            return false;
        }
        self.project = None;
        self.phase = Phase::Projects;
        self.preparation = Preparation::Idle;
        self.intent = None;
        true
    }

    /// Retry a failed read-only open with a new completion generation. Creation
    /// failures return to the form instead of silently replaying a source writer.
    pub fn retry(&mut self) -> bool {
        if self.phase != Phase::Loading
            || self.loading_error().is_none()
            || !matches!(self.intent, Some(Intent::Open(_)))
        {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.preparation = Preparation::Working;
        true
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryMode {
    Create,
    Open,
}

#[derive(Default)]
pub(crate) struct EntryDraft {
    pub mode: Option<EntryMode>,
    pub name: String,
    pub directory: String,
    pub error: String,
}

pub(crate) fn entry_intent(
    form: &EntryDraft,
    home: Option<&std::path::Path>,
) -> Result<Intent, String> {
    if form.directory.is_empty() {
        return Err("Choose an existing directory.".into());
    }
    let directory = if form.directory == "~" || form.directory.starts_with("~/") {
        let home = home.ok_or("Cannot resolve the home directory; enter an absolute path.")?;
        home.join(form.directory.strip_prefix("~/").unwrap_or(""))
    } else {
        PathBuf::from(&form.directory)
    };
    if !directory.is_absolute() {
        return Err("Enter an absolute directory path or use Browse….".into());
    }
    match form.mode {
        Some(EntryMode::Create) => {
            kedra_project::validate_name(&form.name).map_err(|e| e.to_string())?;
            Ok(Intent::Create {
                parent: directory,
                name: form.name.clone(),
            })
        }
        Some(EntryMode::Open) => Ok(Intent::Open(directory)),
        None => Err("No project entry form is open.".into()),
    }
}
