//! Offline, credential-free evidence reports shared by native consumers.
use crate::Result;
use serde_json::Value;
use std::{fs, path::Path};

pub(crate) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn link(value: &Value, suffix: &str) -> Option<String> {
    let value = value.as_str()?;
    if value.starts_with('/')
        || value.split('/').any(|p| p == "..")
        || !value.ends_with(suffix)
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./-".contains(c))
    {
        return None;
    }
    Some(escape(value))
}

/// Write JSON plus an offline HTML evidence queue. Input must contain portable
/// public evidence only, never raw credentials, logs or unrestricted RPC replies.
/// Escapes text and restricts image/detail links; preserves raw comparator verdicts.
/// Existing reports can be updated during a run, but images/references are not modified.
pub fn write(root: &Path, report: &Value) -> Result<()> {
    fs::create_dir_all(root)?;
    fs::write(root.join("report.json"), serde_json::to_vec_pretty(report)?)?;
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Native test evidence</title><style>body{font:16px/1.5 system-ui;margin:24px;background:#202020;color:#eee}a{color:#d7a6ff}pre{white-space:pre-wrap;overflow-wrap:anywhere}section{border-top:1px solid #666;margin-top:24px}.pair{display:flex;gap:16px;flex-wrap:wrap}figure{margin:0;max-width:100%}img{max-width:640px;width:100%;image-rendering:auto}table{border-collapse:collapse}td,th{padding:8px;text-align:left;border-bottom:1px solid #666}</style>",
    );
    let notice = if report["verification"] == "behavior_only" {
        "Behavior checks only. GPU fences do not provide screenshot coverage or visual acceptance. Native input is injected Bevy input, not physical OS certification."
    } else {
        "No reference is automatically approved. Known differences retain their original pixel mismatch. Native input is injected Bevy input, not physical OS certification."
    };
    html.push_str(&format!("<h1>Native tests · {}</h1><p>{}</p><p><a href=\"report.json\">Raw evidence</a></p><pre>{}</pre>", escape(report["result"].as_str().unwrap_or("failed")), escape(notice), escape(report["error"].as_str().or_else(||report["setup"]["error"].as_str()).unwrap_or(""))));
    if let Some(cases) = report["cases"].as_object() {
        for (case, result) in cases {
            html.push_str(&format!(
                "<section><h2>{} · {}</h2><pre>{}</pre>",
                escape(case),
                escape(result["status"].as_str().unwrap_or("not_run")),
                escape(result["error"].as_str().unwrap_or(""))
            ));
            for key in ["typography", "shape", "capture_errors"] {
                if let Some(value) = result.get(key) {
                    html.push_str(&format!(
                        "<h3>{key}</h3><pre>{}</pre>",
                        escape(&serde_json::to_string_pretty(value)?)
                    ));
                }
            }
            if let Some(roles) = result["expected_roles"].as_object() {
                html.push_str("<table><tr><th>Role</th><th>Independent source size/RGBA</th><th>Native size/RGBA</th></tr>");
                for (name, expected) in roles {
                    let actual = &result["actual_roles"][name];
                    html.push_str(&format!(
                        "<tr><td>{}</td><td>{} px / {}</td><td>{} px / {}</td></tr>",
                        escape(name),
                        escape(&expected["size_px"].to_string()),
                        escape(&expected["color"].to_string()),
                        escape(&actual["size_px"].to_string()),
                        escape(&actual["color"].to_string())
                    ));
                }
                html.push_str("</table>");
            }
            if let Some(comparisons) = result["comparisons"].as_object() {
                for (layer, comparison) in comparisons {
                    html.push_str(&format!(
                        "<h3>{} · {}% local changed pixels · {}</h3><div class=\"pair\">",
                        escape(layer),
                        escape(&comparison["changed_percent"].to_string()),
                        escape(comparison["automated_result"].as_str().unwrap_or("blocked"))
                    ));
                    for key in ["reference", "actual", "difference"] {
                        if let Some(path) = link(&comparison[key], ".png") {
                            html.push_str(&format!("<figure><figcaption>{key}</figcaption><a href=\"{path}\"><img loading=\"lazy\" src=\"{path}\" alt=\"{key}\"></a></figure>"));
                        }
                    }
                    html.push_str("</div>");
                    if let Some(path) = link(&comparison["report"], ".html") {
                        html.push_str(&format!(
                            "<p><a href=\"{path}\">Original comparator detail</a></p>"
                        ));
                    }
                }
            }
            for key in ["evidence", "renderer_frames"] {
                if let Some(value) = result.get(key) {
                    html.push_str(&format!(
                        "<details><summary>{key}</summary><pre>{}</pre></details>",
                        escape(&serde_json::to_string_pretty(value)?)
                    ));
                }
            }
            html.push_str(&format!("<details><summary>Measurements and native frames</summary><pre>{}</pre></details></section>", escape(&serde_json::to_string_pretty(&result["runs"])?)));
        }
    }
    html.push_str("</html>");
    fs::write(root.join("index.html"), html)?;
    Ok(())
}
