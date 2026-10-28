use bevy::{
    asset::{AssetPath, LoadFromPath, UntypedHandle},
    prelude::*,
    reflect::TypeRegistry,
};
struct NoLoad;
impl LoadFromPath for NoLoad {
    fn load_from_path_erased(
        &mut self,
        _: std::any::TypeId,
        _: AssetPath<'static>,
    ) -> UntypedHandle {
        panic!("Clear must not request asset loading")
    }
}
#[test]
fn required_reference_cannot_clear_and_optional_clear_preserves_the_exact_native_type()
-> Result<(), Box<dyn std::error::Error>> {
    let mut types = TypeRegistry::default();
    types.register::<Handle<Image>>();
    types.register::<Option<Handle<Image>>>();
    let required = Handle::<Image>::default();
    assert!(kedra_assets::references::proposal(&required, None, &types, &mut NoLoad).is_err());
    let optional = Some(required.clone());
    let clear = kedra_assets::references::proposal(&optional, None, &types, &mut NoLoad)?;
    assert_eq!(
        clear.try_downcast_ref::<Option<Handle<Image>>>(),
        Some(&None)
    );
    assert_eq!(optional, Some(required));
    assert!(kedra_assets::references::asset_type(&42_u32, &types).is_err());
    Ok(())
}
