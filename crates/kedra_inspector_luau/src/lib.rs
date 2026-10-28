//! Explicitly authorized Luau drawers over the existing Inspector and native field kit.
//! Scripts choose native field paths/labels, never own source authority or mutable World access.
mod bindings;
mod view;
use bevy::prelude::*;
use kedra_inspector::{binding::ValueBinding, editors::*};
use kedra_script::{Program, Run, Status};
use kedra_ui::fields::{FieldAccess, FieldTemplates, PropertyRow, SectionView, ValueView};

/// Presentation change notification, including asynchronous script failure.
/// Hosts refresh their existing TreeViews; it is not an authoring command.
#[derive(Event)]
pub struct DrawerChanged;

/// Whether an emitted field belongs to a complete, successful composition.
/// Consumers must still check native access, source identity and revision before editing.
pub fn ready(world: &World, field: &kedra_inspector::fields::ReflectedField) -> bool {
    world
        .get::<State>(field.owner)
        .is_some_and(|state| matches!(state.outcome, Some(Ok(()))))
}

/// Polls owned drawer outcomes after the shared runtime dispatch. Does not install
/// LuauPlugin, authorize project scripts, or discover/load source files.
pub struct InspectorLuauPlugin;
impl Plugin for InspectorLuauPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, view::outcomes.after(kedra_script::ScriptSystems::Run));
    }
}

/// One trusted-host-selected finite composition program. Registration alone does
/// not execute it. Mount authorizes a fresh owner-bound VM through kedra_script.
/// Scripts implement `run(ctx, args)` and use `ctx.fields.add(path, label)`.
pub struct LuauEditor {
    program: Program,
}
impl LuauEditor {
    /// Select owned source explicitly, with the shared runtime's budgets/module rules.
    /// Source syntax/entrypoint errors become ordinary custom-editor diagnostics.
    pub fn new(name: &str, path: &str, source: &str) -> Self {
        Self {
            program: Program::sequence(name, path, source, bindings::install),
        }
    }
    /// Supply the existing runtime's admitted module snapshot; no disk access is added.
    pub fn sources(mut self, sources: kedra_script::Sources) -> Self {
        self.program.sources = sources;
        self
    }
}
impl ValueEditor for LuauEditor {
    fn layout(&self) -> EditorLayout {
        EditorLayout::Block
    }
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        view::mount(world, parent, context, binding, self.program.clone())
    }
}

struct Control {
    path: bevy::reflect::ParsedPath,
    label: String,
    row: PropertyRow,
    value: ValueView,
    choice: Option<kedra_inspector::controls::ChoiceView>,
}
#[derive(Component)]
struct State {
    templates: FieldTemplates,
    namespace: String,
    candidates: Vec<(
        bevy::reflect::ParsedPath,
        kedra_inspector::fields::ReflectedField,
    )>,
    limit: usize,
    controls: Vec<Control>,
    run: Option<Run>,
    outcome: Option<Result<(), String>>,
}
