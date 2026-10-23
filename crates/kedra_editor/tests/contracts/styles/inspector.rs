//! Native Flair visibility must recover after the initial no-selection frame.
use super::*;

#[test]
fn inspector_surfaces_reappear_after_hidden_state_is_removed() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(
        source.join("inspector.css"),
        format!(
            "{}\n.hidden {{display:none;}}",
            include_str!("../../../../../assets/editor/styles/inspector.css")
        ),
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    let mut app = consumer(&output, manifest);
    let sheet = load(&mut app, "game://inspector.css")?;
    let root = app
        .world_mut()
        .spawn((Node::default(), Styled::new(sheet)))
        .id();
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
