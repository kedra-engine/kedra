//! Windowless startup: delivery verification and authored catalog loading overlap.
use bevy::{prelude::*, tasks::IoTaskPool};
use kedra_scene::assets::PrefabSources;
use kedra_wakeup::{Task, Wakeup};
use std::{path::PathBuf, time::Duration};

#[derive(Resource, Default)]
pub(crate) struct Boot {
    pub complete: bool,
    pub delivery: Option<(PathBuf, kedra_assets::delivery::Manifest)>,
    pub(crate) started: Option<Duration>,
    verification: Option<Task<Result<(), String>>>,
    verified: bool,
}

pub(crate) fn poll(world: &mut World) {
    if *world.resource::<State<crate::AppPhase>>().get() != crate::AppPhase::Boot
        || world.resource::<Boot>().complete
    {
        return;
    }
    let elapsed = world.resource::<Time<Real>>().elapsed();
    if world.resource::<Boot>().started.is_none() {
        let Some((delivery, manifest)) = world.resource::<Boot>().delivery.clone() else {
            crate::views::fail(world, "Editor asset delivery is not configured".into());
            return;
        };
        info!("Editor startup: verifying delivery and loading editor sources concurrently");
        let verification = Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
            verify_delivery(&delivery, &manifest)
        });
        let mut boot = world.resource_mut::<Boot>();
        boot.started = Some(elapsed);
        boot.verification = Some(verification);
    }
    let result = world
        .resource_mut::<Boot>()
        .verification
        .as_mut()
        .and_then(Task::poll);
    if let Some(result) = result {
        world.resource_mut::<Boot>().verification = None;
        match result {
            Ok(()) => world.resource_mut::<Boot>().verified = true,
            Err(error) => {
                crate::views::fail(world, error);
                return;
            }
        }
    }
    let sources = world.resource::<PrefabSources>();
    if let Some(error) = sources.error.clone() {
        crate::views::fail(world, error);
        return;
    }
    if sources.ready() && world.resource::<Boot>().verified {
        world.resource_mut::<Boot>().complete = true;
        crate::jobs::load_history(world);
        info!("Editor startup complete; entering project workflow");
    } else if world
        .resource::<Boot>()
        .started
        .is_some_and(|started| elapsed.saturating_sub(started) > Duration::from_secs(30))
    {
        crate::views::fail(
            world,
            "Editor startup did not complete within 30 seconds".into(),
        );
    }
}

fn verify_delivery(
    root: &std::path::Path,
    expected: &kedra_assets::delivery::Manifest,
) -> Result<(), String> {
    let (_, verified) = kedra_assets::delivery::read_directory(root, "editor-desktop")?;
    if serde_json::to_value(expected).map_err(|e| e.to_string())?
        != serde_json::to_value(verified).map_err(|e| e.to_string())?
    {
        return Err("Asset delivery manifest changed during editor startup".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "boot/lifecycle_tests.rs"]
mod lifecycle_tests;
