//! Compact retained arrays. Item fields carry full native paths to the host;
//! only complete structural proposals pass through the array's root control.
use super::*;
use crate::{
    binding::ValueBinding,
    controls::ControlBinding,
    editors::*,
    view::{Disclosure, TreeView},
};
use bevy::reflect::{Access, ParsedPath, ReflectRef};
use kedra_ui::{
    docking::class,
    fields::{CommandView, PropertyRow, SectionView},
};

pub(crate) enum Kind {
    List,
    Classes,
    Shadows,
}
pub(crate) struct CollectionEditor(pub Kind);
struct ItemView {
    id: u64,
    row: PropertyRow,
    tree: TreeView,
    grip: CommandView,
    toggle: Option<CommandView>,
}
struct CollectionView {
    section: SectionView,
    count: usize,
    commands: Vec<CommandView>,
    items: Vec<ItemView>,
    scope: String,
    kind: Kind,
    filter: String,
}
fn list_binding<'a>(
    kind: &Kind,
    binding: ValueBinding<'a>,
) -> Result<(&'a dyn PartialReflect, ParsedPath), String> {
    let mut path = binding
        .path
        .into_iter()
        .flat_map(|path| &path.0)
        .map(|part| part.access.clone())
        .collect::<Vec<_>>();
    let value: &dyn PartialReflect = match kind {
        Kind::List => binding.value,
        Kind::Classes => {
            path.push(Access::Field("names".into()));
            &binding
                .value
                .try_downcast_ref::<kedra_ui::authored::components::UiClasses>()
                .ok_or("Native classes required")?
                .names
        }
        Kind::Shadows => {
            path.push(Access::TupleIndex(0));
            &binding
                .value
                .try_downcast_ref::<BoxShadow>()
                .ok_or("Native shadows required")?
                .0
        }
    };
    value::count(value)?;
    Ok((value, ParsedPath::from(path)))
}
impl ValueEditor for CollectionEditor {
    fn layout(&self) -> EditorLayout {
        EditorLayout::Block
    }
    fn title(&self) -> Option<&'static str> {
        match self.0 {
            Kind::Classes => Some("Classes"),
            Kind::Shadows => Some("Shadows"),
            Kind::List => None,
        }
    }
    fn search_terms(&self) -> &'static [&'static str] {
        &["Add", "Remove", "Reorder"]
    }
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let (list, _) = list_binding(&self.0, binding)?;
        let count = value::count(list)?;
        if count > 128 {
            return Err("Collection presentation is limited to 128 entries".into());
        }
        let section = SectionView::mount(world, parent, context.templates, context.scope)
            .map_err(|e| e.to_string())?;
        let root = section.root();
        class(world, root, "property-composition", true);
        class(world, root, "property-array", true);
        let result = (|| {
            let diagnostic = PropertyRow::mount(
                world,
                section.body(),
                context.templates,
                &format!("{}/diagnostic", context.scope),
            )
            .map_err(|e| e.to_string())?;
            class(world, diagnostic.root(), "property-diagnostic-row", true);
            let area = SectionView::mount(
                world,
                section.body(),
                context.templates,
                &format!("{}/items", context.scope),
            )
            .map_err(|e| e.to_string())?;
            class(world, area.root(), "property-composition", true);
            let actions = SectionView::mount(
                world,
                section.body(),
                context.templates,
                &format!("{}/actions", context.scope),
            )
            .map_err(|e| e.to_string())?;
            class(world, actions.root(), "property-composition", true);
            class(world, actions.body(), "property-actions", true);
            let mut commands = Vec::new();
            for name in ["add", "remove"] {
                let command = CommandView::mount(
                    world,
                    actions.body(),
                    context.templates,
                    &format!("{}/{name}", context.scope),
                )
                .map_err(|e| e.to_string())?;
                class(
                    world,
                    command.root(),
                    if name == "add" {
                        "property-array-add"
                    } else {
                        "property-array-remove"
                    },
                    true,
                );
                commands.push(command);
            }
            world.entity_mut(root).insert(State {
                value: value::native(list, context.types)?,
                revision: 0,
                ids: (1..=count as u64).collect(),
                next_id: count as u64 + 1,
                selected: None,
                pending: None,
                diagnostic: diagnostic.diagnostic(),
                area: area.body(),
                queued: 0,
                classes: matches!(self.0, Kind::Classes),
                drag: None,
                focus: None,
                focus_ready: false,
                inserted_image: false,
                expanded: default(),
                restoration: None,
            });
            let mut view = CollectionView {
                section,
                count,
                commands,
                items: Vec::new(),
                scope: context.scope.into(),
                kind: match self.0 {
                    Kind::List => Kind::List,
                    Kind::Classes => Kind::Classes,
                    Kind::Shadows => Kind::Shadows,
                },
                filter: String::new(),
            };
            view.update(world, context, binding)?;
            Ok(Box::new(view) as Box<dyn ValueEditorView>)
        })();
        if result.is_err()
            && let Ok(entity) = world.get_entity_mut(root)
        {
            entity.despawn();
        }
        result
    }
}
impl ValueEditorView for CollectionView {
    fn item_count(&self) -> Option<usize> {
        Some(self.count)
    }
    fn root(&self) -> Entity {
        self.section.root()
    }
    fn filter(&mut self, filter: &str) {
        self.filter = filter.into();
    }
    fn owns_disclosure(&self, action: &Disclosure) -> bool {
        self.items
            .iter()
            .any(|item| item.tree.owns_disclosure(action))
    }
    fn disclose(&mut self, action: &Disclosure) -> bool {
        self.items.iter_mut().any(|item| item.tree.disclose(action))
    }
    fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let root = self.root();
        let mut state = world
            .entity_mut(root)
            .take::<State>()
            .ok_or("Collection retired")?;
        let result = (|| {
            if state.pending.as_ref().is_some_and(|pending| {
                pending.binding.token != binding.token
                    || world
                        .get::<Control>(root)
                        .is_none_or(|control| !Arc::ptr_eq(&control.0, &pending.binding))
            }) {
                state.pending = None;
            }
            let (source, source_path) = list_binding(&self.kind, binding)?;
            let count = value::count(source)?;
            if count > 128 {
                return Err("Collection presentation is limited to 128 entries".into());
            }
            let changed = state.value.as_ref().reflect_partial_eq(source) != Some(true);
            if let Some(restoration) = state.restoration.take() {
                restoration.apply(&mut state, count);
            }
            if changed {
                state.value = value::native(source, context.types)?;
                state.revision = state
                    .revision
                    .checked_add(1)
                    .ok_or("Collection revision exhausted")?;
            }
            if count != state.ids.len() {
                // Foreign structural source changes retire presentation identities;
                // accepted local proposals already supplied their retained mapping.
                state.ids.clear();
                for _ in 0..count {
                    state.ids.push(state.next_id);
                    state.next_id = state
                        .next_id
                        .checked_add(1)
                        .ok_or("Collection identity exhausted")?;
                }
                state.selected = None;
                if state.inserted_image {
                    state.selected = state.ids.last().copied();
                    state.focus = state.selected.map(interaction::Focus::Item);
                    state.focus_ready = false;
                }
                state.inserted_image = false;
            }
            self.count = count;
            // Keep an already queued action pinned to its original source binding.
            if state.queued == 0 && state.pending.is_none() && state.drag.is_none() {
                world
                    .entity_mut(root)
                    .insert(Control(Arc::new(ControlBinding {
                        token: binding.token,
                        path: source_path.clone(),
                        diagnostic: state.diagnostic,
                    })));
            }
            world.entity_mut(root).insert(binding.access.clone());
            let enabled = !binding.access.is_read_only()
                && state.pending.is_none()
                && !unsettled(world, state.area, None);
            let selected = state
                .selected
                .and_then(|id| state.ids.iter().position(|candidate| *candidate == id));
            for (index, (label, kind, available)) in [
                (
                    "Add item",
                    Operation::Add,
                    count < if state.classes { 64 } else { 128 }
                        && value::new_item(&state, context.types).is_ok(),
                ),
                (
                    "Remove item",
                    Operation::Remove(selected.unwrap_or(0)),
                    selected.is_some(),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let command = &self.commands[index];
                command
                    .update(world, label, enabled && available)
                    .map_err(|e| e.to_string())?;
                world.entity_mut(command.root()).insert((
                    binding.access.clone(),
                    Action {
                        owner: root,
                        revision: state.revision,
                        kind,
                    },
                ));
                if index == 0 {
                    let needs_image = count == 0 && state.value.try_downcast_ref::<Vec<kedra_ui::authored::components::UiImageDensity>>().is_some();
                    if needs_image {
                        let choices = binding
                            .metadata
                            .map(|metadata| metadata.asset_choices(std::any::TypeId::of::<Image>()))
                            .unwrap_or_default();
                        command
                            .update(world, label, enabled && !choices.is_empty())
                            .map_err(|error| error.to_string())?;
                        let source = world
                            .get::<Control>(root)
                            .ok_or("Image insertion binding unavailable")?
                            .0
                            .clone();
                        crate::resources::image_insertion_picker(
                            world,
                            command.root(),
                            source,
                            choices,
                            context.templates,
                            &format!("{}/insert-image", self.scope),
                        );
                        world
                            .entity_mut(command.root())
                            .remove::<Action>()
                            .insert(ImageInsertion(root));
                    } else {
                        crate::resources::clear_insertion_picker(world, command.root());
                        world.entity_mut(command.root()).remove::<ImageInsertion>();
                    }
                }
            }
            let mut retained = Vec::new();
            for item in self.items.drain(..) {
                if state.ids.contains(&item.id) {
                    retained.push(item);
                } else {
                    item.tree.retire(world);
                    item.row.retire(world);
                }
            }
            self.items = retained;
            let ReflectRef::List(list) = state.value.as_ref().reflect_ref() else {
                return Err("Native list required".into());
            };
            for (index, value) in list.iter().enumerate() {
                let id = state.ids[index];
                let scope = format!("{}/items/{id}", self.scope);
                let mut path = source_path
                    .0
                    .iter()
                    .map(|part| part.access.clone())
                    .collect::<Vec<_>>();
                path.push(Access::ListIndex(index));
                let path = ParsedPath::from(path);
                let label = index.to_string();
                let current = ValueBinding {
                    value,
                    path: Some(&path),
                    label: &label,
                    diagnostic: None,
                    ..binding
                };
                let mut child = EditorContext {
                    scope: &scope,
                    templates: context.templates,
                    types: context.types,
                    editors: context.editors,
                    budget: &mut *context.budget,
                };
                let position = self.items.iter().position(|item| item.id == id);
                let item = if let Some(position) = position {
                    self.items.remove(position)
                } else {
                    let row = PropertyRow::mount(world, state.area, context.templates, &scope)
                        .map_err(|e| e.to_string())?;
                    class(world, row.root(), "property-collection-item", true);
                    kedra_ui::fields::insertion_marker(world, row.root());
                    let density = value
                        .try_downcast_ref::<kedra_ui::authored::components::UiImageDensity>()
                        .is_some();
                    let compound = !matches!(self.kind, Kind::Classes) && !density;
                    class(world, row.root(), "property-collection-scalar", !compound);
                    class(world, row.root(), "property-collection-compound", compound);
                    class(world, row.root(), "property-density-row", density);
                    let tree = TreeView::mount(world, row.editor(), &mut child, current, false)?;
                    class(world, tree.root(), "property-composition", true);
                    class(world, tree.root(), "property-collection-value", true);
                    let grip = CommandView::mount(
                        world,
                        row.root(),
                        context.templates,
                        &format!("{scope}/grip"),
                    )
                    .map_err(|e| e.to_string())?;
                    class(world, grip.root(), "property-array-grip", true);
                    world
                        .entity_mut(row.root())
                        .insert_children(0, &[grip.root()]);
                    let toggle = if matches!(self.kind, Kind::Shadows) {
                        let toggle = CommandView::mount(
                            world,
                            row.root(),
                            context.templates,
                            &format!("{scope}/header"),
                        )
                        .map_err(|e| e.to_string())?;
                        toggle.choice_markers(world);
                        class(world, toggle.root(), "property-shadow-header", true);
                        class(world, row.root(), "property-collection-shadow", true);
                        world
                            .entity_mut(row.root())
                            .insert_children(2, &[toggle.root()]);
                        Some(toggle)
                    } else {
                        None
                    };
                    ItemView {
                        id,
                        row,
                        tree,
                        grip,
                        toggle,
                    }
                };
                let mut item = item;
                if let Some(toggle) = &item.toggle {
                    toggle
                        .update(world, &format!("Shadow {index}"), true)
                        .map_err(|e| e.to_string())?;
                    world.entity_mut(toggle.root()).insert((
                        FieldAccess::Editable,
                        Action {
                            owner: root,
                            revision: state.revision,
                            kind: Operation::Expand(id),
                        },
                    ));
                    class(
                        world,
                        item.row.root(),
                        "collapsed",
                        !state.expanded.contains(&id),
                    );
                }
                item.row
                    .update(world, &label, None)
                    .map_err(|e| e.to_string())?;
                if state.queued == 0 || binding.access.is_read_only() {
                    item.tree.filter(&self.filter);
                    item.tree.update(world, &mut child, current, false)?;
                }
                item.grip
                    .update(world, "Select item", !binding.access.is_read_only())
                    .map_err(|e| e.to_string())?;
                world.entity_mut(item.grip.root()).insert((
                    binding.access.clone(),
                    Action {
                        owner: root,
                        revision: state.revision,
                        kind: Operation::Select(id),
                    },
                ));
                world
                    .entity_mut(item.grip.root())
                    .insert(interaction::Grip {
                        owner: root,
                        id,
                        row: item.row.root(),
                    });
                world
                    .entity_mut(item.row.root())
                    .insert(interaction::Row { owner: root, id });
                class(
                    world,
                    item.row.root(),
                    "selected",
                    state.selected == Some(id),
                );
                class(world, item.row.root(), "hidden", !item.tree.has_matches());
                self.items.insert(index, item);
            }
            let rows = self
                .items
                .iter()
                .map(|item| item.row.root())
                .collect::<Vec<_>>();
            world.entity_mut(state.area).replace_children(&rows);
            state.focus_ready = true;
            Ok(())
        })();
        world.entity_mut(root).insert(state);
        result
    }
}
