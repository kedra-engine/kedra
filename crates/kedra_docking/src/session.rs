//! Unpublished interaction previews. No event loop, timer or ECS scheduler.
use crate::{
    Error, Result,
    command::Command,
    model::{Candidate, Workspace},
};
use uuid::Uuid;

/// A drag/resize preview bound to the starting publication. Dropping cancels.
///
/// Every update plans from the original committed state, not the previous preview.
/// Thus resize Escape needs no compensating command or history mutation. Native
/// adapters must additionally cancel on capture loss or retired source entities.
pub struct Session {
    stamp: (Uuid, u64),
    candidate: Option<Candidate>,
}
impl Session {
    /// Start against the current workspace, without changing state or revision.
    pub fn new(workspace: &Workspace) -> Self {
        Self {
            stamp: workspace.stamp(),
            candidate: None,
        }
    }
    /// Replace the preview. Failure clears the previous preview, preventing a
    /// release over an invalid target from accidentally committing an older one.
    pub fn update(&mut self, workspace: &Workspace, command: Command) -> Result<&Candidate> {
        self.candidate = None;
        if workspace.stamp() != self.stamp {
            return Err(Error::Stale);
        }
        self.candidate = Some(workspace.prepare(command)?);
        self.candidate.as_ref().ok_or(Error::Missing)
    }
    /// Latest valid preview, if any. Adapters project its snapshot, not a second model.
    pub fn candidate(&self) -> Option<&Candidate> {
        self.candidate.as_ref()
    }
    /// Publish once on release. Empty/invalid preview cannot publish a prior target.
    pub fn finish(self, workspace: &mut Workspace) -> Result<bool> {
        if workspace.stamp() != self.stamp {
            return Err(Error::Stale);
        }
        workspace.commit(self.candidate.ok_or(Error::Missing)?)
    }
}
