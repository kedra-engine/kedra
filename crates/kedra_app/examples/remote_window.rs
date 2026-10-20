//! Independent native game-style host: no editor, importer or project writer.
//! Set KEDRA_QA_CREDENTIALS to a NEW path; optionally KEDRA_QA_ARTIFACTS.
use bevy::prelude::*;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let credentials = std::env::var_os("KEDRA_QA_CREDENTIALS")
        .map(PathBuf::from)
        .ok_or("Set KEDRA_QA_CREDENTIALS to a new local credential path")?;
    let artifacts = std::env::var_os("KEDRA_QA_ARTIFACTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/qa/game-captures".into());
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, kedra_app::KedraAppPlugin));
    app.add_systems(Startup, setup);
    let endpoint = kedra_app::transport::enable(&mut app, 0, &credentials, &artifacts)?;
    eprintln!(
        "Independent host listening at {}; credentials: {}",
        endpoint.url,
        credentials.display()
    );
    let exit = app.run();
    if exit.is_error() {
        Err("Native host failed".into())
    } else {
        Ok(())
    }
}
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(36, 36, 36)),
            kedra_ui::ScreenId("game.fixture".into()),
            kedra_ui::ElementId::new("game.panel"),
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    width: px(120),
                    height: px(64),
                    ..default()
                },
                BackgroundColor(Color::srgb_u8(65, 107, 157)),
                kedra_ui::ElementId::new("game.target"),
            ));
        });
}
