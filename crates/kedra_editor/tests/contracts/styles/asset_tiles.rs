//! Asset tile geometry and name alignment belong to the editor's authored styles.
use super::*;

#[test]
fn compact_tiles_use_intrinsic_names_and_list_allocates_remaining_width() -> Result<()> {
    // This CPU style fixture has no font loader. Delivered fonts and rendered
    // Ink centering and clipping are checked by the native asset-folders scenario.
    let tokens = include_str!("../../../../../assets/editor/styles/tokens.css")
        .lines()
        .filter(|line| !line.starts_with("@font-face"))
        .collect::<Vec<_>>()
        .join("\n");
    let (mut app, root) = stylesheet(format!(
        "{}\n{}",
        tokens,
        include_str!("../../../../../assets/editor/styles/asset-browser.css")
    ))?;
    let canvas = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("browser-tile-canvas"),
            ChildOf(root),
        ))
        .id();
    let tile = app
        .world_mut()
        .spawn((
            Node::default(),
            ClassList::new("browser-entry"),
            ChildOf(canvas),
        ))
        .id();
    let name = app
        .world_mut()
        .spawn((
            Node::default(),
            Text::new("models"),
            TextLayout::no_wrap(),
            ClassList::new("browser-name"),
            ChildOf(tile),
        ))
        .id();
    frames(&mut app);
    let node = app.world().get::<Node>(tile).ok_or("Missing tile")?;
    assert_eq!((node.width, node.height), (Val::Px(80.), Val::Px(96.)));
    assert_eq!(node.align_items, AlignItems::Center);
    assert_eq!(
        app.world().get::<Node>(name).ok_or("Missing name")?.width,
        Val::Auto
    );
    kedra_ui::docking::class(app.world_mut(), canvas, "list", true);
    frames(&mut app);
    let list = app.world().get::<Node>(name).ok_or("Missing list name")?;
    assert_eq!(list.flex_grow, 1.);
    assert_eq!(list.flex_basis, Val::Px(0.));
    kedra_ui::docking::class(app.world_mut(), canvas, "list", false);
    frames(&mut app);
    let grid = app.world().get::<Node>(name).ok_or("Missing grid name")?;
    assert_eq!(grid.width, Val::Auto);
    assert_eq!(grid.max_width, Val::Percent(100.));
    assert_eq!(grid.flex_grow, 0.);
    assert_eq!(grid.flex_shrink, 0.);
    assert_eq!(grid.flex_basis, Val::Auto);
    Ok(())
}
