//! Headless Inspector host; the authored kit is also used by its Luau adapter.
mod kit;
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub fn fixture() -> TestResult<(
    bevy::prelude::App,
    bevy::prelude::Entity,
    kedra_ui::fields::FieldTemplates,
)> {
    kit::fixture(|_| {})
}
