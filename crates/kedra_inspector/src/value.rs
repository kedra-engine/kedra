//! Bounded presentation formatting; this is not a serialization or value model.
use bevy::reflect::{PartialReflect, ReflectRef};

/// Display-only text; truncation must be communicated before offering a copy.
#[derive(Debug, PartialEq, Eq)]
pub struct DisplayValue {
    /// Unicode-preserving preview, never an edit payload.
    pub text: String,
    /// Whether the preview omits source content.
    pub truncated: bool,
    /// Whether controls/line separators were escaped for a single-line preview.
    pub escaped: bool,
}

/// Format exact native primitive values without numeric coercion or debug recursion.
/// Strings containing line/control characters are escaped for a single-line surface.
/// At most 4096 Unicode scalars are exposed even if a caller requests more.
pub fn display(value: &dyn PartialReflect, limit: usize) -> DisplayValue {
    let limit = limit.min(4096);
    if let Some(text) = value.try_downcast_ref::<String>() {
        return preview(text, limit);
    }
    macro_rules! primitive {
        ($($ty:ty),* $(,)?) => { $(
            if let Some(value) = value.try_downcast_ref::<$ty>() {
                return preview(&value.to_string(), limit);
            }
        )* };
    }
    primitive!(
        bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
    );
    let text = match value.reflect_ref() {
        ReflectRef::Struct(_) => "Fields".into(),
        ReflectRef::Tuple(_) | ReflectRef::TupleStruct(_) => "Fields".into(),
        ReflectRef::Enum(value) => value.variant_name().into(),
        ReflectRef::List(value) => format!("{} items", value.len()),
        ReflectRef::Array(value) => format!("{} items", value.len()),
        ReflectRef::Map(value) => format!("{} entries", value.len()),
        ReflectRef::Set(value) => format!("{} members", value.len()),
        _ => "Unsupported value".into(),
    };
    preview(&text, limit)
}

fn preview(source: &str, limit: usize) -> DisplayValue {
    let mut escaped = false;
    let mut chars = source.chars().flat_map(|c| {
        if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
            escaped = true;
            c.escape_default().collect::<Vec<_>>()
        } else {
            vec![c]
        }
    });
    let mut text: String = chars.by_ref().take(limit).collect();
    let truncated = chars.next().is_some();
    if truncated {
        text.push('…');
    }
    DisplayValue {
        text,
        truncated,
        escaped,
    }
}

pub(crate) fn primitive(value: &dyn PartialReflect) -> bool {
    macro_rules! is { ($($ty:ty),*) => { false $(|| value.try_downcast_ref::<$ty>().is_some())* }; }
    is!(
        String, bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32,
        f64
    )
}
