//! Native color representation and explicit sRGB hex input, without implicit quantization.
use bevy::prelude::*;

#[derive(Clone)]
pub(super) struct ColorValue {
    pub value: Color,
    pub display: String,
}
impl ColorValue {
    pub fn bind(value: Color) -> Result<Self, String> {
        let channels = match value {
            Color::Srgba(value) => [value.red, value.green, value.blue, value.alpha],
            Color::LinearRgba(value) => [value.red, value.green, value.blue, value.alpha],
            _ => {
                return Err(
                    "This color space requires default view; no conversion was applied.".into(),
                );
            }
        };
        if channels
            .iter()
            .any(|value| !value.is_finite() || !(0. ..=1.).contains(value))
        {
            return Err(
                "HDR/out-of-range colors require default view; no clamping was applied.".into(),
            );
        }
        Ok(Self {
            value,
            display: value.to_srgba().to_hex(),
        })
    }
    pub fn propose(&self, text: &str) -> Result<Box<dyn PartialReflect>, String> {
        if text == self.display {
            return Ok(Box::new(self.value));
        }
        let raw = text
            .strip_prefix('#')
            .ok_or("Enter #RRGGBB or #RRGGBBAA in sRGB.")?;
        if !matches!(raw.len(), 6 | 8) || !raw.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Enter #RRGGBB or #RRGGBBAA in sRGB.".into());
        }
        let value = Srgba::hex(raw).map_err(|error| error.to_string())?;
        if value.to_hex() == self.display {
            return Ok(Box::new(self.value));
        }
        Ok(Box::new(match self.value {
            Color::LinearRgba(_) => Color::LinearRgba(value.into()),
            _ => Color::Srgba(value),
        }))
    }
}
