//! Native Flair visibility must recover after the initial no-selection frame.
use super::*;

#[test]
fn inspector_surfaces_reappear_after_hidden_state_is_removed() -> Result<()> {
    let (mut app, root) = stylesheet(format!(
        "{}\n.hidden {{display:none;}}",
        include_str!("../../../../../assets/editor/styles/inspector.css")
    ))?;
    for class in [
        "inspector-identity",
        "inspector-entries",
        "inspector-toolbar",
        "inspector-message",
    ] {
        let node = app
            .world_mut()
            .spawn((
                Node::default(),
                ClassList::new(&format!("{class} hidden")),
                ChildOf(root),
            ))
            .id();
        frames(&mut app);
        assert_eq!(
            app.world().get::<Node>(node).ok_or("node")?.display,
            Display::None
        );
        kedra_ui::docking::class(app.world_mut(), node, "hidden", false);
        frames(&mut app);
        assert_eq!(
            app.world().get::<Node>(node).ok_or("node")?.display,
            Display::Flex,
            "{class} remained hidden after source selection"
        );
    }
    Ok(())
}
