//! Browser parents may select one native scene, never guess among multiple labels.
use super::drop_choice;
#[test]
fn bare_model_drop_requires_one_advertised_native_scene_of_that_exact_parent() {
    let choices = vec!["game://ship.glb#Scene7".into()];
    assert_eq!(
        drop_choice(&choices, "game://ship.glb").as_deref(),
        Some("game://ship.glb#Scene7")
    );
    assert_eq!(drop_choice(&choices, "game://ship.glb#Scene0"), None);
    assert_eq!(drop_choice(&choices, "game://foreign.glb"), None);
    assert_eq!(drop_choice(&choices, "other://ship.glb"), None);
    let multiple = vec![
        "game://ship.glb#Scene7".into(),
        "game://ship.glb#Scene2".into(),
    ];
    assert_eq!(drop_choice(&multiple, "game://ship.glb"), None);
    assert_eq!(
        drop_choice(&multiple, "game://ship.glb#Scene2").as_deref(),
        Some("game://ship.glb#Scene2")
    );
}
