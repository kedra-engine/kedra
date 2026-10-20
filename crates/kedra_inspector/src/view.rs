//! Retained default/custom composition using only shared authored property controls.
use crate::{
    binding::*,
    discovery::{Limits, inspect},
    editors::*,
    value,
};
use bevy::{
    prelude::*,
    reflect::{PartialReflect, ReflectRef},
};
use kedra_ui::fields::{PropertyRow, SectionView, ValueView};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Native disclosure binding. Hosts route activation to the owning `TreeView`.
#[derive(Component, Clone)]
pub struct Disclosure {
    /// Retained presentation root, not an inspected source entity.
    pub owner: Entity,
    /// Structural key valid only for this view's current host token.
    pub key: Vec<usize>,
    /// Reject delayed actions from another target/revision.
    pub token: BindingToken,
}

enum NodeView {
    Group(SectionView),
    Leaf(PropertyRow, ValueView),
    Custom(PropertyRow, Box<dyn ValueEditorView>, Arc<dyn ValueEditor>),
    Failed(PropertyRow, ValueView, Arc<dyn ValueEditor>, String),
}
impl NodeView {
    fn root(&self) -> Entity {
        match self {
            Self::Group(s) => s.root(),
            Self::Leaf(r, _) | Self::Custom(r, _, _) | Self::Failed(r, _, _, _) => r.root(),
        }
    }
    fn retire(&self, world: &mut World) {
        if let Self::Custom(_, custom, _) = self {
            custom.retire(world);
        }
        if let Ok(root) = world.get_entity_mut(self.root()) {
            root.despawn();
        }
    }
}

/// One immutable value's retained presentation, usable outside editor documents.
/// The host owns target lifetime, expansion/search state and source write authority.
pub struct TreeView {
    frame: SectionView,
    nodes: BTreeMap<Vec<usize>, NodeView>,
    collapsed: BTreeSet<Vec<usize>>,
    token: BindingToken,
    filter: String,
    matched: bool,
    incomplete: bool,
}
impl TreeView {
    /// Mount a value tree. On failure all newly owned UI is reclaimed.
    pub fn mount(
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
        default_mode: bool,
    ) -> Result<Self, String> {
        Self::mount_filtered(world, parent, context, binding, default_mode, "")
    }
    /// Mount with an already active query, avoiding an unfiltered intermediate view
    /// or a second discovery pass when a host changes target while searching.
    pub fn mount_filtered(
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
        default_mode: bool,
        filter: &str,
    ) -> Result<Self, String> {
        let frame = SectionView::mount(world, parent, context.templates, context.scope)
            .map_err(|e| e.to_string())?;
        let mut view = Self {
            frame,
            nodes: BTreeMap::new(),
            collapsed: BTreeSet::new(),
            token: binding.token,
            filter: filter.to_lowercase(),
            matched: false,
            incomplete: false,
        };
        if let Err(error) = view.update(world, context, binding, default_mode) {
            view.retire(world);
            return Err(error);
        }
        Ok(view)
    }
    /// Owned presentation root, stable through refresh and dock host movement.
    pub fn root(&self) -> Entity {
        self.frame.root()
    }
    /// Toggle a native disclosure only if its retained owner/token are still current.
    pub fn disclose(&mut self, action: &Disclosure) -> bool {
        if action.owner != self.root() || action.token != self.token {
            return false;
        }
        if !self.collapsed.remove(&action.key) {
            self.collapsed.insert(action.key.clone());
        }
        true
    }
    /// Filter only presentation; ancestors remain visible and values are never mutated.
    pub fn filter(&mut self, filter: &str) {
        self.filter = filter.to_lowercase();
    }
    /// Whether the last bounded rebuild found a property/component matching its query.
    /// A host can show explicit no-results guidance without walking source values again.
    pub fn has_matches(&self) -> bool {
        self.matched
    }
    /// Whether traversal omitted values in the last rebuild. Search covers only
    /// discovered content and must not be presented as a complete-source query.
    pub fn search_incomplete(&self) -> bool {
        self.incomplete
    }
    /// Refresh default and custom controls in place. Explicit default mode bypasses
    /// all custom presentations in this subtree, retaining source and access semantics.
    pub fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
        default_mode: bool,
    ) -> Result<(), String> {
        if context.budget.depth >= 32 || context.budget.remaining == 0 {
            return Err("Inspector composition limit reached".into());
        }
        context.budget.depth += 1;
        let result = self.refresh(world, context, binding, default_mode);
        context.budget.depth -= 1;
        result
    }
    fn refresh(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
        default_mode: bool,
    ) -> Result<(), String> {
        if self.token != binding.token {
            for node in self.nodes.values() {
                node.retire(world);
            }
            self.nodes.clear();
            self.collapsed.clear();
            self.token = binding.token;
        }
        let title = if binding
            .metadata
            .and_then(|m| m.limitation(binding.value, binding.path))
            .is_none()
            && let ReflectRef::Enum(value) = binding.value.reflect_ref()
        {
            format!("{}: {}", binding.label, value.variant_name())
        } else {
            binding.label.into()
        };
        self.frame
            .update(
                world,
                &title,
                !self.collapsed.contains(&Vec::new()) || !self.filter.is_empty(),
            )
            .map_err(|e| e.to_string())?;
        world.entity_mut(self.frame.toggle()).insert(Disclosure {
            owner: self.root(),
            key: Vec::new(),
            token: self.token,
        });
        let rows = inspect(
            binding.value,
            context.types,
            Limits {
                rows: context.budget.remaining,
                ..Default::default()
            },
        );
        context.budget.remaining = context.budget.remaining.saturating_sub(rows.len());
        self.incomplete = rows.iter().any(|row| row.incomplete);
        if self.incomplete && !self.filter.is_empty() {
            self.frame
                .update(
                    world,
                    &format!("{title} · Search incomplete: inspection limit reached"),
                    true,
                )
                .map_err(|e| e.to_string())?;
        }
        let mut desired = BTreeSet::new();
        let mut skip: Option<Vec<usize>> = None;
        let mut attachments: BTreeMap<Entity, Vec<Entity>> = BTreeMap::new();
        let mut visible = BTreeSet::new();
        let root_match = binding.label.to_lowercase().contains(&self.filter);
        let mut matched = vec![false; rows.len()];
        for (index, row) in rows.iter().enumerate() {
            matched[index] = root_match
                || row.parent.is_some_and(|i| matched[i])
                || row.label.to_lowercase().contains(&self.filter)
                || value::display(row.value, 4096)
                    .text
                    .to_lowercase()
                    .contains(&self.filter);
            if matched[index] {
                let mut ancestor = Some(index);
                while let Some(i) = ancestor {
                    visible.insert(rows[i].key.clone());
                    ancestor = rows[i].parent;
                }
            }
        }
        self.matched = !visible.is_empty();
        kedra_ui::docking::class(
            world,
            self.root(),
            "hidden",
            !self.matched && !self.incomplete,
        );
        for (index, row) in rows.iter().enumerate() {
            if skip
                .as_ref()
                .is_some_and(|key| row.key.len() > key.len() && row.key.starts_with(key))
            {
                continue;
            }
            skip = None;
            let ty = row.value.get_represented_type_info().map(|t| t.type_id());
            let owner = binding.owner_type.or_else(|| {
                binding
                    .value
                    .get_represented_type_info()
                    .map(|t| t.type_id())
            });
            let path = row.path.as_ref().map(|path| {
                let accesses = binding
                    .path
                    .into_iter()
                    .flat_map(|p| p.0.iter())
                    .chain(path.0.iter())
                    .map(|p| p.access.clone())
                    .collect::<Vec<_>>();
                bevy::reflect::ParsedPath::from(accesses)
            });
            let selection = context
                .editors
                .resolve(owner, path.as_ref(), ty, default_mode);
            let limitation = binding
                .metadata
                .and_then(|m| m.limitation(row.value, path.as_ref()));
            let description = binding.metadata.and_then(|m| m.describe(path.as_ref()));
            let group = structural(row.value) && selection.is_none() && limitation.is_none();
            if index == 0 && group && row.diagnostic.is_none() {
                continue;
            }
            let parent = row
                .parent
                .and_then(|i| self.nodes.get(&rows[i].key))
                .and_then(|view| match view {
                    NodeView::Group(s) => Some(s.body()),
                    _ => None,
                })
                .unwrap_or(self.frame.body());
            let label = if index == 0 {
                binding.label
            } else {
                &row.label
            };
            let scope = format!("{}/{:?}", context.scope, row.key);
            let current = ValueBinding {
                value: row.value,
                label,
                path: path.as_ref(),
                owner_type: owner,
                ..binding
            };
            let compatible =
                self.nodes
                    .get(&row.key)
                    .is_some_and(|node| match (node, &selection) {
                        (
                            NodeView::Custom(_, _, old) | NodeView::Failed(_, _, old, _),
                            Some(new),
                        ) => Arc::ptr_eq(old, new.editor),
                        (NodeView::Group(_), None) => group,
                        (NodeView::Leaf(_, _), None) => !group,
                        _ => false,
                    });
            if !compatible && let Some(old) = self.nodes.remove(&row.key) {
                old.retire(world);
            }
            if !self.nodes.contains_key(&row.key) {
                let node = if let Some(selection) = &selection {
                    let mut row_view = PropertyRow::mount(world, parent, context.templates, &scope)
                        .map_err(|e| e.to_string())?;
                    let mut child_context = EditorContext {
                        templates: context.templates,
                        types: context.types,
                        editors: context.editors,
                        budget: &mut *context.budget,
                        scope: &scope,
                    };
                    match selection.editor.mount(
                        world,
                        row_view.editor(),
                        &mut child_context,
                        current,
                    ) {
                        Ok(custom) => NodeView::Custom(row_view, custom, selection.editor.clone()),
                        Err(error) => {
                            row_view.retire(world);
                            row_view = PropertyRow::mount(world, parent, context.templates, &scope)
                                .map_err(|e| e.to_string())?;
                            let value = match ValueView::mount(
                                world,
                                row_view.editor(),
                                context.templates,
                                &scope,
                            ) {
                                Ok(value) => value,
                                Err(error) => {
                                    row_view.retire(world);
                                    return Err(error.to_string());
                                }
                            };
                            NodeView::Failed(row_view, value, selection.editor.clone(), error)
                        }
                    }
                } else if group {
                    NodeView::Group(
                        SectionView::mount(world, parent, context.templates, &scope)
                            .map_err(|e| e.to_string())?,
                    )
                } else {
                    let row_view = PropertyRow::mount(world, parent, context.templates, &scope)
                        .map_err(|e| e.to_string())?;
                    let value =
                        match ValueView::mount(world, row_view.editor(), context.templates, &scope)
                        {
                            Ok(value) => value,
                            Err(error) => {
                                row_view.retire(world);
                                return Err(error.to_string());
                            }
                        };
                    NodeView::Leaf(row_view, value)
                };
                self.nodes.insert(row.key.clone(), node);
            }
            let node = self
                .nodes
                .get_mut(&row.key)
                .ok_or("Retained inspector row disappeared")?;
            match node {
                NodeView::Group(section) => {
                    let summary = value::display(row.value, 4096);
                    let title = if matches!(row.value.reflect_ref(), ReflectRef::Enum(_)) {
                        format!("{label}: {}", summary.text)
                    } else {
                        label.into()
                    };
                    section
                        .update(
                            world,
                            &title,
                            !self.collapsed.contains(&row.key) || !self.filter.is_empty(),
                        )
                        .map_err(|e| e.to_string())?;
                    world.entity_mut(section.toggle()).insert(Disclosure {
                        owner: self.frame.root(),
                        key: row.key.clone(),
                        token: self.token,
                    });
                    if let Some(diagnostic) = &row.diagnostic {
                        section
                            .update(
                                world,
                                &format!("{title} · {diagnostic}"),
                                !self.collapsed.contains(&row.key) || !self.filter.is_empty(),
                            )
                            .map_err(|e| e.to_string())?;
                    }
                }
                NodeView::Leaf(row_view, value) => {
                    let preview = value::display(row.value, 4096);
                    let diagnostic = if preview.truncated {
                        Some("Truncated preview; copying does not yield the complete source value")
                    } else if preview.escaped {
                        Some("Escaped single-line preview")
                    } else {
                        limitation
                            .as_deref()
                            .or(row.diagnostic.as_deref())
                            .or(binding.diagnostic)
                    };
                    row_view
                        .update(world, label, diagnostic)
                        .map_err(|e| e.to_string())?;
                    value
                        .update(
                            world,
                            if limitation.is_some() {
                                "Source reference"
                            } else {
                                &preview.text
                            },
                            binding.access,
                        )
                        .map_err(|e| e.to_string())?;
                    let kedra_ui::fields::FieldAccess::ReadOnly { reason } = binding.access;
                    value
                        .describe(
                            world,
                            label,
                            &format!(
                                "{}\n{}\n{}\n{}",
                                row.value.reflect_type_path(),
                                reason,
                                description.as_deref().unwrap_or(""),
                                diagnostic.unwrap_or("")
                            ),
                        )
                        .map_err(|e| e.to_string())?;
                    if limitation.is_some() {
                        skip = Some(row.key.clone());
                    }
                }
                NodeView::Custom(row_view, custom, _) => {
                    let mut child_context = EditorContext {
                        templates: context.templates,
                        types: context.types,
                        editors: context.editors,
                        budget: &mut *context.budget,
                        scope: &scope,
                    };
                    let diagnostic = custom
                        .update(world, &mut child_context, current)
                        .err()
                        .map(|e| format!("Custom editor failed: {e}. Use default view."));
                    row_view
                        .update(world, label, diagnostic.as_deref().or(binding.diagnostic))
                        .map_err(|e| e.to_string())?;
                    skip = Some(row.key.clone());
                }
                NodeView::Failed(row_view, value, _, error) => {
                    row_view
                        .update(
                            world,
                            label,
                            Some(&format!("Custom editor failed: {error}. Use default view.")),
                        )
                        .map_err(|e| e.to_string())?;
                    value
                        .update(world, "Presentation unavailable", binding.access)
                        .map_err(|e| e.to_string())?;
                    skip = Some(row.key.clone());
                }
            }
            kedra_ui::docking::class(
                world,
                node.root(),
                "hidden",
                !self.filter.is_empty() && !visible.contains(&row.key),
            );
            attachments.entry(parent).or_default().push(node.root());
            desired.insert(row.key.clone());
        }
        self.nodes.retain(|key, view| {
            if desired.contains(key) {
                true
            } else {
                view.retire(world);
                false
            }
        });
        for (parent, children) in attachments {
            world.entity_mut(parent).replace_children(&children);
        }
        Ok(())
    }
    /// Idempotently retire all owned UI, including custom editor cleanup.
    pub fn retire(&self, world: &mut World) {
        for node in self.nodes.values() {
            if let NodeView::Custom(_, view, _) = node {
                view.retire(world);
            }
        }
        self.frame.retire(world);
    }
}
fn structural(value: &dyn PartialReflect) -> bool {
    matches!(
        value.reflect_ref(),
        ReflectRef::Struct(_)
            | ReflectRef::Tuple(_)
            | ReflectRef::TupleStruct(_)
            | ReflectRef::Enum(_)
            | ReflectRef::Array(_)
            | ReflectRef::List(_)
            | ReflectRef::Map(_)
            | ReflectRef::Set(_)
    )
}
