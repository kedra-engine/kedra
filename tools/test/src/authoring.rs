//! Explicit source intake/plugin generation, separate from ordinary native runs.
use crate::{
    catalog::{FILE, Scenario},
    sources,
};
use base64::Engine;
use kedra_test::{Result, evidence};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub fn import(bundle: &Path, output: &Path, scenarios: &[Scenario]) -> Result<()> {
    let bundle: Value = serde_json::from_slice(&evidence::read(bundle, 8 * 1024 * 1024)?)?;
    if bundle["schema"] != 1 || bundle.as_object().is_none_or(|o| o.len() != 2) {
        return Err("Invalid or incomplete explicit source bundle".into());
    }
    let exports = bundle["exports"]
        .as_array()
        .ok_or("Missing explicit source exports")?;
    if exports.len() != scenarios.len() {
        return Err("Incomplete source export cases".into());
    }
    let parent = output.parent().ok_or("Missing source output parent")?;
    let staging = tempfile::tempdir_in(parent)?;
    let mut manifest = json!({"schema":1,"cases":{}});
    for entry in exports {
        if entry.as_object().is_none_or(|o| o.len() != 3) {
            return Err("Unsupported source bundle entry".into());
        }
        let case = entry["metadata"]["case"]
            .as_str()
            .ok_or("Missing source case")?;
        if !scenarios.iter().any(|s| s.id == case) || !manifest["cases"][case].is_null() {
            return Err("Unsupported or duplicate source case".into());
        }
        for layer in ["metadata", "full", "shape"] {
            let filename = if layer == "metadata" {
                format!("{case}.json")
            } else {
                format!("{case}.{layer}.png")
            };
            let bytes = if layer == "metadata" {
                serde_json::to_vec_pretty(&entry[layer])?
            } else {
                base64::engine::general_purpose::STANDARD
                    .decode(entry[layer].as_str().ok_or("Missing encoded source PNG")?)?
            };
            if bytes.len() > 65536 {
                return Err("Source asset exceeds size bound".into());
            }
            fs::write(staging.path().join(&filename), &bytes)?;
            manifest["cases"][case][layer] =
                json!({"file":filename,"sha256":evidence::digest(&bytes)});
        }
    }
    fs::write(
        staging.path().join("text-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let sources = sources::load(staging.path(), scenarios)?;
    sources::copy(staging.path(), output, &sources)?;
    kedra_test::visual::inspect_reference(
        &output.join("integrity-manifest.json"),
        &parent.join("integrity"),
    )?;
    eprintln!(
        "SUCCESS: independent sources retained at {}; no references approved",
        output.display()
    );
    Ok(())
}

pub fn plugin(root: &Path, output: &Path, scenarios: &[Scenario]) -> Result<()> {
    let families = crate::read_json(&root.join("docs/ui/corpus/text-families.json"))?;
    let requests=scenarios.iter().map(|s|Ok(json!({"case":s.id,"node":s.node,"region":s.region(),
        "roles":s.family(&families)?["roles"].as_object().ok_or("Missing source roles")?.keys().collect::<Vec<_>>()}))).collect::<Result<Vec<_>>>()?;
    fs::create_dir(output)?;
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({"name":"Kedra text source export",
        "id":"kedra-text-source-export","api":"1.0.0","main":"code.js","editorType":["figma"],
        "documentAccess":"dynamic-page","enablePrivatePluginApi":true,"networkAccess":{"allowedDomains":["none"]}}))?,
    )?;
    let ui = r#"<!doctype html><meta charset="utf-8"><p id="status">Exporting independent Figma sources…</p><a id="download" hidden>Download source bundle</a>
<script>onmessage=event=>{const data=event.data.pluginMessage;if(!data)return;
document.getElementById('status').textContent=data.message||data.type;
if(data.type==='complete'||data.type==='partial'){const a=document.getElementById('download');a.href=URL.createObjectURL(new Blob([JSON.stringify(data.bundle)],{type:'application/json'}));a.download=data.type==='complete'?'kedra-text-sources.json':'kedra-text-sources-incomplete.json';a.hidden=false;}};</script>"#;
    let caller = format!(
        r#"
// Explicit project source requests; no native captures or approval.
globalThis.kedraTextExport=(async()=>{{
figma.showUI({},{{width:420,height:160}});
const exports=[];let currentCase=null;
const encode=result=>({{metadata:result.metadata,full:result.full?figma.base64Encode(result.full):null,shape:result.shape?figma.base64Encode(result.shape):null}});
try{{
if(figma.fileKey!=={})throw Error("Open the exact Kedra UI-KIT file before exporting");
const page=await figma.getNodeByIdAsync("352:1515");await figma.setCurrentPageAsync(page);
for(const request of {}){{currentCase=request.case;figma.ui.postMessage({{type:"progress",message:request.case}});exports.push(encode(await exportTextScenario(figma,request)));}}
figma.ui.postMessage({{type:"complete",message:"Export complete; no references approved",bundle:{{schema:1,exports}}}});
}}catch(error){{figma.ui.postMessage({{type:"partial",message:String(error),bundle:{{schema:1,complete:false,exports,failed:{{case:currentCase,error:String(error),available:error.partial?encode(error.partial):null}}}}}});}}
}})();
"#,
        serde_json::to_string(ui)?,
        serde_json::to_string(FILE)?,
        serde_json::to_string(&requests)?
    );
    fs::write(
        output.join("code.js"),
        fs::read_to_string(root.join("tools/test/figma_text_export.js"))? + &caller,
    )?;
    fs::write(
        output.join("requests.json"),
        serde_json::to_vec_pretty(&requests)?,
    )?;
    fs::write(
        output.join("README.md"),
        "# Local Figma source export\n\nImport manifest.json as a local development plugin in the exact UI-KIT file.\nRun, download the bundle, then use `cargo xtask test -- export --text-bundle FILE`.\nNo network, publishing or reference approval. Incomplete diagnostics cannot be imported.\n",
    )?;
    eprintln!(
        "SUCCESS: local source plugin: {}",
        output.join("manifest.json").display()
    );
    Ok(())
}
