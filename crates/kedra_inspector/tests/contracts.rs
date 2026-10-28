//! Public reflection/presentation contracts with independent read-only consumers.
#[path = "contracts/builtin_views.rs"]
mod builtin_views;
#[path = "contracts/collection_editing.rs"]
mod collection_editing;
#[path = "contracts/custom_views.rs"]
mod custom_views;
#[path = "contracts/discrete_controls.rs"]
mod discrete_controls;
#[path = "contracts/editor_dispatch.rs"]
mod editor_dispatch;
#[path = "contracts/grouped_layout.rs"]
mod grouped_layout;
#[path = "contracts/reflected_values.rs"]
mod reflected_values;
#[path = "contracts/resource_references.rs"]
mod resource_references;
mod support;
#[path = "contracts/typed_fields.rs"]
mod typed_fields;
