//! Editor-owned explicit restart action crossing Bevy's consumed-World boundary.
use std::{ffi::OsString, process::Child};

type Commit = dyn FnOnce(&[OsString]) -> Result<Child, String> + Send + Sync;

pub(crate) struct Restart {
    commit: Box<Commit>,
}

impl Restart {
    pub(super) fn new(prepared: kedra_update::Prepared) -> Self {
        Self {
            commit: Box::new(move |arguments| prepared.commit(arguments)),
        }
    }

    pub(crate) fn commit(self, arguments: &[OsString]) -> Result<Child, String> {
        (self.commit)(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn dropping_authorized_restart_cleans_up_without_committing()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().to_owned();
        let committed = Arc::new(AtomicBool::new(false));
        let receipt = committed.clone();
        let restart = Restart {
            commit: Box::new(move |_| {
                let _owned = directory;
                receipt.store(true, Ordering::SeqCst);
                Err("Must not be invoked on cancellation".into())
            }),
        };
        let mut app = crate::workspace::browser_test_app()?;
        app.init_resource::<crate::model::ProjectManager>()
            .add_message::<AppExit>();
        let generation = app
            .world()
            .resource::<crate::model::ProjectManager>()
            .generation;
        app.insert_resource(crate::updates::Updates {
            prepared: Some(restart),
            project_generation: Some(generation),
            ..Default::default()
        });
        let receiver = crate::updates::handoff(app.world_mut());
        crate::updates::authorize_exit(app.world_mut());
        assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
        assert!(path.exists());
        drop(receiver);
        assert!(!path.exists());
        assert!(!committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[test]
    fn explicit_handoff_survives_bevy_consuming_the_app() -> Result<(), Box<dyn std::error::Error>>
    {
        let committed = Arc::new(AtomicBool::new(false));
        let receipt = committed.clone();
        let restart = Restart {
            commit: Box::new(move |_| {
                receipt.store(true, Ordering::SeqCst);
                Err("Isolated test does not spawn an application".into())
            }),
        };
        let mut app = crate::workspace::browser_test_app()?;
        app.init_resource::<crate::model::ProjectManager>()
            .add_message::<AppExit>();
        let generation = app
            .world()
            .resource::<crate::model::ProjectManager>()
            .generation;
        app.insert_resource(crate::updates::Updates {
            prepared: Some(restart),
            project_generation: Some(generation),
            ..Default::default()
        });
        let receiver = crate::updates::handoff(app.world_mut());
        crate::updates::authorize_exit(app.world_mut());
        assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
        assert!(
            !committed.load(Ordering::SeqCst),
            "Exit authorization must not install the update"
        );
        app.set_runner(|app| {
            drop(app);
            AppExit::Success
        });
        app.run();
        let (restart, arguments) = receiver.try_recv()?;
        assert!(!committed.load(Ordering::SeqCst));
        assert!(restart.commit(&arguments).is_err());
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }
}
