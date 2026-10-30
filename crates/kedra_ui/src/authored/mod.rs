//! Shared authored UI hydration and native behavior, used by games and editor alike.
pub mod components;
mod style;
pub(crate) use style::sync_editing;
pub mod view;
use bevy::{
    prelude::*,
    reflect::{PartialReflect, ReflectRef},
};
use components::*;
use kedra_scene::reflection::ComponentPolicies;

// One declaration drives source registration and passive-preview admission. New
// entries must be inert source data with audited passive hydration, not behavior.
macro_rules! passive_schema {
    ($policies:ident, $types:ident; $(
        $component:ty, $key:literal, $validator:expr;
    )*) => {
        /// Whether this key belongs to the shared inert UI schema supported by
        /// [`view::UiView::mount_passive`]. This does not validate values/versions
        /// or authorize native behavior, host bindings, scripts or diagnostics.
        /// Host-specific schemas (including docking) are deliberately excluded.
        pub fn supports_passive_component(key: &str) -> bool {
            matches!(key, $($key)|*)
        }

        fn register_schema(
            $policies: &mut ComponentPolicies,
            $types: &mut bevy::reflect::TypeRegistry,
        ) -> kedra_scene::Result<()> {
            $($policies.register::<$component>($types, $key, 1, $validator)?;)*
            Ok(())
        }
    };
}

/// Installs authored component policies and retained native styling. The host must
/// already install Bevy's UI, text, input-focus, picking and widget plugins.
pub struct AuthoredUiPlugin;
impl Plugin for AuthoredUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ComponentPolicies>()
            .init_resource::<UiMotion>()
            .init_resource::<UiFailure>();
        app.add_plugins(crate::styles::StylesPlugin);
        // Full native hosts use the shared window-owned input adapter. Headless
        // asset/schema consumers do not install input or allocate native windows.
        if app.is_plugin_added::<bevy::input_focus::InputDispatchPlugin>()
            && app.is_plugin_added::<bevy::ui_widgets::TextInputPlugin>()
            && !app.is_plugin_added::<crate::native_input::NativeInputPlugin>()
        {
            app.add_plugins(crate::native_input::NativeInputPlugin);
        }
        let registry = app.world().resource::<AppTypeRegistry>().clone();
        let registration = {
            let mut policies = app.world_mut().resource_mut::<ComponentPolicies>();
            let mut types = registry.write();
            register_schema(&mut policies, &mut types)
        };
        if let Err(error) = registration {
            app.world_mut().resource_mut::<UiFailure>().0 = Some(error.to_string());
        }
        app.add_systems(Update, (style::editing, style::images, style::spin));
        if app.is_plugin_added::<bevy::input_focus::InputDispatchPlugin>() {
            if !app.is_plugin_added::<crate::popup::cascade::CascadePlugin>() {
                app.add_plugins(crate::popup::cascade::CascadePlugin);
            }
            app.add_systems(
                PostUpdate,
                crate::popup::constrain.before(bevy::ui::UiSystems::Layout),
            );
        }
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "render")]
        crate::render_clipping::install(_app);
        #[cfg(feature = "render")]
        crate::wake_render::install(_app);
    }
}

passive_schema! { policies, types;
    UiLayout, "ui.layout", finite;
    UiCaption, "ui.caption", finite;
    UiStylesheet, "ui.stylesheet", finite;
    UiClasses, "ui.classes", |value| {
        let classes = UiClasses::from_reflect(value)
            .ok_or_else(|| kedra_scene::Error("invalid classes".into()))?;
        let mut unique = std::collections::BTreeSet::new();
        if classes.names.len() > 64
            || classes.names.iter().any(|name| {
                name.is_empty()
                    || name.len() > 256
                    || name.chars().any(char::is_whitespace)
                    || !unique.insert(name)
            })
        {
            return Err(kedra_scene::Error(
                "CSS classes must be unique, bounded and contain no whitespace".into(),
            ));
        }
        Ok(())
    };
    UiImage, "ui.image", |value| {
        finite(value)?;
        let v = UiImage::from_reflect(value)
            .ok_or_else(|| kedra_scene::Error("invalid image".into()))?;
        if v.variants.is_empty()
            || v.variants.len() > 8
            || v.variants.windows(2).any(|p| p[0].density >= p[1].density)
            || v.variants.iter().any(|p| p.density <= 0.)
        {
            return Err(kedra_scene::Error(
                "image densities must be nonempty, positive and ordered".into(),
            ));
        }
        Ok(())
    };
    UiButton, "ui.button", finite;
    UiEditable, "ui.editable", finite;
    UiModal, "ui.modal", finite;
    UiSpin, "ui.spin", |value| {
        finite(value)?;
        if UiSpin::from_reflect(value).is_none_or(|v| v.seconds <= 0.) {
            return Err(kedra_scene::Error("spin duration must be positive".into()));
        }
        Ok(())
    };
    crate::ElementId, "ui.id", |value| {
        let id = crate::ElementId::from_reflect(value)
            .ok_or_else(|| kedra_scene::Error("invalid UI identity".into()))?;
        if id.0.trim().is_empty() || id.0.len() > 256 {
            return Err(kedra_scene::Error(
                "UI identity must contain 1..=256 bytes".into(),
            ));
        }
        Ok(())
    };
    crate::ScreenId, "ui.screen", |value| {
        let id = crate::ScreenId::from_reflect(value)
            .ok_or_else(|| kedra_scene::Error("invalid screen identity".into()))?;
        if id.0.trim().is_empty() || id.0.len() > 256 {
            return Err(kedra_scene::Error(
                "Screen identity must contain 1..=256 bytes".into(),
            ));
        }
        Ok(())
    };
    BackgroundColor, "ui.background", finite;
    BorderColor, "ui.border", finite;
    BoxShadow, "ui.shadow", finite;
    ZIndex, "ui.z", finite;
    GlobalZIndex, "ui.global_z", finite;
    Outline, "ui.outline", finite;
    // Bevy's existing picking policy, not a parallel interaction/hit-test path.
    bevy::picking::Pickable, "ui.pickable", finite;
}

// Native reflected numeric fields are validated through Bevy's data model. This
// is a source-policy visitor, not a parser, layout engine or reflection registry.
fn finite(value: &dyn PartialReflect) -> kedra_scene::Result<()> {
    fn visit(value: &dyn PartialReflect, depth: usize) -> kedra_scene::Result<()> {
        if depth > 128 {
            return Err(kedra_scene::Error(
                "UI value exceeds reflection depth limit".into(),
            ));
        }
        if value
            .try_downcast_ref::<f32>()
            .is_some_and(|v| !v.is_finite())
            || value
                .try_downcast_ref::<f64>()
                .is_some_and(|v| !v.is_finite())
        {
            return Err(kedra_scene::Error(
                "UI numeric fields must be finite".into(),
            ));
        }
        let children: Vec<&dyn PartialReflect> = match value.reflect_ref() {
            ReflectRef::Struct(v) => (0..v.field_len()).filter_map(|i| v.field_at(i)).collect(),
            ReflectRef::TupleStruct(v) => (0..v.field_len()).filter_map(|i| v.field(i)).collect(),
            ReflectRef::Tuple(v) => (0..v.field_len()).filter_map(|i| v.field(i)).collect(),
            ReflectRef::Enum(v) => (0..v.field_len()).filter_map(|i| v.field_at(i)).collect(),
            ReflectRef::List(v) => v.iter().collect(),
            ReflectRef::Array(v) => v.iter().collect(),
            ReflectRef::Map(v) => v.iter().flat_map(|(k, v)| [k, v]).collect(),
            ReflectRef::Set(v) => v.iter().collect(),
            _ => Vec::new(),
        };
        for child in children {
            visit(child, depth + 1)?;
        }
        Ok(())
    }
    visit(value, 0)
}
