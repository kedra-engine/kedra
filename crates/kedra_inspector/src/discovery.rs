//! Budgeted presentation traversal over Bevy reflection, not a second type schema.
use bevy::reflect::{Access, ParsedPath, PartialReflect, ReflectRef, TypeRegistry};

/// Tightenable presentation limits. Larger values are clamped to the profile ceiling.
#[derive(Clone, Copy)]
pub struct Limits {
    /// Maximum nested reflected depth; ceiling 32.
    pub depth: usize,
    /// Maximum entries shown per collection page; ceiling 128.
    pub collection: usize,
    /// Maximum presentation rows per rebuild; ceiling 2048.
    pub rows: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            depth: 32,
            collection: 128,
            rows: 2048,
        }
    }
}

/// One transient presentation row borrowing its owning immutable source snapshot.
/// Native paths are absent for map keys/membership: ordinal display is not identity.
pub struct Row<'a> {
    /// Structural UI key, valid only within this exact snapshot/binding.
    pub key: Vec<usize>,
    /// Native reflected access path, where Bevy supports one.
    pub path: Option<ParsedPath>,
    /// Original native value; never a normalized copy or writable buffer.
    pub value: &'a dyn PartialReflect,
    /// Presentation label, not field identity.
    pub label: String,
    /// Parent row index, preserving context during filtering.
    pub parent: Option<usize>,
    /// Native nesting depth, not authoring instance depth.
    pub depth: usize,
    /// Capability or visible traversal-limit explanation.
    pub diagnostic: Option<String>,
    /// A traversal ceiling omitted source content; filtering must not hide this fact.
    pub incomplete: bool,
}

/// Discover immutable reflected structure with explicit safety limits.
/// No type construction, hooks, source decoding or unbounded debug formatting occurs.
pub fn inspect<'a>(
    value: &'a dyn PartialReflect,
    registry: &TypeRegistry,
    limits: Limits,
) -> Vec<Row<'a>> {
    let limits = Limits {
        depth: limits.depth.min(32),
        collection: limits.collection.clamp(1, 128),
        rows: limits.rows.clamp(1, 2048),
    };
    let mut rows = Vec::new();
    visit(
        value,
        "Value".into(),
        Some(Vec::new()),
        Vec::new(),
        None,
        registry,
        limits,
        &mut rows,
    );
    rows
}

#[allow(
    clippy::too_many_arguments,
    reason = "bounded traversal carries native paths and presentation ancestry independently"
)]
fn visit<'a>(
    value: &'a dyn PartialReflect,
    label: String,
    accesses: Option<Vec<Access<'static>>>,
    key: Vec<usize>,
    parent: Option<usize>,
    registry: &TypeRegistry,
    limits: Limits,
    rows: &mut Vec<Row<'a>>,
) {
    if rows.len() >= limits.rows {
        if let Some(last) = rows.last_mut() {
            last.diagnostic = Some("Presentation row limit reached".into());
            last.incomplete = true;
        }
        return;
    }
    let index = rows.len();
    let depth = key.len();
    let diagnostic = match value.get_represented_type_info() {
        None => Some("Missing represented type information".into()),
        Some(info) if registry.get(info.type_id()).is_none() => {
            Some("Type is not registered in the host registry".into())
        }
        _ => None,
    };
    rows.push(Row {
        key: key.clone(),
        path: accesses.clone().map(ParsedPath::from),
        value,
        label,
        parent,
        depth,
        diagnostic,
        incomplete: false,
    });
    let children = matches!(
        value.reflect_ref(),
        ReflectRef::Struct(_)
            | ReflectRef::Tuple(_)
            | ReflectRef::TupleStruct(_)
            | ReflectRef::Enum(_)
            | ReflectRef::List(_)
            | ReflectRef::Array(_)
            | ReflectRef::Map(_)
            | ReflectRef::Set(_)
    );
    if children && depth >= limits.depth {
        rows[index].diagnostic = Some("Reflected depth limit reached".into());
        rows[index].incomplete = true;
        return;
    }
    let mut position = 0;
    let mut child = |value, label, access: Option<Access<'static>>| {
        let mut child_key = key.clone();
        child_key.push(position);
        position += 1;
        let path = accesses.clone().and_then(|mut path| {
            path.push(access?);
            Some(path)
        });
        visit(
            value,
            label,
            path,
            child_key,
            Some(index),
            registry,
            limits,
            rows,
        );
    };
    match value.reflect_ref() {
        ReflectRef::Struct(value) => {
            for i in 0..value.field_len().min(limits.rows) {
                if let (Some(name), Some(value)) = (value.name_at(i), value.field_at(i)) {
                    child(
                        value,
                        name.into(),
                        Some(Access::Field(name.to_owned().into())),
                    );
                }
            }
        }
        ReflectRef::Tuple(value) => {
            for i in 0..value.field_len().min(limits.rows) {
                if let Some(value) = value.field(i) {
                    child(value, i.to_string(), Some(Access::TupleIndex(i)));
                }
            }
        }
        ReflectRef::TupleStruct(value) => {
            for i in 0..value.field_len().min(limits.rows) {
                if let Some(value) = value.field(i) {
                    child(value, i.to_string(), Some(Access::TupleIndex(i)));
                }
            }
        }
        ReflectRef::Enum(value) => {
            for i in 0..value.field_len().min(limits.rows) {
                if let Some(field) = value.field_at(i) {
                    let (label, access) = value.name_at(i).map_or_else(
                        || (i.to_string(), Access::TupleIndex(i)),
                        |name| (name.into(), Access::Field(name.to_owned().into())),
                    );
                    child(field, label, Some(access));
                }
            }
        }
        ReflectRef::List(value) => {
            for (i, value) in value.iter().take(limits.collection).enumerate() {
                child(value, format!("[{i}]"), Some(Access::ListIndex(i)));
            }
            if value.len() > limits.collection {
                rows[index].diagnostic = Some("Collection page limit reached".into());
                rows[index].incomplete = true;
            }
        }
        ReflectRef::Array(value) => {
            for (i, value) in value.iter().take(limits.collection).enumerate() {
                child(value, format!("[{i}]"), Some(Access::ListIndex(i)));
            }
            if value.len() > limits.collection {
                rows[index].diagnostic = Some("Collection page limit reached".into());
                rows[index].incomplete = true;
            }
        }
        ReflectRef::Map(value) => {
            for (i, (key, value)) in value.iter().take(limits.collection).enumerate() {
                child(key, format!("Entry {i} key"), None);
                child(value, format!("Entry {i} value"), None);
            }
            rows[index].incomplete = value.len() > limits.collection;
            rows[index].diagnostic = Some(
                if value.len() > limits.collection {
                    "Collection page limit reached; map entries have no positional field path"
                } else {
                    "Map entries have no positional field path"
                }
                .into(),
            );
        }
        ReflectRef::Set(value) => {
            for (i, value) in value.iter().take(limits.collection).enumerate() {
                child(value, format!("Member {i}"), None);
            }
            rows[index].incomplete = value.len() > limits.collection;
            rows[index].diagnostic = Some(
                if value.len() > limits.collection {
                    "Collection page limit reached; set membership has no positional field path"
                } else {
                    "Set membership has no positional field path"
                }
                .into(),
            );
        }
        _ => {
            if !crate::value::primitive(value) {
                rows[index].diagnostic =
                    Some("Opaque value needs a type-specific presentation".into());
            }
        }
    }
}
