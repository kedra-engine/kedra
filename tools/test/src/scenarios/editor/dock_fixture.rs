//! Production dock behavior through real input and GPU fences, without image goldens.
use super::support::*;
use kedra_test::{Result, process::NativeProcess};
use serde_json::{Value, json};
use std::{cell::RefCell, fs, path::Path, process::Command, time::Duration};

struct Dock<'a> {
    c: Editor<'a>,
    checkpoints: RefCell<serde_json::Map<String, Value>>,
}
impl Dock<'_> {
    fn fixture(&self) -> Result<Value> {
        self.c.rpc("dock.fixture")
    }
    fn rendered(&self, window: u64) -> Result<Value> {
        self.c.until("native glyph readiness", || {
            Ok(self.fixture()?["feedback_ready"] == true)
        })?;
        self.c.frame(window, "dock renderer synchronization")
    }
    fn reset(&self, width: u32, height: u32) -> Result<()> {
        let generation = self.fixture()?["generation"]
            .as_u64()
            .ok_or("Missing fixture generation")?;
        self.c
            .client
            .scenario("dock.reset", json!({"width":width,"height":height}))?;
        self.c.until("fixture source reset", || {
            Ok(self.fixture()?["generation"]
                .as_u64()
                .is_some_and(|n| n > generation))
        })?;
        self.c.until("retired fixture floats", || {
            Ok(array(&self.c.client.observe()?["windows"])?.len() == 1)
        })?;
        self.c.activate(self.c.window()?)?;
        self.c.move_to([1., 1.])?;
        self.c.until("baseline splitters", || {
            Ok(self
                .c
                .nodes()?
                .iter()
                .filter(|n| {
                    n.id.ends_with("/divider") && n.rect[2] > n.rect[0] && n.rect[3] > n.rect[1]
                })
                .count()
                == 2)
        })?;
        self.rendered(self.c.window()?)?;
        Ok(())
    }
    fn preview(&self) -> Result<Node> {
        self.c.until("delivered preview glyphs", || {
            Ok(self.fixture()?["feedback_ready"] == true)
        })?;
        self.feedback_node("dock.preview")?;
        self.c.preview()
    }
    fn feedback_node(&self, id: &str) -> Result<Node> {
        self.c.client.wait(id, Duration::from_secs(40), || {
            check(self.fixture()?["dragging"] == true,
                format!("Dock gesture ended before {id}; inspect retained dock.trace for focus loss or cancellation"))?;
            self.c.optional(id)
        })
    }
    fn point(&self, view: &str) -> Result<[f64; 2]> {
        Ok(self
            .c
            .node(&Editor::group_id(&self.c.group(view)?))?
            .center())
    }
    fn checkpoint(&self, name: &str, window: Option<u64>) -> Result<()> {
        let fence = self.rendered(window.unwrap_or(self.c.window()?))?;
        self.checkpoints.borrow_mut().insert(
            name.into(),
            json!({"status":"passed","renderer_frame":fence}),
        );
        Ok(())
    }
    fn cancel(&self, window: u64) -> Result<()> {
        self.c.input(window, json!({"kind":"key","key":"escape"}))?;
        self.c.release(window)?;
        for id in ["dock.drag/ghost", "dock.preview", "dock.insertion"] {
            check(
                !self.c.present(id)?,
                "Cancelled dock fixture retained feedback",
            )?;
        }
        Ok(())
    }
    fn split(&self, divider: &Node, target: [f64; 2]) -> Result<()> {
        let before = self.c.state()?["layout"].clone();
        self.c.move_to(divider.center())?;
        self.c.held(true)?;
        self.c.move_to(target)?;
        self.c.held(false)?;
        self.c.until("committed native divider resize", || {
            Ok(self.c.state()?["layout"] != before)
        })
    }
    fn compact(&self) -> Result<()> {
        self.reset(960, 640)?;
        let vertical = self
            .c
            .nodes()?
            .into_iter()
            .find(|n| n.id.ends_with("/divider") && n.rect[2] - n.rect[0] < 8.)
            .ok_or("Missing vertical divider")?;
        self.split(&vertical, [303., 400.])?;
        let horizontal = self
            .c
            .nodes()?
            .into_iter()
            .find(|n| n.id.ends_with("/divider") && n.rect[3] - n.rect[1] < 8.)
            .ok_or("Missing horizontal divider")?;
        self.split(&horizontal, [600., 341.])
    }
}

pub(super) fn run(root: &Path, output: &Path, delivery: &Path) -> Result<Value> {
    let binary = kedra_test::cargo::build(&root.join("Cargo.toml"), "kedra", "kedra", false, &[])?;
    let public = output.join("public");
    let mut runs = Vec::new();
    let mut report = json!({"schema":1,"fixture":"docking.fixture.v1","verification":"behavior_only","status":"incomplete","runs":[]});
    let result = (|| -> Result<()> {
        for repeat in 1..=2 {
            let private = output
                .join("private/dock-fixture")
                .join(format!("run-{repeat}"));
            let artifacts = private.join("frames");
            let mut command = Command::new(&binary);
            command
                .current_dir(root)
                .arg("--assets")
                .arg(delivery)
                .args(["--scene", "editor://fixtures/docking.scene.ron"])
                .args(["--remote-port", "0", "--remote-credentials"])
                .arg(private.join("session.json"))
                .env("KEDRA_DOCK_TRACE", "1")
                .arg("--remote-artifacts")
                .arg(&artifacts);
            let mut native = NativeProcess::spawn(&mut command, &private)?;
            let client = native.connect()?;
            let c = Editor::new(&client, root, &artifacts, &private, None, "dock.state");
            let dock = Dock {
                c,
                checkpoints: RefCell::new(serde_json::Map::new()),
            };
            dock.c.until(
                "dock fixture ready",
                || Ok(dock.fixture()?["ready"] == true),
            )?;
            check(
                dock.fixture()?["project_tools"] == false
                    && dock.fixture()?["preferences"] == false,
                "Isolated dock host installed editor tools/preferences",
            )?;
            let execution = scenarios(&dock);
            if let Err(error) = &execution {
                fs::write(
                    private.join("failure.json"),
                    serde_json::to_vec_pretty(&json!({
                        "error":error.to_string(),"fixture":dock.fixture().ok(),"workspace":dock.c.state().ok(),
                        "observation":client.observe().ok(),"trace":client.rpc("dock.trace",Value::Null).ok()
                    }))?,
                )?;
            }
            runs.push(json!({"repeat":repeat,"checkpoints":dock.checkpoints.borrow().clone(),"renderer_frames":dock.c.captures.borrow().clone()}));
            report["runs"] = json!(runs);
            execution?;
            native.finish(|| {
                dock.c.held(false)?;
                let _ = client.input(client.window()?, json!({"kind":"key","key":"f12"}));
                Ok(())
            })?;
        }
        check(
            runs.len() == 2,
            "Both independent native dock sessions are required",
        )?;
        check(
            runs[0]["checkpoints"]
                .as_object()
                .map(|v| v.keys().collect::<Vec<_>>())
                == runs[1]["checkpoints"]
                    .as_object()
                    .map(|v| v.keys().collect::<Vec<_>>()),
            "Native dock sessions did not complete the same behavior sequence",
        )?;
        report["status"] = json!("passed");
        Ok(())
    })();
    if let Err(error) = &result {
        report["status"] = json!("failed");
        report["error"] = json!(error.to_string());
    }
    fs::create_dir_all(public.join("dock-fixture"))?;
    fs::write(
        public.join("dock-fixture/report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    result?;
    Ok(report)
}

fn scenarios(dock: &Dock<'_>) -> Result<()> {
    let c = &dock.c;
    let fixture = dock.fixture()?;
    let views = array(&fixture["views"])?;
    check(views.len() == 4, "Wrong fixture view inventory")?;
    let initial_layout = c.state()?["layout"].clone();
    for arguments in [json!({"width":0,"height":800}), json!({"height":800})] {
        check(
            c.client.scenario("dock.reset", arguments).is_err(),
            "Diagnostic reset accepted invalid dimensions",
        )?;
        check(
            dock.fixture()?["generation"] == fixture["generation"]
                && c.state()?["layout"] == initial_layout,
            "Rejected diagnostic program changed the fixture",
        )?;
    }
    let (a, b, child, d) = (
        text(&views[0])?,
        text(&views[1])?,
        text(&views[2])?,
        text(&views[3])?,
    );
    dock.reset(1200, 800)?;
    dock.checkpoint("baseline", None)?;
    dock.compact()?;
    dock.checkpoint("compact resize", None)?;
    dock.reset(1200, 800)?;
    let before = c.state()?["layout"].clone();
    let window = c.drag(child, [784., 270.])?;
    dock.preview()?;
    check(
        c.node("dock.drag/title")?.caption() == "Panel C",
        "Drag lost its source caption",
    )?;
    check(c.state()?["layout"] == before, "Live drag published layout")?;
    dock.checkpoint("compass preview", None)?;
    c.move_to([1031., 270.])?;
    let preview = dock.preview()?;
    dock.checkpoint("direct edge preview", None)?;
    c.release(window)?;
    c.until("split result", || Ok(c.group(child)? != c.group(b)?))?;
    for (a, b) in preview
        .rect
        .into_iter()
        .zip(c.node(&Editor::group_id(&c.group(child)?))?.rect)
    {
        near(a, b, 1.)?;
    }
    dock.checkpoint("committed split", None)?;
    dock.reset(1200, 800)?;
    let window = c.drag(child, [151., 420.])?;
    dock.preview()?;
    dock.checkpoint("merge preview", None)?;
    c.release(window)?;
    c.until("merge result", || Ok(c.group(child)? == c.group(a)?))?;
    dock.checkpoint("committed merge", None)?;
    dock.reset(1200, 800)?;
    let before = c.state()?["layout"].clone();
    let window = c.drag(child, [519., 525.])?;
    dock.feedback_node("dock.insertion")?;
    check(
        !c.present("dock.preview")? && !c.present("dock.guide/tab/guide")?,
        "Rail insertion painted split/compass feedback",
    )?;
    check(
        c.state()?["layout"] == before,
        "Live insertion published layout",
    )?;
    dock.checkpoint("tab insertion", None)?;
    c.release(window)?;
    check(
        c.state()?["layout"]["nodes"][c.group(child)?]["Group"]["tabs"] == json!([d, child]),
        "Insertion lost tab order",
    )?;
    dock.checkpoint("committed tab order", None)?;
    let tab = c.node(&format!("dock/tab/ViewId({d})"))?;
    let window = c.drag(child, [tab.rect[0] + 1., tab.center()[1]])?;
    dock.feedback_node("dock.insertion")?;
    c.release(window)?;
    c.until("reordered tabs", || {
        Ok(c.state()?["layout"]["nodes"][c.group(child)?]["Group"]["tabs"] == json!([child, d]))
    })?;
    dock.checkpoint("reordered tabs", None)?;
    dock.reset(1200, 800)?;
    let before = c.state()?["layout"].clone();
    let window = c.drag(child, [183., 420.])?;
    check(
        dock.feedback_node("dock.drag/reason")?.caption() == "Not enough room · dock as tab",
        "Blocked drop lost its refusal reason",
    )?;
    check(!c.present("dock.preview")?, "Blocked split painted preview")?;
    c.release(window)?;
    check(c.state()?["layout"] == before, "Blocked split committed")?;
    dock.checkpoint("blocked split", None)?;
    dock.reset(1200, 800)?;
    let divider = c
        .nodes()?
        .into_iter()
        .find(|n| n.id.ends_with("/divider") && n.rect[2] - n.rect[0] < 8.)
        .ok_or("Missing splitter")?;
    dock.split(&divider, [343., 420.])?;
    dock.checkpoint("committed resize", None)?;
    dock.reset(1200, 800)?;
    let baseline = c.state()?["layout"].clone();
    c.menu(b, "Maximize group")?;
    check(
        object(&c.state()?["layout"]["hosts"])?
            .values()
            .any(|h| h["maximized"] == c.group(b).map(Value::String).unwrap_or(Value::Null)),
        "Maximize did not publish target",
    )?;
    c.menu(b, "Restore group")?;
    check(
        c.state()?["layout"]["nodes"] == baseline["nodes"]
            && object(&c.state()?["layout"]["hosts"])?
                .values()
                .all(|h| h["maximized"].is_null()),
        "Restore changed docking topology",
    )?;
    dock.checkpoint("maximize restore", None)?;
    dock.reset(1200, 800)?;
    c.client.scenario("dock.overflow", Value::Null)?;
    let overflow = c.wait_node(&format!("dock/node/NodeId({})/menu", c.group(b)?))?;
    check(
        !c.nodes()?.iter().any(|n| {
            n.id.starts_with("dock/node/")
                && (n.id.ends_with("/maximize") || n.id.ends_with("/overflow"))
        }),
        "Header contains competing actions",
    )?;
    check(
        c.nodes()?
            .iter()
            .any(|n| n.id.ends_with("/title") && n.caption().ends_with('…')),
        "Long native title lacks shaped ellipsis",
    )?;
    c.click(&overflow.id)?;
    c.wait_node("dock-menu")?;
    for caption in [
        "Maximize group",
        "Panel with a very long title that does not fit its tab",
    ] {
        check(
            c.nodes()?.iter().any(|n| n.caption() == caption),
            "Overflow omitted original full title/action",
        )?;
    }
    c.key("escape")?;
    dock.checkpoint("overflow actions", None)?;
    dock.reset(1200, 800)?;
    c.menu(child, "Float active tab")?;
    c.until("native float transfer", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2
            && dock.fixture()?["pending"] == false
            && c.node(&Editor::label(child))?.window()? != c.window()?)
    })?;
    let floating = c.node(&Editor::label(child))?.window()?;
    c.resize(floating, 640, 480, 1)?;
    dock.checkpoint("floating main host", None)?;
    dock.checkpoint("floating child host", Some(floating))?;
    c.drag(child, [100., 100.])?;
    c.input(c.window()?, json!({"kind":"move","position":[415,525]}))?;
    dock.feedback_node("dock.insertion")?;
    c.held(false)?;
    c.until("redock retires float", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    dock.checkpoint("native redock", None)?;
    dock.compact()?;
    let before = c.state()?["layout"].clone();
    let window = c.drag(child, [183., 340.])?;
    check(
        dock.feedback_node("dock.drag/reason")?.caption() == "Not enough room · dock as tab",
        "Compact refusal reason changed",
    )?;
    dock.cancel(window)?;
    check(
        c.state()?["layout"] == before,
        "Compact cancelled drop committed",
    )?;
    dock.checkpoint("compact cancellation", None)?;
    mechanics(dock, a, b, child, d)
}

fn mechanics(dock: &Dock<'_>, a: &str, b: &str, child: &str, d: &str) -> Result<()> {
    let c = &dock.c;
    dock.reset(1440, 1100)?;
    c.menu(child, "Maximize group")?;
    c.click(&Editor::label(d))?;
    let before = c.state()?["layout"].clone();
    let group = c.node(&Editor::group_id(&c.group(d)?))?;
    let window = c.drag(d, [group.rect[0] + 8., group.center()[1]])?;
    dock.rendered(window)?;
    let preview = c.optional("dock.preview")?;
    c.release(window)?;
    if let Some(preview) = preview {
        for (a, b) in preview
            .rect
            .into_iter()
            .zip(c.node(&Editor::group_id(&c.group(d)?))?.rect)
        {
            near(a, b, 1.)?;
        }
    } else {
        check(
            c.state()?["layout"] == before,
            "Unmeasured maximized drop published invisible sibling",
        )?;
    }
    for direction in ["left", "right", "top", "bottom", "tab"] {
        dock.reset(1440, 1100)?;
        let window = c.drag(child, dock.point(b)?)?;
        c.move_to(
            dock.feedback_node(&format!("dock.guide/{direction}/guide"))?
                .center(),
        )?;
        let preview = dock.preview()?;
        let before = c.state()?["layout"].clone();
        check(
            dock.fixture()?["dragging"] == true,
            "Fixture drag disappeared before commit",
        )?;
        c.release(window)?;
        c.until("guide commit", || Ok(c.state()?["layout"] != before))?;
        check(
            (c.group(child)? == c.group(b)?) == (direction == "tab"),
            "Guide committed wrong destination",
        )?;
        for (a, b) in preview
            .rect
            .into_iter()
            .zip(c.node(&Editor::group_id(&c.group(child)?))?.rect)
        {
            near(a, b, 1.)?;
        }
    }
    dock.reset(1200, 800)?;
    c.client.scenario("dock.drafts", Value::Null)?;
    c.click("fixture.C/draft")?;
    c.type_text("Retained native draft")?;
    let field = c.node("fixture.C/draft")?;
    check(
        field.value() == "Retained native draft",
        "Native draft input not delivered",
    )?;
    let window = c.drag(child, dock.point(a)?)?;
    dock.preview()?;
    c.release(window)?;
    let current = c.node(&field.id)?;
    check(
        current.entity == field.entity && current.value() == "Retained native draft",
        "Docking lost native draft identity/value",
    )?;
    dock.reset(1200, 800)?;
    let before = c.state()?["layout"]["nodes"][c.group(child)?]["Group"].clone();
    c.modifier("shift", true)?;
    let window = c.drag(child, dock.point(a)?)?;
    dock.preview()?;
    c.release(window)?;
    c.modifier("shift", false)?;
    check(
        c.group(child)? == c.group(d)? && c.group(d)? == c.group(a)?,
        "Whole-group drop lost source views",
    )?;
    let state = c.state()?;
    let order = array(&state["layout"]["nodes"][c.group(a)?]["Group"]["tabs"])?;
    let tabs = array(&before["tabs"])?;
    check(
        order.len() >= tabs.len() && order[order.len() - tabs.len()..] == tabs[..],
        "Whole-group drag changed tab order",
    )?;
    dock.reset(1200, 800)?;
    for dpr in [1, 2] {
        c.resize(c.window()?, 1200, 800, dpr)?;
        for view in [a, child] {
            let button = c.node(&format!("dock/node/NodeId({})/menu", c.group(view)?))?;
            c.move_to([1., 1.])?;
            c.until("transparent flat idle", || {
                Ok(c.node(&button.id)?.background.is_some_and(|b| b[3] == 0))
            })?;
            let idle = c.node(&button.id)?.background;
            c.move_to(button.center())?;
            c.until("flat hover", || {
                let background = c.node(&button.id)?.background;
                Ok(background != idle && background.is_some_and(|b| b[3] > 0))
            })?;
            let hover = c.node(&button.id)?.background;
            c.held(true)?;
            c.until("flat pressed", || {
                let background = c.node(&button.id)?.background;
                Ok(background != hover && background.is_some_and(|b| b[3] > 0))
            })?;
            check(
                dock.fixture()?["dragging"] == false,
                "Menu press started tab drag",
            )?;
            c.held(false)?;
            c.anchored_popup(&button)?;
            c.key("escape")?;
        }
    }
    dock.reset(1200, 800)?;
    c.menu(child, "Float active tab")?;
    c.until("floating close setup", || {
        Ok(dock.fixture()?["pending"] == false
            && c.node(&Editor::label(child))?.window()? != c.window()?)
    })?;
    c.close_window(c.node(&Editor::label(child))?.window()?)?;
    check(c.hidden(child)?, "Close did not hide floating source view")?;
    dock.reset(1200, 800)?;
    c.client.scenario("dock.drafts", Value::Null)?;
    c.menu(child, "Float active tab")?;
    c.until("floating keyboard field", || {
        Ok(dock.fixture()?["pending"] == false
            && c.optional("fixture.C/draft")?
                .is_some_and(|n| n.window != c.window().ok()))
    })?;
    c.click("fixture.C/draft")?;
    let field = c.node("fixture.C/draft")?;
    let window = field.window()?;
    check(field.focused, "Floating field did not receive focus")?;
    c.input(window, json!({"kind":"key","key":"tab"}))?;
    let focused = c.find("floating Tab traversal", |n| {
        n.focused && n.entity != field.entity
    })?;
    check(
        focused.window()? == window && focused.enabled && focused.visible,
        "Tab escaped floating native window",
    )?;
    c.input(window, json!({"kind":"key","key":"shift_tab"}))?;
    c.until("floating reverse Tab", || Ok(c.node(&field.id)?.focused))?;
    dock.rendered(window)?;
    dock.checkpoint("retained fields and local keyboard", Some(window))?;
    dock.reset(1200, 800)
}
