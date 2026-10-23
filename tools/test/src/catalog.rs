//! Project-owned source bindings and concrete feature scenarios, not a JSON DSL.
use kedra_test::Result;
use serde_json::{Value, json};
use std::path::Path;

pub const FILE: &str = "2EZlK40cfx2Mub7GmDyI70";
pub const FLAT: &str = "editor://prefabs/browser-icon.prefab.ron";
pub const FIELD: &str = "editor://prefabs/field.prefab.ron";

pub struct Scenario {
    pub id: String,
    pub source: &'static str,
    pub document: String,
    pub node: &'static str,
    pub state: &'static str,
    pub width: u32,
}

impl Scenario {
    pub fn region(&self) -> [u32; 4] {
        let margin = if self.state == "focus" { 3 } else { 0 };
        [
            32 - margin,
            32 - margin,
            self.width + 2 * margin,
            22 + 2 * margin,
        ]
    }
    pub fn text(&self) -> &'static str {
        match self.source {
            FLAT => "",
            FIELD => "Main Camera",
            _ => "Save changes",
        }
    }
    pub fn family<'a>(&self, families: &'a Value) -> Result<&'a Value> {
        let found: Vec<_> = families["families"]
            .as_array()
            .ok_or("Missing family bindings")?
            .iter()
            .filter(|f| {
                f["documents"].as_array().is_some_and(|documents| {
                    documents
                        .iter()
                        .any(|d| d.as_str() == Some(self.document.as_str()))
                })
            })
            .collect();
        if found.len() != 1 {
            return Err("Missing or ambiguous production family binding".into());
        }
        Ok(found[0])
    }
}

pub fn scenarios(root: &Path) -> Result<Vec<Scenario>> {
    let catalog: Value = super::read_json(&root.join("docs/ui/corpus/catalog.json"))?;
    let listed = catalog["scenarios"].as_array().ok_or("Missing catalog")?;
    let mut result = Vec::new();
    let mut push = |id: String,
                    source: &'static str,
                    node: &'static str,
                    state: &'static str,
                    width: u32|
     -> Result<()> {
        let found: Vec<_> = listed.iter().filter(|entry| entry["id"] == id).collect();
        if found.len() != 1 {
            return Err("Missing or ambiguous catalog scenario".into());
        }
        let entry = found[0];
        let document = entry["params"]["document"]
            .as_str()
            .ok_or("Missing source document identity")?;
        uuid::Uuid::parse_str(document)?;
        if entry["figma"] != node
            || entry["params"] != json!({"document":document,"state":state})
            || entry["size"] != json!([640, 360])
            || entry["recipe"] != "component"
        {
            return Err("Catalog binding differs from concrete Rust scenario".into());
        }
        result.push(Scenario {
            id,
            source,
            document: document.into(),
            node,
            state,
            width,
        });
        Ok(())
    };
    for (prefix, source, nodes, width) in [
        (
            "button-quiet",
            "editor://prefabs/button.prefab.ron",
            ["352:1516", "352:1519", "352:1522", "352:1525", "352:1529"],
            110,
        ),
        (
            "button-secondary",
            "editor://prefabs/raised-button.prefab.ron",
            ["352:1532", "352:1535", "352:1538", "352:1541", "352:1545"],
            110,
        ),
        (
            "button-primary",
            "editor://prefabs/primary-button.prefab.ron",
            ["352:1548", "352:1551", "352:1554", "352:1557", "352:1561"],
            110,
        ),
        (
            "button-flat",
            FLAT,
            ["352:1564", "352:1571", "352:1578", "352:1585", "352:1593"],
            22,
        ),
    ] {
        for (index, state) in ["default", "hover", "pressed", "focus", "disabled"]
            .into_iter()
            .enumerate()
        {
            let id = if state == "default" {
                prefix.into()
            } else if prefix == "button-primary" {
                format!("button-{state}")
            } else {
                format!("{prefix}-{state}")
            };
            push(id, source, nodes[index], state, width)?;
        }
    }
    for (state, node) in [("default", "352:1600"), ("focus", "352:1604")] {
        push(format!("field-{state}"), FIELD, node, state, 172)?;
    }
    // Catalog is descriptive source metadata; behavior remains Rust above.
    if listed.len() != result.len() || catalog["figma_file"] != FILE {
        return Err("Catalog scope differs from Rust scenarios".into());
    }
    Ok(result)
}
