//! Editor-specific input/identity helpers shared by concrete Rust feature scenarios.
use kedra_test::{Client, Result, evidence};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn check(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

impl Editor<'_> {
    pub fn hierarchy_menu(&self, node: Option<&str>, path: &str) -> Result<()> {
        let anchor = self.find("Hierarchy context target", |n| {
            n.id.starts_with("hierarchy/")
                && match node {
                    Some(name) => n.id.ends_with("/name") && n.caption() == name,
                    None => n.id.ends_with("/entries"),
                }
        })?;
        let position = if node.is_some() {
            anchor.center()
        } else {
            [anchor.rect[2] - 8., anchor.rect[3] - 8.]
        };
        self.input(
            anchor.window()?,
            json!({"kind":"click", "position":position, "button":"right"}),
        )?;
        if node.is_some() {
            // Let release-triggered row activation and focus updates settle before
            // choosing a command: the context menu must survive that boundary.
            self.frame(
                anchor.window()?,
                "Hierarchy context menu after secondary release",
            )?;
            check(
                self.nodes()?
                    .iter()
                    .any(|n| n.visible && n.id.starts_with("hierarchy-menu/")),
                "Hierarchy context menu disappeared after secondary release",
            )?;
        }
        for part in path.split('/') {
            let item = self.find("Hierarchy menu action", |n| {
                n.id.starts_with("hierarchy-menu/")
                    && n.id.ends_with("/caption")
                    && n.caption().trim_end_matches(" ▸") == part
            })?;
            self.click(item.id.strip_suffix("/caption").ok_or("Menu caption")?)?;
        }
        Ok(())
    }
}
pub fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| "Missing native string".into())
}
pub fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(|| "Missing native array".into())
}
pub fn object(v: &Value) -> Result<&serde_json::Map<String, Value>> {
    v.as_object().ok_or_else(|| "Missing native object".into())
}
pub fn number(v: &Value) -> Result<f64> {
    v.as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| "Invalid native coordinate".into())
}
pub fn near(a: f64, b: f64, t: f64) -> Result<()> {
    check(
        (a - b).abs() <= t,
        format!("Geometry differs: {a} vs {b}, tolerance {t}"),
    )
}

#[derive(Clone, Deserialize)]
pub struct Node {
    pub id: String,
    pub entity: u64,
    pub parent: Option<u64>,
    pub window: Option<u64>,
    pub rect: [f64; 4],
    pub visible: bool,
    pub enabled: bool,
    pub focused: bool,
    pub text: Option<String>,
    pub value: Option<String>,
    pub settled: Option<bool>,
    pub checked: Option<bool>,
    pub background: Option<[u8; 4]>,
}
impl Node {
    pub fn center(&self) -> [f64; 2] {
        [
            (self.rect[0] + self.rect[2]) / 2.,
            (self.rect[1] + self.rect[3]) / 2.,
        ]
    }
    pub fn window(&self) -> Result<u64> {
        self.window
            .ok_or_else(|| "Node has no native window".into())
    }
    pub fn value(&self) -> &str {
        self.value.as_deref().unwrap_or("")
    }
    pub fn caption(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }
}

pub struct Editor<'a> {
    pub client: &'a Client,
    pub root: &'a Path,
    pub artifacts: &'a Path,
    pub state_dir: &'a Path,
    pub project: Option<&'a Path>,
    workspace_method: &'static str,
    pub captures: RefCell<Vec<Value>>,
}
impl<'a> Editor<'a> {
    pub fn new(
        client: &'a Client,
        root: &'a Path,
        artifacts: &'a Path,
        state_dir: &'a Path,
        project: Option<&'a Path>,
        workspace_method: &'static str,
    ) -> Self {
        Self {
            client,
            root,
            artifacts,
            state_dir,
            project,
            workspace_method,
            captures: RefCell::new(Vec::new()),
        }
    }
    pub fn rpc(&self, name: &str) -> Result<Value> {
        self.client.rpc(name, Value::Null)
    }
    pub fn state(&self) -> Result<Value> {
        self.rpc(self.workspace_method)
    }
    pub fn isolated(&self) -> Result<()> {
        self.until("isolated editor startup", || {
            Ok(self.rpc("editor.state")?["preferences"].is_string())
        })?;
        let state = self.rpc("editor.state")?;
        let preferences = Path::new(text(&state["preferences"])?);
        check(
            preferences == self.state_dir.join("recent-projects.toml"),
            "Refusing personal editor preferences",
        )
    }
    pub fn nodes(&self) -> Result<Vec<Node>> {
        let observation = self.client.observe()?;
        array(&observation["elements"])?
            .iter()
            .filter(|n| n["visible"] == true)
            .map(|n| Ok(serde_json::from_value(n.clone())?))
            .collect()
    }
    pub fn optional(&self, id: &str) -> Result<Option<Node>> {
        Ok(self.nodes()?.into_iter().find(|n| n.id == id))
    }
    pub fn node(&self, id: &str) -> Result<Node> {
        self.optional(id)?
            .ok_or_else(|| format!("Missing visible node: {id}").into())
    }
    pub fn retained(&self, id: &str) -> Result<Node> {
        let o = self.client.observe()?;
        let n = array(&o["elements"])?
            .iter()
            .find(|n| n["id"] == id)
            .ok_or("Retired native node")?;
        Ok(serde_json::from_value(n.clone())?)
    }
    pub fn present(&self, id: &str) -> Result<bool> {
        Ok(self.optional(id)?.is_some())
    }
    pub fn until(&self, label: &str, mut predicate: impl FnMut() -> Result<bool>) -> Result<()> {
        self.client.wait(label, Duration::from_secs(40), || {
            Ok(predicate()?.then_some(()))
        })
    }
    pub fn wait_node(&self, id: &str) -> Result<Node> {
        self.client
            .wait(id, Duration::from_secs(40), || self.optional(id))
    }
    pub fn find(&self, label: &str, mut predicate: impl FnMut(&Node) -> bool) -> Result<Node> {
        self.client.wait(label, Duration::from_secs(40), || {
            Ok(self.nodes()?.into_iter().find(&mut predicate))
        })
    }
    pub fn window(&self) -> Result<u64> {
        self.client.window()
    }
    pub fn event(&self, event: Value) -> Result<Value> {
        self.client.input(self.window()?, event)
    }
    pub fn input(&self, window: u64, event: Value) -> Result<Value> {
        self.client.input(window, event)
    }
    pub fn click(&self, id: &str) -> Result<()> {
        self.client.scenario("app.click", json!({"id":id}))?;
        Ok(())
    }
    pub fn activate(&self, window: u64) -> Result<()> {
        let observation = self.client.observe()?;
        let target = array(&observation["windows"])?
            .iter()
            .find(|target| target["id"] == window)
            .ok_or("Activation target window was retired")?;
        if target["focused"] == true {
            return Ok(());
        }
        check(
            self.state()?["navigation_input"]["gesture"] != true,
            "Cannot activate another window during a live dock gesture",
        )?;
        self.client.input(window, json!({"kind":"focus_window"}))?;
        Ok(())
    }
    pub fn click_retired(&self, id: &str) -> Result<()> {
        let n = self.wait_node(id)?;
        let result = self.input(n.window()?, json!({"kind":"click","element":id}));
        if result.is_err() {
            let o = self.client.observe()?;
            check(
                !array(&o["windows"])?
                    .iter()
                    .any(|w| w["id"] == n.window.unwrap_or(0)),
                "Click failed without retiring native target",
            )?;
        }
        Ok(())
    }
    pub fn close_window(&self, window: u64) -> Result<()> {
        // OS-owned caption buttons are not Bevy UI. Exercise the host's real
        // close-request path without pretending to press a physical OS button.
        let result = self.input(window, json!({"kind":"close_window"}));
        if result.is_err() {
            let observation = self.client.observe()?;
            check(
                !array(&observation["windows"])?
                    .iter()
                    .any(|w| w["id"] == window),
                "Close request failed without retiring its native window",
            )?;
        }
        self.until("native close-request retirement", || {
            Ok(!array(&self.client.observe()?["windows"])?
                .iter()
                .any(|w| w["id"] == window))
        })
    }
    pub fn key(&self, key: &str) -> Result<()> {
        self.event(json!({"kind":"key","key":key}))?;
        Ok(())
    }
    pub fn move_to(&self, p: [f64; 2]) -> Result<()> {
        self.event(json!({"kind":"move","position":p}))?;
        Ok(())
    }
    pub fn held(&self, pressed: bool) -> Result<()> {
        self.event(json!({"kind":"button","pressed":pressed}))?;
        Ok(())
    }
    pub fn modifier(&self, key: &str, pressed: bool) -> Result<()> {
        self.event(json!({"kind":"modifier","modifier":key,"pressed":pressed}))?;
        Ok(())
    }
    pub fn type_text(&self, value: &str) -> Result<()> {
        self.event(json!({"kind":"text","text":value}))?;
        Ok(())
    }
    pub fn field(&self, id: &str, value: &str) -> Result<()> {
        self.client
            .scenario("app.field", json!({"id":id,"value":value}))?;
        Ok(())
    }
    pub fn hidden(&self, view: &str) -> Result<bool> {
        Ok(object(&self.state()?["layout"]["hidden"])?.contains_key(view))
    }
    pub fn anchored_popup(&self, anchor: &Node) -> Result<Node> {
        let popup = self.wait_node("dock-menu")?;
        check(
            popup.parent == Some(anchor.entity) && popup.window == anchor.window,
            "Popup lost its invoking anchor/window",
        )?;
        let observation = self.client.observe()?;
        let window = array(&observation["windows"])?
            .iter()
            .find(|w| w["id"] == anchor.window.unwrap_or(0))
            .ok_or("Popup host is absent")?;
        let (width, height) = (number(&window["width"])?, number(&window["height"])?);
        let (w, h) = (popup.rect[2] - popup.rect[0], popup.rect[3] - popup.rect[1]);
        check(
            w > 0.
                && h > 0.
                && popup.rect[0] >= 0.
                && popup.rect[1] >= 0.
                && popup.rect[2] <= width
                && popup.rect[3] <= height,
            "Popup escaped its native host",
        )?;
        Ok(popup)
    }
    pub fn preview(&self) -> Result<Node> {
        self.client.wait(
            "drop-ready live dock preview",
            Duration::from_secs(40),
            || {
                let state = self.state()?;
                check(
                    state["navigation_input"]["gesture"] == true,
                    "Dock gesture ended while waiting for its preview; no input was replayed",
                )?;
                if state["navigation_input"]["preview_ready"] != true {
                    return Ok(None);
                }
                let Some(before) = self.optional("dock.preview")? else {
                    return Ok(None);
                };
                self.frame(before.window()?, "dock preview synchronization")?;
                let state = self.state()?;
                check(
                    state["navigation_input"]["gesture"] == true,
                    "Dock gesture ended during preview readback; no input was replayed",
                )?;
                if state["navigation_input"]["preview_ready"] != true {
                    return Ok(None);
                }
                if let Some(after) = self.optional("dock.preview")? {
                    let observation = self.client.observe()?;
                    let window = array(&observation["windows"])?
                        .iter()
                        .find(|w| w["id"] == after.window.unwrap_or(0))
                        .ok_or("Preview host is absent")?;
                    check(
                        after.rect[0] >= -0.5
                            && after.rect[1] >= -0.5
                            && after.rect[2] <= number(&window["width"])? + 0.5
                            && after.rect[3] <= number(&window["height"])? + 0.5,
                        "Measured split candidate overflows its native host",
                    )?;
                    return Ok(Some(after));
                }
                Ok(None)
            },
        )
    }
    pub fn resize(&self, window: u64, width: u32, height: u32, dpr: u32) -> Result<()> {
        self.client
            .resize(window, width, height, Some(f64::from(dpr)))?;
        self.until("requested native logical size and DPR", || {
            let observed = self.client.observe()?;
            let target = array(&observed["windows"])?
                .iter()
                .find(|value| value["id"] == window)
                .ok_or("Resize target retired")?;
            Ok((number(&target["width"])? - f64::from(width)).abs() <= 1.
                && (number(&target["height"])? - f64::from(height)).abs() <= 1.
                && (number(&target["dpr"])? - f64::from(dpr)).abs() <= 0.001)
        })
    }
    pub fn view(&self, kind: &str) -> Result<String> {
        let s = self.state()?;
        let found: Vec<_> = object(&s["layout"]["views"])?
            .iter()
            .filter(|(_, v)| v["kind"] == kind)
            .collect();
        check(found.len() == 1, "Missing or ambiguous editor view kind")?;
        Ok(found[0].0.clone())
    }
    pub fn group(&self, view: &str) -> Result<String> {
        let s = self.state()?;
        object(&s["layout"]["nodes"])?
            .iter()
            .find(|(_, n)| {
                n["Group"]["tabs"]
                    .as_array()
                    .is_some_and(|tabs| tabs.iter().any(|t| t == view))
            })
            .map(|(id, _)| id.clone())
            .ok_or_else(|| format!("No group for view {view}").into())
    }
    pub fn label(view: &str) -> String {
        format!("dock/view/ViewId({view})/label")
    }
    pub fn group_id(group: &str) -> String {
        format!("dock/group/NodeId({group})")
    }
    pub fn main_menu(&self, path: &str) -> Result<()> {
        self.activate(self.window()?)?;
        self.click("workspace/panels")?;
        for caption in path.split('/') {
            let node = self.find(caption, |node| {
                node.id.starts_with("dock-menu/") && node.caption() == caption
            })?;
            self.click(node.id.strip_suffix("/caption").unwrap_or(&node.id))?;
        }
        Ok(())
    }
    pub fn menu(&self, view: &str, caption: &str) -> Result<()> {
        self.until("settled dock menu owner", || {
            let state = self.state()?;
            Ok(state["navigation_input"]["pending_host"] == false
                && state["navigation_input"]["rendered"] == state["revision"])
        })?;
        let anchor = self.node(&format!("dock/node/NodeId({})/menu", self.group(view)?))?;
        self.activate(anchor.window()?)?;
        self.click(&format!("dock/node/NodeId({})/menu", self.group(view)?))?;
        if matches!(
            caption,
            "Float tab"
                | "Float group"
                | "Close group"
                | "Maximize group"
                | "Restore group"
                | "Redock to main window"
        ) {
            let branch = self.find("Layout", |node| {
                node.id.starts_with("dock-menu/") && node.caption() == "Layout"
            })?;
            self.click(branch.id.strip_suffix("/caption").unwrap_or(&branch.id))?;
        }
        for caption in caption.split('/') {
            let n = self.find(caption, |n| {
                n.id.starts_with("dock-menu/") && n.caption() == caption
            })?;
            let id = n.id.strip_suffix("/caption").unwrap_or(&n.id);
            if caption == "Redock to main window" {
                self.click_retired(id)?;
            } else {
                self.click(id)?;
            }
        }
        Ok(())
    }
    pub fn drag(&self, view: &str, point: [f64; 2]) -> Result<u64> {
        let label = self.node(&Self::label(view))?;
        let window = label.window()?;
        self.activate(window)?;
        let label = self.node(&Self::label(view))?;
        self.input(window, json!({"kind":"move","position":label.center()}))?;
        self.input(window, json!({"kind":"button","pressed":true}))?;
        for _ in 0..2 {
            self.input(window, json!({"kind":"move","position":point}))?;
        }
        Ok(window)
    }
    pub fn release(&self, window: u64) -> Result<()> {
        self.input(window, json!({"kind":"button","pressed":false}))?;
        Ok(())
    }
    pub fn frame(&self, window: u64, label: &str) -> Result<Value> {
        let o = self.client.observe()?;
        let w = array(&o["windows"])?
            .iter()
            .find(|w| w["id"] == window)
            .ok_or("Missing capture window")?;
        let size = [
            w["physical_width"]
                .as_u64()
                .ok_or("Missing physical width")? as u32,
            w["physical_height"]
                .as_u64()
                .ok_or("Missing physical height")? as u32,
        ];
        let frame = evidence::render_fence(self.client, window, self.artifacts, size)?;
        let mut value = serde_json::to_value(&frame)?;
        value["label"] = json!(label);
        self.captures.borrow_mut().push(value.clone());
        Ok(value)
    }
    pub fn open_project(&self) -> Result<PathBuf> {
        let project = self
            .project
            .ok_or("This independent component host has no project")?;
        if self.rpc("editor.state")?["phase"] == "projects" {
            self.client.operation(
                "editor.project",
                json!({"command":{"kind":"open","root":project}}),
            )?;
        }
        self.until("editor workspace", || {
            let state = self.rpc("editor.state")?;
            Ok(state["phase"] == "editor"
                && state["project"]["root"] == json!(project)
                && self.state()?["bound"] == true)
        })?;
        Ok(project.to_path_buf())
    }
    pub fn reference(&self, name: &str) -> Result<Value> {
        let project = self.project.ok_or("Missing shared test project")?;
        Ok(crate::read_json(&project.join("cases.json"))?[name].clone())
    }
    pub fn browser_folder(&self, name: &str) -> Result<()> {
        let root = self.find("Assets root", |n| {
            n.id.starts_with("browser/") && n.id.ends_with("/folder/0")
        })?;
        self.click(&root.id)?;
        self.browser_select(name)?;
        self.menu(&self.view("assets")?, "Open selected source")
    }
    pub fn browser_select(&self, name: &str) -> Result<()> {
        let n = self.find(name, |n| {
            n.caption() == name
                && n.id.starts_with("browser/")
                && n.id.contains("/row/")
                && n.id.ends_with("/name")
        })?;
        let parent = self
            .nodes()?
            .into_iter()
            .find(|p| Some(p.entity) == n.parent)
            .ok_or("Missing browser item parent")?;
        let prefix = n.id.split("/row/").next().ok_or("Missing browser owner")?;
        let viewport = self.node(&format!("{prefix}/entries"))?;
        let left = parent.rect[0].max(viewport.rect[0]);
        let top = parent.rect[1].max(viewport.rect[1]);
        let right = parent.rect[2].min(viewport.rect[2]);
        let bottom = parent.rect[3].min(viewport.rect[3]);
        check(
            right > left && bottom > top,
            "Browser source is outside its viewport",
        )?;
        // A diagnostic can leave less than one grid tile of vertical space.
        // Click its actual visible portion, never a clipped parent center.
        // Keep the aim fixed when selection clears a diagnostic and expands the
        // viewport, so two real presses still form a native double click.
        self.input(parent.window()?, json!({"kind":"click","position":[(left + right) / 2., top + (bottom - top).min(16.) / 2.]}))?;
        Ok(())
    }
    pub fn open_asset(&self, name: &str) -> Result<()> {
        let browser = self.view("assets")?;
        self.menu(&browser, "Refresh sources")?;
        self.browser_select(name)?;
        self.menu(&browser, "Open selected source")
    }
    pub fn current_document(&self) -> Result<Value> {
        let s = self.state()?;
        array(&s["documents"]["documents"])?
            .iter()
            .find(|d| d["document"] == s["documents"]["active"])
            .cloned()
            .ok_or_else(|| "No active source document".into())
    }
    pub fn selection(&self) -> Result<Value> {
        Ok(self.current_document()?["selection"].clone())
    }
}
