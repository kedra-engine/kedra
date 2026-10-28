use bevy::{
    asset::AssetPlugin,
    input::InputPlugin,
    input_focus::{InputDispatchPlugin, InputFocusPlugin},
    prelude::*,
    text::{EditableText, TextEdit, TextPlugin},
    ui_widgets::{TextInput, TextInputPlugin},
};
use kedra_text_input::{Command, InputEvent, TextSession, TextSessionPlugin};
pub type TestResult = Result<(), Box<dyn std::error::Error>>;
#[derive(Resource, Default)]
pub struct Events(pub Vec<InputEvent>);
pub fn app(value: &str, limit: usize) -> Result<(App, Entity), Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        WindowPlugin {
            primary_window: None,
            ..default()
        },
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        TextPlugin,
        TextInputPlugin,
        TextSessionPlugin,
    ))
    .init_resource::<bevy::picking::events::PointerState>()
    .init_resource::<UiScale>()
    .init_resource::<Events>()
    .add_message::<bevy::picking::events::PointerRelease>()
    .add_observer(|event: On<InputEvent>, mut events: ResMut<Events>| events.0.push(event.clone()));
    let font = bevy::text::Font::from_bytes(
        include_bytes!("../../../../tests/fixtures/test_project/assets/Fixture-Inter.ttf").to_vec(),
    );
    app.world_mut()
        .resource_mut::<bevy::text::FontCx>()
        .collection
        .register_fonts(font.data, None);
    let mut text = EditableText::new(value);
    text.viewport.size = Vec2::new(320., 22.);
    text.editor.set_width(Some(320.));
    text.editor
        .edit_styles()
        .insert(parley::FontFamilyName::Named(std::borrow::Cow::Borrowed("Inter")).into());
    text.editor
        .edit_styles()
        .insert(parley::StyleProperty::FontSize(14.));
    let session = TextSession::attach(&text, limit).map_err(|e| format!("{e:?}"))?;
    let input = app.world_mut().spawn((TextInput, text, session)).id();
    app.finish();
    app.cleanup();
    app.update();
    Ok((app, input))
}
pub fn edit(app: &mut App, input: Entity, edit: TextEdit) -> TestResult {
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("input")?
        .queue_edit(edit);
    Ok(())
}
pub fn command(app: &mut App, input: Entity, command: Command) -> TestResult {
    let world = app.world_mut();
    let mut query = world.query::<(&mut TextSession, &mut EditableText)>();
    let (mut session, mut text) = query.get_mut(world, input)?;
    session.request(&mut text, command);
    Ok(())
}
pub fn value(app: &App, input: Entity) -> Result<String, Box<dyn std::error::Error>> {
    Ok(app
        .world()
        .get::<EditableText>(input)
        .ok_or("input")?
        .value()
        .to_string())
}
pub fn selection(app: &App, input: Entity) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let selection = app
        .world()
        .get::<EditableText>(input)
        .ok_or("input")?
        .editor
        .raw_selection();
    Ok((selection.anchor().index(), selection.focus().index()))
}
