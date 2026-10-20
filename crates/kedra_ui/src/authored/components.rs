//! Side-effect-free authored data. Bevy owns actual layout, widgets, text and picking.
use bevy::{prelude::*, reflect::std_traits::ReflectDefault};
use serde::{Deserialize, Serialize};

/// Authored native Bevy layout. Kept separate until hydration because native Node
/// installs computed presentation components and must not execute in the source world.
#[derive(Component, Clone, Default, Reflect, Serialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct UiLayout {
    /// Native Node data, not another layout solver or proprietary UI markup.
    pub node: Node,
}
impl<'de> Deserialize<'de> for UiLayout {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            node: Node,
        }
        let mut ignored = None;
        let wire: Wire = serde_ignored::deserialize(de, |path| {
            ignored.get_or_insert_with(|| path.to_string());
        })?;
        if let Some(path) = ignored {
            return Err(serde::de::Error::custom(format!(
                "unknown native UI layout field: {path}"
            )));
        }
        Ok(Self { node: wire.node })
    }
}

/// Authored content. Typography and color belong to the selected stylesheet.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct UiCaption {
    /// Static or initial text. Feature bindings update the retained native Text/EditableText.
    pub text: String,
    /// Native line justification.
    pub justify: Justify,
    /// Explicit wrapping behavior; paths/controls normally use false.
    pub wrap: bool,
}

/// One lossless native image dependency at its actual raster density.
#[derive(Clone, Reflect)]
pub struct UiImageDensity {
    /// Physical pixels per logical pixel, strictly increasing within UiImage.
    pub density: f32,
    /// Native image handle, including subasset label when appropriate.
    pub image: Handle<Image>,
}

/// Authored image with explicitly supported densities. Missing coverage is an error.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct UiImage {
    /// Ordered native density variants, not rescaled screenshot placeholders.
    pub variants: Vec<UiImageDensity>,
    /// Semantic tint, normally white for an unchanged SVG export.
    pub color: Color,
}

/// Pointer/keyboard button behavior. Stylesheets own all visual states.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct UiButton;

/// Inert stylesheet dependency. Live hydration installs native Flair `Styled`.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct UiStylesheet {
    /// Native CSS asset, whose imports, fonts and images are ordinary dependencies.
    pub asset: Handle<bevy_flair::style::StyleSheet>,
}

/// Explicit CSS classes, separate from persistent source and observation IDs.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct UiClasses {
    /// Ordered author-owned class names, validated before instantiation.
    pub names: Vec<String>,
}

/// Native single-line editing behavior; a UiCaption on the same node supplies font/style.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct UiEditable {
    /// Request initial focus only when this instance is first hydrated.
    pub autofocus: bool,
    /// Select existing content on focus, without replacing its editor state.
    pub select_all: bool,
}

/// Native modal ownership on a pre-authored hierarchy; no hand-built modal tree.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub enum UiModal {
    /// Dialog content owns a modal native TabGroup.
    Dialog,
    /// Barrier catches outside clicks and native dismiss requests.
    Barrier,
}

/// UI-only real-time spin. Gameplay clocks and source editing are separate mechanisms.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct UiSpin {
    /// Positive real-time seconds per full revolution.
    pub seconds: f32,
    /// Feature-owned presentation pause, for example after an asynchronous failure.
    pub paused: bool,
}

/// Host presentation preference, not a simulation-clock override.
#[derive(Resource, Default)]
pub struct UiMotion {
    /// Keep UI motion stationary; normal animation requires its own acceptance evidence.
    pub reduced: bool,
}

/// Hydrated UI root, distinct from source compilation and GPU/frame readiness.
#[derive(Component)]
pub struct UiReady;

/// UI materialization/render-preparation failure. Consumers must report it explicitly.
#[derive(Resource, Default)]
pub struct UiFailure(pub Option<String>);

/// Passive instance-local presentation status. Passive projection never routes
/// resource failures into the serving application's global UI failure channel.
#[derive(Component, Default)]
pub struct PassiveUi {
    /// Latest native hydration/resource failure on this node, if any.
    pub error: Option<String>,
}
