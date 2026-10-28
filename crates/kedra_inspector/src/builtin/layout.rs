//! Semantic groups over the real native Node; no parallel layout/value schema.
use super::*;
use crate::view::{Disclosure, TreeView};
use kedra_ui::authored::components::UiLayout;
use std::collections::BTreeSet;

pub(super) struct LayoutEditor;
struct LayoutView {
    section: SectionView,
    groups: Vec<(String, String, TreeView)>,
}
fn groups(node: &Node) -> Result<Vec<(String, BTreeSet<String>)>, String> {
    let bevy::reflect::ReflectRef::Struct(native) = node.reflect_ref() else {
        return Err("Native Node requires named fields".into());
    };
    let mut remaining = (0..native.field_len())
        .filter_map(|index| native.name_at(index))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut groups = Vec::new();
    for (label, names) in [
        (
            "Size",
            &[
                "width",
                "height",
                "min_width",
                "min_height",
                "max_width",
                "max_height",
                "aspect_ratio",
                "box_sizing",
            ][..],
        ),
        (
            "Position",
            &[
                "display",
                "position_type",
                "left",
                "right",
                "top",
                "bottom",
                "overflow",
                "overflow_clip_margin",
            ][..],
        ),
        (
            "Flex & alignment",
            &[
                "flex_direction",
                "flex_wrap",
                "align_items",
                "align_self",
                "align_content",
                "justify_content",
                "justify_items",
                "justify_self",
                "flex_grow",
                "flex_shrink",
                "flex_basis",
                "row_gap",
                "column_gap",
            ][..],
        ),
        ("Spacing", &["margin", "padding", "border"][..]),
        (
            "Grid",
            &[
                "grid_auto_flow",
                "grid_template_rows",
                "grid_template_columns",
                "grid_auto_rows",
                "grid_auto_columns",
                "grid_row",
                "grid_column",
            ][..],
        ),
    ] {
        let fields = names
            .iter()
            .filter(|name| remaining.remove(**name))
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        if !fields.is_empty() {
            groups.push((label.to_owned(), fields));
        }
    }
    if !remaining.is_empty() {
        groups.push(("Advanced".into(), remaining));
    }
    Ok(groups)
}
fn path(binding: ValueBinding<'_>) -> ParsedPath {
    let mut parts = binding
        .path
        .into_iter()
        .flat_map(|path| &path.0)
        .map(|part| part.access.clone())
        .collect::<Vec<_>>();
    parts.push(Access::Field("node".into()));
    ParsedPath::from(parts)
}
impl ValueEditor for LayoutEditor {
    fn layout(&self) -> EditorLayout {
        EditorLayout::Block
    }
    fn title(&self) -> Option<&'static str> {
        Some("Layout")
    }
    fn search_terms(&self) -> &'static [&'static str] {
        &[
            "Size",
            "Position",
            "Flex",
            "Alignment",
            "Spacing",
            "Grid",
            "Advanced",
        ]
    }
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let layout = binding
            .value
            .try_downcast_ref::<UiLayout>()
            .ok_or("Native UiLayout required")?;
        let section = SectionView::mount(world, parent, context.templates, context.scope)
            .map_err(|e| e.to_string())?;
        class(world, section.root(), "property-composition", true);
        let root = section.root();
        let result = (|| {
            let mut trees = Vec::new();
            let path = path(binding);
            for (index, (label, fields)) in groups(&layout.node)?.into_iter().enumerate() {
                let scope = format!("{}/layout/{index}", context.scope);
                let mut child = EditorContext {
                    scope: &scope,
                    templates: context.templates,
                    types: context.types,
                    editors: context.editors,
                    budget: &mut *context.budget,
                };
                let current = ValueBinding {
                    value: &layout.node,
                    label: &label,
                    path: Some(&path),
                    ..binding
                };
                let mut tree = TreeView::mount_fields(
                    world,
                    section.body(),
                    &mut child,
                    current,
                    false,
                    fields,
                )?;
                if label != "Size" {
                    tree.disclose(&Disclosure {
                        owner: tree.root(),
                        key: Vec::new(),
                        token: binding.token,
                    });
                }
                trees.push((label, scope, tree));
            }
            Ok(LayoutView {
                section,
                groups: trees,
            })
        })();
        match result {
            Ok(view) => Ok(Box::new(view)),
            Err(error) => {
                if let Ok(entity) = world.get_entity_mut(root) {
                    entity.despawn();
                }
                Err(error)
            }
        }
    }
}
impl ValueEditorView for LayoutView {
    fn filter(&mut self, filter: &str) {
        for (_, _, tree) in &mut self.groups {
            tree.filter(filter);
        }
    }
    fn root(&self) -> Entity {
        self.section.root()
    }
    fn owns_disclosure(&self, action: &Disclosure) -> bool {
        self.groups
            .iter()
            .any(|(_, _, tree)| tree.owns_disclosure(action))
    }
    fn disclose(&mut self, action: &Disclosure) -> bool {
        self.groups
            .iter_mut()
            .any(|(_, _, tree)| tree.disclose(action))
    }
    fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let layout = binding
            .value
            .try_downcast_ref::<UiLayout>()
            .ok_or("Native UiLayout required")?;
        let path = path(binding);
        for (label, scope, tree) in &mut self.groups {
            let mut child = EditorContext {
                scope,
                templates: context.templates,
                types: context.types,
                editors: context.editors,
                budget: &mut *context.budget,
            };
            tree.update(
                world,
                &mut child,
                ValueBinding {
                    value: &layout.node,
                    label,
                    path: Some(&path),
                    ..binding
                },
                false,
            )?;
        }
        Ok(())
    }
}
