//! Public scoped execution, lifecycle and budget contracts through the actual Luau VM.
#[path = "contracts/behaviors.rs"]
mod behaviors;
#[path = "contracts/budgets.rs"]
mod budgets;
#[path = "contracts/children.rs"]
mod children;
#[path = "contracts/diagnostics.rs"]
mod diagnostics;
#[path = "contracts/events.rs"]
mod events;
#[path = "contracts/lifecycle.rs"]
mod lifecycle;
#[path = "contracts/modules.rs"]
mod modules;
#[path = "contracts/parameters.rs"]
mod parameters;
mod support;
