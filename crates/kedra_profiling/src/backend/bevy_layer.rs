//! A narrow Bevy span adapter, not a recorder: SDK owns encoding, clocks and buffers.
//! Retain only diagnostic scope identity. Repeated source-location/argument emission
//! in the generic upstream adapter is unsuitable for a bounded per-frame capture.
use bevy::log::tracing::{
    Subscriber,
    field::{Field, Visit},
    span,
};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicU64, Ordering},
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

pub(super) static DROPPED: AtomicU64 = AtomicU64::new(0);
pub(crate) struct BevyLayer;
struct Label {
    kind: &'static str,
    name: String,
    target: &'static str,
}

impl Visit for Label {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" {
            self.name = bounded(&format!("{value:?}"));
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.name = bounded(value);
        }
    }
}

fn category(label: &Label) -> crate::Category {
    use crate::Category;
    let name = if label.name.is_empty() {
        label.target
    } else {
        label.name.trim_matches('"')
    };
    if [
        "multithreaded executor",
        "par_for_each",
        "check_conditions",
        "schedule",
    ]
    .contains(&label.kind)
        || name.starts_with("bevy_ecs::schedule")
    {
        return Category::Scheduling;
    }
    if name.starts_with("bevy_ui") || name.starts_with("bevy_flair") || name.starts_with("kedra_ui")
    {
        return Category::Ui;
    }
    if [
        "bevy_render",
        "bevy_pbr",
        "bevy_core_pipeline",
        "bevy_sprite_render",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
    {
        return Category::Rendering;
    }
    if [
        "bevy_asset",
        "kedra_assets",
        "kedra_scene::assets",
        "kedra_import",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
    {
        return Category::Assets;
    }
    if name.starts_with("kedra_script") || name.starts_with("bevy_mod_scripting") {
        return Category::Scripting;
    }
    if name.starts_with("kedra_editor") {
        return Category::Editor;
    }
    if name.starts_with("bevy_") {
        return Category::Runtime;
    }
    Category::Application
}
fn bounded(value: &str) -> String {
    let mut end = value.len().min(1024);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = value[..end].to_owned();
    if end < value.len() {
        result.push('…');
    }
    result
}

impl Label {
    fn display(&self) -> String {
        if self.name.is_empty() {
            self.kind.into()
        } else {
            format!("{}: {}", self.kind, self.name)
        }
    }
}

#[derive(Default)]
struct Entered {
    scopes: Vec<(u64, super::markers::Scope)>,
    overflow: usize,
}
thread_local! { static ENTERED: RefCell<Entered> = RefCell::new(Entered::default()); }

impl<S> Layer<S> for BevyLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attributes: &span::Attributes<'_>,
        id: &span::Id,
        context: Context<'_, S>,
    ) {
        let Some(span) = context.span(id) else {
            return;
        };
        let mut label = Label {
            kind: attributes.metadata().name(),
            name: String::new(),
            target: attributes.metadata().target(),
        };
        attributes.record(&mut label);
        span.extensions_mut().insert(label);
    }
    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, context: Context<'_, S>) {
        if let Some(span) = context.span(id)
            && let Some(label) = span.extensions_mut().get_mut::<Label>()
        {
            values.record(label);
        }
    }
    fn on_enter(&self, id: &span::Id, context: Context<'_, S>) {
        ENTERED.with(|entered| {
            let mut entered = entered.borrow_mut();
            if entered.overflow > 0 || entered.scopes.len() >= 256 {
                entered.overflow = entered.overflow.saturating_add(1);
                DROPPED.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let label = context.span(id).and_then(|span| {
                span.extensions()
                    .get::<Label>()
                    .map(|label| (label.display(), category(label)))
            });
            let (name, category) =
                label.unwrap_or_else(|| ("Bevy scope".into(), crate::Category::Runtime));
            let scope = super::markers::enter_in(&name, category);
            entered.scopes.push((id.into_u64(), scope));
        });
    }
    fn on_exit(&self, id: &span::Id, _context: Context<'_, S>) {
        ENTERED.with(|entered| {
            let mut entered = entered.borrow_mut();
            if entered.overflow > 0 {
                entered.overflow -= 1;
                return;
            }
            if let Some((expected, scope)) = entered.scopes.pop() {
                if expected != id.into_u64() {
                    DROPPED.fetch_add(1, Ordering::Relaxed);
                }
                drop(scope);
            } else {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_scopes_remain_detail_even_with_an_application_schedule_name() {
        let label = Label {
            kind: "schedule",
            name: "MyGameplay".into(),
            target: "bevy_ecs::schedule",
        };
        assert_eq!(category(&label), crate::Category::Scheduling);
        assert_eq!(label.display(), "schedule: MyGameplay");
    }

    #[test]
    fn quoted_system_identity_classifies_without_shortening_the_original_name() {
        let name = "\"bevy_ui_render::prepare_uinodes\"";
        let label = Label {
            kind: "system",
            name: name.into(),
            target: "bevy_ecs",
        };
        assert_eq!(category(&label), crate::Category::Ui);
        assert_eq!(label.display(), format!("system: {name}"));
        let custom = Label {
            kind: "system",
            name: "game::combat".into(),
            target: "bevy_ecs",
        };
        assert_eq!(category(&custom), crate::Category::Application);
    }

    #[test]
    fn oversized_diagnostic_identity_preserves_utf8_at_the_byte_boundary() {
        let label = bounded(&"界".repeat(400));
        assert_eq!(label, format!("{}…", "界".repeat(341)));
    }
}
