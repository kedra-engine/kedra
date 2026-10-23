//! Custom editor selection uses native identity, never registration order or labels.
use bevy::{prelude::*, reflect::ParsedPath};
use kedra_inspector::{
    binding::ValueBinding,
    editors::{EditorContext, EditorRegistry, ValueEditor, ValueEditorView},
};
use std::{any::TypeId, sync::Arc};

struct TestEditor;
impl ValueEditor for TestEditor {
    fn mount(
        &self,
        _: &mut World,
        _: Entity,
        _: &mut EditorContext<'_>,
        _: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        Err("Fixture requests explicit default view".into())
    }
}

#[test]
fn field_then_exact_type_then_default_and_duplicate_errors_are_deterministic() -> Result<(), String>
{
    let mut registry = EditorRegistry::default();
    let ty = TypeId::of::<f32>();
    let owner = TypeId::of::<Vec3>();
    let path = ParsedPath::parse("x").map_err(|e| e.to_string())?;
    let type_id = registry.register_type(ty, Arc::new(TestEditor))?;
    let field_id = registry.register_field(owner, path.clone(), Arc::new(TestEditor))?;
    assert_eq!(
        registry
            .resolve(Some(owner), Some(&path), Some(ty), false)
            .map(|s| s.id),
        Some(field_id)
    );
    assert_eq!(
        registry
            .resolve(Some(TypeId::of::<Vec2>()), Some(&path), Some(ty), false)
            .map(|s| s.id),
        Some(type_id)
    );
    assert!(
        registry
            .resolve(Some(owner), Some(&path), Some(ty), true)
            .is_none()
    );
    assert!(
        registry
            .resolve(None, None, Some(TypeId::of::<String>()), false)
            .is_none()
    );
    assert!(registry.register_type(ty, Arc::new(TestEditor)).is_err());
    assert!(
        registry
            .register_field(owner, path.clone(), Arc::new(TestEditor))
            .is_err()
    );
    assert!(
        registry
            .register_field(
                owner,
                ParsedPath::parse(".x").map_err(|e| e.to_string())?,
                Arc::new(TestEditor)
            )
            .is_err()
    );
    let native = ParsedPath::from(vec![bevy::reflect::Access::Field("x".into())]);
    assert_eq!(
        registry
            .resolve(Some(owner), Some(&native), Some(ty), false)
            .map(|s| s.id),
        Some(field_id)
    );
    assert_eq!(
        registry
            .resolve(Some(owner), Some(&path), Some(ty), false)
            .map(|s| s.id),
        Some(field_id)
    );
    let mut reverse = EditorRegistry::default();
    let field = reverse.register_field(owner, path.clone(), Arc::new(TestEditor))?;
    reverse.register_type(ty, Arc::new(TestEditor))?;
    assert_eq!(
        reverse
            .resolve(Some(owner), Some(&path), Some(ty), false)
            .map(|s| s.id),
        Some(field)
    );
    Ok(())
}
