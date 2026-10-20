//! The native loader and authoring share exact catalog-byte admission.
use kedra_scene::assets::CatalogLoader;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn catalog_admission_checks_exact_byte_path_and_inventory_bounds() -> Result<()> {
    let mut exact = "[\"game://actor.prefab.ron\"]".to_owned();
    exact.extend(std::iter::repeat_n(' ', 65_536 - exact.len()));
    assert_eq!(CatalogLoader::parse(&exact)?.len(), 1);
    exact.push(' ');
    assert!(CatalogLoader::parse(&exact).is_err());
    for invalid in [
        "[]",
        "[\"same.prefab.ron\",\"same.prefab.ron\"]",
        "[\"://path\"]",
        "invalid",
    ] {
        assert!(CatalogLoader::parse(invalid).is_err(), "accepted {invalid}");
    }
    let path = format!("game://{}.prefab.ron", "x".repeat(4096));
    assert!(CatalogLoader::parse(&ron::to_string(&vec![path])?).is_err());
    let paths: Vec<_> = (0..129)
        .map(|index| format!("game://{index}.prefab.ron"))
        .collect();
    assert!(CatalogLoader::parse(&ron::to_string(&paths)?).is_err());
    Ok(())
}
