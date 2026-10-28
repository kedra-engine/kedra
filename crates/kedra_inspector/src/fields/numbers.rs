//! Exact native numeric adapters; no intermediate floating-point value or source parser.
use bevy::prelude::*;

#[derive(Clone)]
pub(super) struct Number {
    pub display: String,
    parse: fn(&str) -> Result<Box<dyn PartialReflect>, String>,
}
impl Number {
    pub fn bind(value: &dyn PartialReflect) -> Option<Self> {
        macro_rules! integer {
            ($($ty:ty),* $(,)?) => { $(
                if let Some(value) = value.try_downcast_ref::<$ty>() {
                    return Some(Self {
                        display: value.to_string(),
                        parse: |text| text.parse::<$ty>()
                            .map(|value| Box::new(value) as Box<dyn PartialReflect>)
                            .map_err(|_| format!("Enter an integer in {}..={}.", <$ty>::MIN, <$ty>::MAX)),
                    });
                }
            )* };
        }
        integer!(
            u8,
            u16,
            u32,
            u64,
            u128,
            usize,
            i8,
            i16,
            i32,
            i64,
            i128,
            isize,
            std::num::NonZeroU16,
            std::num::NonZeroI16
        );
        macro_rules! float {
            ($($ty:ty),* $(,)?) => { $(
                if let Some(value) = value.try_downcast_ref::<$ty>() {
                    if !value.is_finite() { return None; }
                    return Some(Self {
                        display: value.to_string(),
                        parse: |text| {
                            let value = text.parse::<$ty>()
                                .map_err(|_| "Enter a finite number.".to_owned())?;
                            if !value.is_finite() { return Err("Enter a finite number.".into()); }
                            Ok(Box::new(value))
                        },
                    });
                }
            )* };
        }
        float!(f32, f64);
        None
    }
    pub fn propose(&self, text: &str) -> Result<Box<dyn PartialReflect>, String> {
        (self.parse)(text)
    }
}
