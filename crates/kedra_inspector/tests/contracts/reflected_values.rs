//! A plain application type needs no component-specific inspector implementation.
use bevy::reflect::{Reflect, TypeRegistry, structs::DynamicStruct};
use kedra_inspector::discovery::{Limits, inspect};
use kedra_inspector::value::display;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Reflect)]
struct ApplicationSettings {
    name: String,
    exact: u64,
    signed: i128,
    tuple: (bool, f64),
    choice: Choice,
    samples: Vec<u16>,
    map: BTreeMap<String, u32>,
    set: HashSet<u32>,
    opaque_set: BTreeSet<u32>,
}
#[derive(Reflect)]
enum Choice {
    Active { strength: f32 },
    Empty,
}

#[test]
fn native_structure_and_exact_primitive_values_need_no_bespoke_drawer() {
    let mut registry = TypeRegistry::default();
    registry.register::<ApplicationSettings>();
    let source = ApplicationSettings {
        name: " 日本語 e\u{301} 👩🏽‍🚀 ".into(),
        exact: u64::MAX,
        signed: i128::MIN,
        tuple: (true, 1.2345678901234567),
        choice: Choice::Active { strength: 0.25 },
        samples: vec![2, 7],
        map: BTreeMap::from([("key".into(), 42)]),
        set: HashSet::from([9]),
        opaque_set: BTreeSet::from([10]),
    };
    let rows = inspect(&source, &registry, Limits::default());
    let texts: Vec<_> = rows.iter().map(|r| display(r.value, 4096).text).collect();
    for expected in [
        " 日本語 e\u{301} 👩🏽‍🚀 ",
        "18446744073709551615",
        "-170141183460469231731687303715884105728",
        "1.2345678901234567",
        "Active",
        "42",
        "9",
    ] {
        assert!(
            texts.iter().any(|s| s == expected),
            "missing {expected}: {texts:?}"
        );
    }
    let strength = rows
        .iter()
        .find(|r| r.label == "strength")
        .unwrap_or_else(|| panic!("missing enum payload"));
    assert_eq!(
        strength.path.as_ref().map(ToString::to_string).as_deref(),
        Some(".choice.strength")
    );
    assert_eq!(source.exact, u64::MAX);
    assert!(
        rows.iter()
            .find(|r| r.label == "opaque_set")
            .is_some_and(|r| r.diagnostic.is_some())
    );
}

#[test]
fn literal_unsupported_text_is_not_a_capability_failure_and_escaped_previews_mark_limits() {
    let mut registry = TypeRegistry::default();
    registry.register::<String>();
    let source = "Unsupported value".to_string();
    assert!(
        inspect(&source, &registry, Limits::default())[0]
            .diagnostic
            .is_none()
    );
    let preview = display(&"\n\n".to_string(), 3);
    assert!(preview.truncated);
    assert!(preview.escaped);
}

#[test]
fn depth_rows_collections_and_unicode_previews_are_visibly_bounded() {
    let mut registry = TypeRegistry::default();
    registry.register::<Vec<Vec<u32>>>();
    let value = vec![vec![1u32; 200]; 200];
    let rows = inspect(
        &value,
        &registry,
        Limits {
            depth: 1,
            collection: 3,
            rows: 5,
        },
    );
    assert!(rows.len() <= 5);
    assert!(rows.iter().any(|r| r.diagnostic.is_some()));
    let preview = display(&"日本語👩🏽‍🚀".to_string(), 3);
    assert_eq!(preview.text, "日本語…");
    assert!(preview.truncated);
    let exact = display(&"日本語".to_string(), 3);
    assert_eq!(exact.text, "日本語");
    assert!(!exact.truncated);
}

#[test]
fn missing_represented_type_is_a_diagnostic_not_an_invented_schema() {
    let mut value = DynamicStruct::default();
    value.insert("field", true);
    let rows = inspect(&value, &TypeRegistry::default(), Limits::default());
    assert!(
        rows[0]
            .diagnostic
            .as_deref()
            .is_some_and(|s| s.contains("represented type"))
    );
}
