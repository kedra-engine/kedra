//! Native reflected leaf semantics shared by scripted and native presentations.
use crate::binding::{BindingToken, ValueBinding};
use bevy::{
    prelude::*,
    reflect::{
        ParsedPath, ReflectPath, ReflectRef, TypeInfo,
        enums::{DynamicEnum, DynamicVariant, VariantInfo},
    },
};
mod color;
mod numbers;

/// Presentation metadata for one source-neutral, exact-type native leaf.
/// The consumer must separately bind its path/token to an authorized source.
#[derive(Component, Clone)]
pub struct ReflectedField {
    /// Owning presentation subtree, never an inspected source entity.
    pub owner: Entity,
    /// Shared row diagnostic target.
    pub diagnostic: Entity,
    /// Consumer's original binding identity.
    pub token: BindingToken,
    /// Complete native path relative to the inspected component.
    pub path: ParsedPath,
    scalar: Scalar,
}
#[derive(Clone)]
enum Scalar {
    Text(String),
    Bool(bool),
    Number(numbers::Number),
    Color(color::ColorValue),
    ColorChannel { value: Color, axis: usize },
    Length(Val),
    Vector { value: Vec3, axis: usize },
    UnitEnum(&'static TypeInfo, String),
}
impl ReflectedField {
    /// Bind the amount of a native UI length while retaining its exact unit and
    /// owning source path. Auto is displayable but rejects numeric proposals.
    pub fn length(
        input: Entity,
        diagnostic: Entity,
        binding: ValueBinding<'_>,
    ) -> Result<Self, String> {
        let value = *binding
            .value
            .try_downcast_ref::<Val>()
            .ok_or("Native Val required")?;
        if let ReflectRef::Enum(native) = value.reflect_ref()
            && native
                .field_at(0)
                .and_then(|field| field.try_downcast_ref::<f32>())
                .is_some_and(|amount| !amount.is_finite())
        {
            return Err("Length requires a finite native amount.".into());
        }
        Ok(Self {
            owner: input,
            diagnostic,
            token: binding.token,
            path: binding
                .path
                .cloned()
                .ok_or("Length requires an owning field path")?,
            scalar: Scalar::Length(value),
        })
    }
    /// Bind an exact native RGBA channel in its current sRGB/linear representation.
    /// Channels use 0..=1, not a quantized 8-bit display. The complete Color proposal
    /// preserves all untouched channels and its native storage variant.
    pub fn color_channel(
        input: Entity,
        diagnostic: Entity,
        binding: ValueBinding<'_>,
        axis: usize,
    ) -> Result<Self, String> {
        let value = *binding
            .value
            .try_downcast_ref::<Color>()
            .ok_or("Native Color required")?;
        color::ColorValue::bind(value)?;
        if axis >= 4 {
            return Err("Color channel must be in 0..4".into());
        }
        Ok(Self {
            owner: input,
            diagnostic,
            token: binding.token,
            path: binding
                .path
                .cloned()
                .ok_or("Color requires an owning native field path")?,
            scalar: Scalar::ColorChannel { value, axis },
        })
    }
    /// Bind a single-line String, exact native number, bool or all-unit native enum.
    /// Missing paths, payload enums and unsupported native types reject without mutation.
    pub fn bind(
        owner: Entity,
        diagnostic: Entity,
        binding: ValueBinding<'_>,
        relative: &ParsedPath,
    ) -> Result<Self, String> {
        let value = relative
            .reflect_element(binding.value)
            .map_err(|error| error.to_string())?;
        let scalar = if let Some(text) = value.try_downcast_ref::<String>() {
            if text.chars().count() > 4096 || text.chars().any(|ch| matches!(ch, '\n' | '\r')) {
                return Err("This field supports at most 4096 single-line characters; use default view for the complete value.".into());
            }
            Scalar::Text(text.clone())
        } else if let Some(value) = value.try_downcast_ref::<bool>() {
            Scalar::Bool(*value)
        } else if let Some(number) = numbers::Number::bind(value) {
            Scalar::Number(number)
        } else if let (ReflectRef::Enum(value), Some(info @ TypeInfo::Enum(schema))) =
            (value.reflect_ref(), value.get_represented_type_info())
        {
            if !schema
                .iter()
                .all(|variant| matches!(variant, VariantInfo::Unit(_)))
            {
                return Err(
                    "Only native enums with unit variants are supported by this field.".into(),
                );
            }
            Scalar::UnitEnum(info, value.variant_name().into())
        } else {
            return Err(
                "This field requires a String, finite number, bool or native unit enum; use default view.".into(),
            );
        };
        let path = ParsedPath::from(
            binding
                .path
                .into_iter()
                .flat_map(|path| &path.0)
                .chain(&relative.0)
                .map(|part| part.access.clone())
                .collect::<Vec<_>>(),
        );
        Ok(Self {
            owner,
            diagnostic,
            token: binding.token,
            path,
            scalar,
        })
    }
    /// Bind one displayed axis to a complete native Vec3 proposal. The owning field
    /// path is preserved, so an edit never depends on opaque vector internals or
    /// overwrites the other axes with rounded display text. Invalid axes/types reject.
    pub fn vector_axis(
        input: Entity,
        diagnostic: Entity,
        binding: ValueBinding<'_>,
        axis: usize,
    ) -> Result<Self, String> {
        let value = *binding
            .value
            .try_downcast_ref::<Vec3>()
            .ok_or("Native Vec3 required")?;
        if axis >= 3 || !value.is_finite() {
            return Err("Vector axis requires a finite native Vec3 and an axis in 0..3".into());
        }
        Ok(Self {
            owner: input,
            diagnostic,
            token: binding.token,
            path: binding
                .path
                .cloned()
                .ok_or("Vector requires an owning field path")?,
            scalar: Scalar::Vector { value, axis },
        })
    }
    /// Bind a native sRGB/linear SDR color at its exact owning field path. Hex input
    /// is explicitly sRGB; the stored color variant is preserved. Unsupported/HDR
    /// values reject rather than silently clamping or converting the source.
    pub fn color(
        input: Entity,
        diagnostic: Entity,
        binding: ValueBinding<'_>,
    ) -> Result<Self, String> {
        let value = *binding
            .value
            .try_downcast_ref::<Color>()
            .ok_or("Native Color required")?;
        Ok(Self {
            owner: input,
            diagnostic,
            token: binding.token,
            path: binding
                .path
                .cloned()
                .ok_or("Color requires an owning field path")?,
            scalar: Scalar::Color(color::ColorValue::bind(value)?),
        })
    }
    /// Native display without source serialization. Numeric text round-trips exactly;
    /// color hex is an SDR preview whose unchanged acceptance preserves exact channels.
    pub fn display(&self) -> String {
        match &self.scalar {
            Scalar::Text(value) | Scalar::UnitEnum(_, value) => value.clone(),
            Scalar::Bool(value) => value.to_string(),
            Scalar::Number(number) => number.display.clone(),
            Scalar::Color(color) => color.display.clone(),
            Scalar::ColorChannel { value, axis } => color_channels(*value)[*axis].to_string(),
            Scalar::Length(value) => match value {
                Val::Auto => "Auto".into(),
                Val::Px(value)
                | Val::Percent(value)
                | Val::Vw(value)
                | Val::Vh(value)
                | Val::VMin(value)
                | Val::VMax(value)
                | Val::Em(value)
                | Val::Rem(value) => value.to_string(),
            },
            Scalar::Vector { value, axis } => value[*axis].to_string(),
        }
    }
    /// Validate native text into an exact-type proposal. No source/UI mutation occurs.
    pub fn propose(&self, text: &str) -> Result<Box<dyn PartialReflect>, String> {
        match &self.scalar {
            Scalar::Number(number) => number.propose(text),
            Scalar::Color(color) => color.propose(text),
            Scalar::Length(value) => {
                let amount = text.parse::<f32>().map_err(|_| "Enter a finite length.")?;
                if !amount.is_finite() {
                    return Err("Enter a finite length.".into());
                }
                Ok(Box::new(match value {
                    Val::Auto => {
                        return Err("Choose a length unit before editing its amount.".into());
                    }
                    Val::Px(_) => Val::Px(amount),
                    Val::Percent(_) => Val::Percent(amount),
                    Val::Vw(_) => Val::Vw(amount),
                    Val::Vh(_) => Val::Vh(amount),
                    Val::VMin(_) => Val::VMin(amount),
                    Val::VMax(_) => Val::VMax(amount),
                    Val::Em(_) => Val::Em(amount),
                    Val::Rem(_) => Val::Rem(amount),
                }))
            }
            Scalar::ColorChannel { value, axis } => {
                let channel = text
                    .parse::<f32>()
                    .map_err(|_| "Enter a channel in 0..=1.")?;
                if !channel.is_finite() || !(0. ..=1.).contains(&channel) {
                    return Err("Enter a finite channel in 0..=1.".into());
                }
                let mut channels = color_channels(*value);
                if channels[*axis] == channel {
                    return Ok(Box::new(*value));
                }
                channels[*axis] = channel;
                Ok(Box::new(if matches!(value, Color::LinearRgba(_)) {
                    Color::linear_rgba(channels[0], channels[1], channels[2], channels[3])
                } else {
                    Color::srgba(channels[0], channels[1], channels[2], channels[3])
                }))
            }
            Scalar::Vector { value, axis } => {
                let proposed = text.parse::<f32>().map_err(|_| "Enter a finite number.")?;
                if !proposed.is_finite() {
                    return Err("Enter a finite number.".into());
                }
                let mut value = *value;
                value[*axis] = proposed;
                Ok(Box::new(value))
            }
            Scalar::Text(_) => {
                if text.chars().count() > 4096 || text.chars().any(|ch| matches!(ch, '\n' | '\r')) {
                    return Err("Enter at most 4096 single-line characters.".into());
                }
                Ok(Box::new(text.to_owned()))
            }
            Scalar::Bool(_) => text
                .parse::<bool>()
                .map(|value| Box::new(value) as Box<dyn PartialReflect>)
                .map_err(|_| "Enter true or false.".into()),
            Scalar::UnitEnum(info @ TypeInfo::Enum(schema), _) => {
                let index = schema
                    .iter()
                    .position(|variant| variant.name() == text)
                    .ok_or_else(|| {
                        format!(
                            "Choose a native variant: {}.",
                            schema
                                .iter()
                                .map(VariantInfo::name)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?;
                let mut value = DynamicEnum::new_with_index(index, text, DynamicVariant::Unit);
                value.set_represented_type(Some(info));
                Ok(Box::new(value))
            }
            Scalar::UnitEnum(_, _) => Err("Native enum schema unavailable.".into()),
        }
    }
}

fn color_channels(value: Color) -> [f32; 4] {
    match value {
        Color::LinearRgba(value) => [value.red, value.green, value.blue, value.alpha],
        _ => {
            let value = value.to_srgba();
            [value.red, value.green, value.blue, value.alpha]
        }
    }
}
