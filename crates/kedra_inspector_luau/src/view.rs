//! One native subtree and shared-runtime owner per presentation. No independent VM.
use super::*;
use bevy::reflect::ReflectPath;
use kedra_inspector::fields::ReflectedField;
use kedra_ui::docking::class;

struct View {
    section: SectionView,
    run: Run,
}
pub(super) fn mount(
    world: &mut World,
    parent: Entity,
    context: &mut EditorContext<'_>,
    binding: ValueBinding<'_>,
    program: Program,
) -> Result<Box<dyn ValueEditorView>, String> {
    let section = SectionView::mount(world, parent, context.templates, context.scope)
        .map_err(|error| error.to_string())?;
    class(world, section.root(), "property-composition", true);
    let root = section.body();
    let limit = context.budget.reserve(32);
    let candidates = candidates(root, context, binding, limit);
    world.entity_mut(root).insert(State {
        templates: context.templates.clone(),
        namespace: context.scope.into(),
        candidates,
        limit,
        controls: Vec::new(),
        run: None,
        outcome: None,
    });
    let run = match kedra_script::attach(world, root, program, serde_json::Value::Null) {
        Ok(run) => run,
        Err(error) => {
            section.retire(world);
            return Err(error.to_string());
        }
    };
    world
        .get_mut::<State>(root)
        .ok_or("Drawer state disappeared")?
        .run = Some(run);
    Ok(Box::new(View { section, run }))
}
impl ValueEditorView for View {
    fn root(&self) -> Entity {
        self.section.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let root = self.section.body();
        let Some(mut state) = world
            .get_entity_mut(root)
            .map_err(|_| "Drawer was retired")?
            .take::<State>()
        else {
            return Err("Drawer was retired".into());
        };
        let result: Result<(), String> = (|| {
            if let Some(Err(error)) = &state.outcome {
                return Err(error.clone());
            }
            if context.budget.reserve(state.controls.len()) != state.controls.len() {
                return Err("Shared Inspector field budget exhausted; use default view.".into());
            }
            if state.outcome.is_none() {
                state.candidates = candidates(root, context, binding, state.limit);
            }
            for (index, control) in state.controls.iter_mut().enumerate() {
                let field =
                    ReflectedField::bind(root, control.row.diagnostic(), binding, &control.path)?;
                control
                    .row
                    .update(world, &control.label, None)
                    .map_err(|error| error.to_string())?;
                let pending = FieldAccess::ReadOnly {
                    reason: "Luau drawer composition is pending".into(),
                };
                let access = if state.outcome.is_none() {
                    &pending
                } else {
                    binding.access
                };
                let native = control
                    .path
                    .reflect_element(binding.value)
                    .map_err(|e| e.to_string())?;
                let current = ValueBinding {
                    value: native,
                    path: Some(&field.path),
                    label: &control.label,
                    access,
                    ..binding
                };
                if let Some(choices) = kedra_inspector::controls::finite_choices(current) {
                    if control.choice.is_none() {
                        control.choice = Some(kedra_inspector::controls::ChoiceView::mount(
                            world,
                            control.row.editor(),
                            context,
                            &format!("{}/field-{index}/choice", state.namespace),
                        )?);
                    }
                    let choice = control.choice.as_ref().ok_or("Native choice disappeared")?;
                    choice.update(
                        world,
                        current,
                        control.row.diagnostic(),
                        &field.display(),
                        choices,
                    )?;
                    if let Some(checked) = native.try_downcast_ref::<bool>() {
                        choice.checkbox(world, *checked, &control.label);
                    }
                    class(world, control.value.root(), "hidden", true);
                    world
                        .entity_mut(control.value.input())
                        .remove::<ReflectedField>();
                    continue;
                }
                control
                    .value
                    .update(world, &field.display(), access)
                    .map_err(|error| error.to_string())?;
                let description = binding
                    .metadata
                    .and_then(|metadata| metadata.describe(Some(&field.path)))
                    .unwrap_or_default();
                control
                    .value
                    .describe(
                        world,
                        &control.label,
                        &format!("{}\n{description}", binding.access.description()),
                    )
                    .map_err(|error| error.to_string())?;
                world.entity_mut(control.value.input()).insert(field);
            }
            Ok(())
        })();
        if let Err(error) = &result {
            state.outcome = Some(Err(error.clone()));
            for control in &state.controls {
                if let Some(choice) = &control.choice {
                    world
                        .entity_mut(choice.root())
                        .insert(FieldAccess::ReadOnly {
                            reason: "Luau drawer failed; use default view.".into(),
                        });
                }
                world
                    .entity_mut(control.value.input())
                    .insert(FieldAccess::ReadOnly {
                        reason: "Luau drawer failed; use default view.".into(),
                    });
            }
        }
        world.entity_mut(root).insert(state);
        result
    }
    fn retire(&self, world: &mut World) {
        let _ = kedra_script::retire(world, self.run);
        self.section.retire(world);
    }
}
fn candidates(
    root: Entity,
    context: &EditorContext<'_>,
    binding: ValueBinding<'_>,
    limit: usize,
) -> Vec<(bevy::reflect::ParsedPath, ReflectedField)> {
    kedra_inspector::discovery::inspect(
        binding.value,
        context.types,
        kedra_inspector::discovery::Limits::default(),
    )
    .into_iter()
    .filter_map(|row| {
        if row.diagnostic.is_some() {
            return None;
        }
        let path = row.path?;
        ReflectedField::bind(root, Entity::PLACEHOLDER, binding, &path)
            .ok()
            .map(|field| (path, field))
    })
    .take(limit)
    .collect()
}
pub(super) fn outcomes(world: &mut World) {
    let pending: Vec<_> = world
        .query::<(Entity, &State)>()
        .iter(world)
        .filter_map(|(root, state)| {
            if state.outcome.is_some() {
                return None;
            }
            let outcome = match state.run.and_then(|run| kedra_script::status(world, run)) {
                Some(Status::Running) => return None,
                Some(Status::Completed(_)) if !state.controls.is_empty() => Ok(()),
                Some(Status::Failed(error)) => Err(error.to_string()),
                _ => Err("Drawer did not produce a complete presentation".into()),
            };
            Some((root, outcome))
        })
        .collect();
    for (root, outcome) in pending {
        if outcome.is_err() {
            let inputs = world
                .get::<State>(root)
                .map(|state| {
                    state
                        .controls
                        .iter()
                        .map(|control| control.value.input())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for input in inputs {
                world.entity_mut(input).insert(FieldAccess::ReadOnly {
                    reason: "Luau drawer failed; use default view.".into(),
                });
            }
        }
        if let Some(mut state) = world.get_mut::<State>(root) {
            state.outcome = Some(outcome);
            state.candidates.clear();
        }
        world.trigger(DrawerChanged);
    }
}
