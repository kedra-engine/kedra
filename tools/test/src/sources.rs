//! Explicit offline Figma input validation; no reference promotion or live service.
use crate::{
    catalog::{FILE, Scenario},
    read_json,
};
use kedra_test::{Result, evidence};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub struct Source {
    pub metadata: Value,
    pub assets: Value,
}

pub fn local(root: &Path, value: &Value) -> Result<PathBuf> {
    let path = Path::new(value.as_str().ok_or("Missing source-relative path")?);
    if path
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Source path must remain relative".into());
    }
    let resolved = root.join(path).canonicalize()?;
    if !resolved.starts_with(root.canonicalize()?) {
        return Err("Source symlink escapes package".into());
    }
    Ok(resolved)
}

fn keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("Expected source object")?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("Missing or unsupported source fields".into());
    }
    Ok(())
}

fn validate_role(role: &Value) -> Result<()> {
    keys(
        role,
        &[
            "node",
            "text",
            "font_family",
            "font_style",
            "size_px",
            "weight",
            "line_height_px",
            "letter_spacing_px",
            "color",
            "rect",
            "lines",
        ],
    )?;
    for key in ["node", "text", "font_family", "font_style"] {
        if role[key]
            .as_str()
            .is_none_or(|s| key != "text" && s.is_empty())
        {
            return Err("Invalid source text identity".into());
        }
    }
    for key in ["size_px", "weight", "line_height_px", "letter_spacing_px"] {
        if role[key].as_f64().is_none_or(|n| !n.is_finite()) {
            return Err("Invalid source font metric".into());
        }
    }
    if role["size_px"].as_f64().is_none_or(|n| n <= 0.)
        || role["line_height_px"].as_f64().is_none_or(|n| n <= 0.)
        || role["weight"]
            .as_f64()
            .is_none_or(|n| !(1. ..=1000.).contains(&n))
        || role["lines"].as_u64().is_none_or(|n| n < 1)
        || role["color"].as_array().is_none_or(|color| {
            color.len() != 4 || color.iter().any(|c| c.as_u64().is_none_or(|n| n > 255))
        })
    {
        return Err("Invalid source text metrics or RGBA".into());
    }
    let rect = role["rect"]
        .as_array()
        .ok_or("Missing source text bounds")?;
    if rect.len() != 4
        || rect
            .iter()
            .any(|n| n.as_f64().is_none_or(|v| !v.is_finite()))
        || rect[2].as_f64() < rect[0].as_f64()
        || rect[3].as_f64() <= rect[1].as_f64()
    {
        return Err("Invalid source text bounds".into());
    }
    Ok(())
}

pub fn load(root: &Path, scenarios: &[Scenario]) -> Result<BTreeMap<String, Source>> {
    let manifest = read_json(&root.join("text-manifest.json"))?;
    keys(&manifest, &["schema", "cases"])?;
    if manifest["schema"] != 1 {
        return Err("Unsupported text source schema".into());
    }
    let mut sources = BTreeMap::new();
    for scenario in scenarios {
        let assets = &manifest["cases"][&scenario.id];
        keys(assets, &["full", "shape", "metadata"])?;
        for asset in assets.as_object().ok_or("Missing source assets")?.values() {
            keys(asset, &["file", "sha256"])?;
            let path = local(root, &asset["file"])?;
            let bytes = evidence::read(&path, 65536)?;
            if asset["sha256"] != evidence::digest(&bytes) {
                return Err("Independent source digest changed".into());
            }
        }
        let metadata = read_json(&local(root, &assets["metadata"]["file"])?)?;
        keys(
            &metadata,
            &[
                "schema",
                "exporter",
                "case",
                "node",
                "file_key",
                "size",
                "region",
                "roles",
                "instances",
                "shape_derivation",
            ],
        )?;
        if metadata["schema"] != 1
            || metadata["exporter"] != "kedra-text-source-v1"
            || metadata["case"] != scenario.id
            || metadata["node"] != scenario.node
            || metadata["file_key"] != FILE
            || metadata["size"] != json!([640, 360])
            || metadata["region"] != json!(scenario.region())
            || metadata["shape_derivation"]
                != "Figma temporary clone; text opacity zero; layout retained"
        {
            return Err("Contradictory independent Figma source identity or fixed region".into());
        }
        for role in metadata["roles"]
            .as_object()
            .ok_or("Missing structured text roles")?
            .values()
        {
            validate_role(role)?;
        }
        let instances = metadata["instances"]
            .as_array()
            .ok_or("Missing instance provenance")?;
        if instances.len() != 1 {
            return Err("Expected exactly one canonical variant".into());
        }
        for instance in instances {
            keys(instance, &["node", "component", "key", "properties"])?;
            if ["node", "component", "key"]
                .iter()
                .any(|key| instance[key].as_str().is_none_or(|s| s.is_empty()))
                || !instance["properties"].is_object()
            {
                return Err("Invalid canonical instance provenance".into());
            }
        }
        let state = &instances[0]["properties"]["State"];
        if state["type"] != "VARIANT"
            || state["value"]
                .as_str()
                .is_none_or(|s| s.to_lowercase() != scenario.state)
        {
            return Err("Independent variant state differs from Rust scenario".into());
        }
        sources.insert(
            scenario.id.clone(),
            Source {
                metadata,
                assets: assets.clone(),
            },
        );
    }
    Ok(sources)
}

pub fn fingerprint(sources: &BTreeMap<String, Source>, families: &Value) -> Result<String> {
    let cases: BTreeMap<_,_> = sources.iter().map(|(case,source)| (case,json!({"metadata":source.metadata,
        "full_sha256":source.assets["full"]["sha256"],"shape_sha256":source.assets["shape"]["sha256"]}))).collect();
    Ok(evidence::digest(&serde_json::to_vec(
        &json!({"families":families,"cases":cases}),
    )?))
}

pub fn copy(root: &Path, output: &Path, sources: &BTreeMap<String, Source>) -> Result<()> {
    fs::create_dir(output)?;
    let mut manifest = json!({"schema":1,"cases":{}});
    let mut frames = serde_json::Map::new();
    for (case, source) in sources {
        for layer in ["full", "shape", "metadata"] {
            let filename = if layer == "metadata" {
                format!("{case}.json")
            } else {
                format!("{case}.{layer}.png")
            };
            let bytes = evidence::read(&local(root, &source.assets[layer]["file"])?, 65536)?;
            if source.assets[layer]["sha256"] != evidence::digest(&bytes) {
                return Err("Source changed during packaging".into());
            }
            fs::write(output.join(&filename), bytes)?;
            manifest["cases"][case][layer] =
                json!({"file":filename,"sha256":source.assets[layer]["sha256"]});
            if layer != "metadata" {
                frames.insert(format!("{case}/{layer}"),json!({"image":filename,
                "sha256":source.assets[layer]["sha256"],"size":[640,360],"kind":"design_reference","review":"pending",
                "source":format!("https://www.figma.com/design/{FILE}/UI-KIT?node-id={}",source.metadata["node"].as_str().ok_or("Missing node")?.replace(':',"-"))}));
            }
        }
    }
    fs::write(
        output.join("text-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        output.join("integrity-manifest.json"),
        serde_json::to_vec_pretty(&json!({"schema":1,
        "export":"Original full and derived glyph-free Figma source PNGs","purpose":"Integrity only; no reference approval","frames":frames}))?,
    )?;
    Ok(())
}
