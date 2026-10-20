//! Opt-in real AppKit contract probe; not a physical drag/drop acceptance test.
//! Run on the macOS main thread with `cargo run -p kedra_desktop --example macos_header_policy`.

#[cfg(target_os = "macos")]
fn main() {
    use bevy::prelude::*;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Kedra native header policy probe".into(),
                visible: false,
                decorations: true,
                fullsize_content_view: true,
                titlebar_transparent: true,
                titlebar_show_title: false,
                movable_by_window_background: false,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Window {
                title: "Kedra background drag opt-in probe".into(),
                visible: false,
                decorations: true,
                fullsize_content_view: true,
                titlebar_transparent: true,
                movable_by_window_background: true,
                ..default()
            });
        })
        .add_systems(Update, check_native_policy)
        .run();
}

#[cfg(target_os = "macos")]
fn check_native_policy(
    _main: bevy::ecs::system::NonSendMarker,
    mut exit: bevy::prelude::MessageWriter<bevy::app::AppExit>,
) {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let Some(marker) = MainThreadMarker::new() else {
        panic!("Native header policy must be checked on the main thread");
    };
    let windows = NSApplication::sharedApplication(marker).windows();
    let Some(window) = windows
        .iter()
        .find(|window| window.title().to_string() == "Kedra native header policy probe")
    else {
        return;
    };
    let Some(background_window) = windows
        .iter()
        .find(|window| window.title().to_string() == "Kedra background drag opt-in probe")
    else {
        return;
    };
    let Some(background_view) = background_window.contentView() else {
        panic!("Background-movable native window has no content view");
    };
    assert!(background_window.isMovableByWindowBackground());
    assert!(
        background_view.mouseDownCanMoveWindow(),
        "Explicit background drag opt-in must be preserved"
    );
    let Some(view) = window.contentView() else {
        panic!("Native window has no content view");
    };
    assert!(
        window.isMovable(),
        "Explicit native window drag must remain available"
    );
    assert!(!window.isMovableByWindowBackground());
    assert!(
        !view.mouseDownCanMoveWindow(),
        "AppKit may steal tab drags before client input: content view allows automatic window movement"
    );
    println!(
        "PASS: AppKit leaves content drags to the client; explicit native drag remains enabled"
    );
    exit.write(bevy::app::AppExit::Success);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The native header policy probe requires macOS/AppKit");
    std::process::exit(1);
}
