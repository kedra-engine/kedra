//! Luau consumer of the Inspector-owned authored kit, with explicit host plugins.
#[path = "../../../kedra_inspector/tests/support/kit.rs"]
mod kit;
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub fn fixture() -> TestResult<(
    bevy::prelude::App,
    bevy::prelude::Entity,
    kedra_ui::fields::FieldTemplates,
)> {
    kit::fixture(|app| {
        app.add_plugins((
            kedra_script::LuauPlugin,
            kedra_inspector_luau::InspectorLuauPlugin,
        ));
    })
}
