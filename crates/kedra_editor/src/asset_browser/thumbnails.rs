//! Visible retained tiles demand snapshots; no native assets load in the browser.
use super::*;
use kedra_preview::thumbnails::{Kind as PreviewKind, Request, Thumbnails};

#[derive(Component)]
struct Thumbnail;
impl Panel {
    pub(crate) fn thumbnail_demand(
        &self,
        world: &mut World,
        revision: &str,
    ) -> Vec<(Entity, Entity, Request)> {
        let mut result = Vec::new();
        let viewport = self
            .element("entries")
            .ok()
            .and_then(|e| kedra_ui::docking::rect(world, e));
        for (name, row) in &self.rows {
            let Ok(preview) = row.element("preview") else {
                continue;
            };
            let Ok(badge) = row.element("badge") else {
                continue;
            };
            let existing = world.get::<Children>(preview).and_then(|children| {
                children
                    .iter()
                    .find(|e| world.get::<Thumbnail>(*e).is_some())
            });
            let identity = format!(
                "{}/thumbnail",
                world
                    .get::<kedra_ui::ElementId>(row.root)
                    .map_or("asset", |v| v.0.as_str())
            );
            let image = existing.unwrap_or_else(|| {
                world
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(72.),
                            height: Val::Px(72.),
                            ..default()
                        },
                        ImageNode::default(),
                        Thumbnail,
                        Visibility::Hidden,
                        bevy::picking::Pickable::IGNORE,
                        ChildOf(preview),
                        kedra_ui::ElementId::new(identity.clone()),
                    ))
                    .id()
            });
            if world
                .get::<kedra_ui::ElementId>(image)
                .is_none_or(|id| id.0 != identity)
            {
                world
                    .entity_mut(image)
                    .insert(kedra_ui::ElementId::new(identity));
            }
            let visible = self.model.mode == model::Mode::Grid
                && !self.model.loading
                && self.model.error.is_none()
                && viewport
                    .zip(kedra_ui::docking::rect(world, row.root))
                    .is_some_and(|(a, b)| {
                        a.width() > 0.
                            && a.height() > 0.
                            && a.intersect(b).width() > 0.
                            && a.intersect(b).height() > 0.
                    });
            let path = self.model.path.join(name);
            let kind = match path.extension().and_then(|s| s.to_str()) {
                Some("png") => Some(PreviewKind::Image),
                Some("glb" | "gltf") => Some(PreviewKind::Model),
                _ => None,
            };
            if visible
                && !revision.is_empty()
                && let Some(kind) = kind
                && let Some(path) = path.to_str()
            {
                result.push((
                    image,
                    badge,
                    Request {
                        path: format!("game://{}", path.replace(std::path::MAIN_SEPARATOR, "/")),
                        revision: revision.into(),
                        kind,
                    },
                ));
            } else {
                show(world, image, badge, None);
            }
        }
        result
    }
}
pub(crate) fn apply(world: &mut World, demand: &[(Entity, Entity, Request)], cache: &Thumbnails) {
    for (image, badge, request) in demand {
        show(world, *image, *badge, cache.image(request));
    }
}
pub(crate) fn show(world: &mut World, entity: Entity, badge: Entity, image: Option<Handle<Image>>) {
    let present = image.is_some();
    if let Some(image) = image {
        let size = world
            .resource::<Assets<Image>>()
            .get(&image)
            .map(|i| i.size().as_vec2())
            .unwrap_or(Vec2::ONE);
        let size = size / size.max_element() * 72.;
        if let Some(mut node) = world.get_mut::<Node>(entity)
            && (node.width != Val::Px(size.x) || node.height != Val::Px(size.y))
        {
            node.width = Val::Px(size.x);
            node.height = Val::Px(size.y);
        }
        if let Some(mut node) = world.get_mut::<ImageNode>(entity)
            && node.image != image
        {
            node.image = image;
        }
    } else if let Some(mut node) = world.get_mut::<ImageNode>(entity)
        && node.image != Handle::default()
    {
        node.image = default();
    }
    let desired = if present {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if let Some(mut visibility) = world.get_mut::<Visibility>(entity)
        && *visibility != desired
    {
        *visibility = desired;
    }
    kedra_ui::docking::class(world, badge, "hidden", present);
}
