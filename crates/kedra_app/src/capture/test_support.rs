//! Session-owned capture inputs without a renderer or native-frame claim.
use super::*;
pub(super) fn host() -> (App, tempfile::TempDir, Entity) {
    let directory = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
    let mut app = App::new();
    app.add_plugins(CapturePlugin)
        .insert_resource(ArtifactRoot(directory.path().into()))
        .insert_resource(crate::AppSession { id: "test".into() })
        .init_resource::<crate::input::InputQueue>()
        .init_resource::<crate::operations::Operations>()
        .init_resource::<bevy::diagnostic::FrameCount>();
    let window = app.world_mut().spawn(Window::default()).id();
    (app, directory, window)
}
pub(super) fn admit(world: &mut World, window: Entity) -> Uuid {
    let id = Uuid::new_v4();
    request(
        In(Some(
            json!({"session":"test","id":id,"window":window.to_bits()}),
        )),
        world,
    )
    .unwrap_or_else(|error| panic!("{error:?}"));
    id
}
pub(super) fn image() -> Image {
    Image::from_dynamic(image::DynamicImage::new_rgba8(2, 2), true, default())
}
