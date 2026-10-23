//! Optional structured typography gates; full-frame consumers need none of these.
use crate::Result;
use serde_json::{Value, json};

/// Check an explicit reviewed source/profile/lock decision. Stale decisions fail
/// closed; this function never changes a policy or derives new acceptance.
pub fn verify_profile(
    policy: &Value,
    actual: &Value,
    lock: &str,
    source_contract: &str,
) -> Result<()> {
    if policy["schema"] != 1
        || policy["review"] != "human_reviewed"
        || policy["scope"].as_str().is_none_or(|s| s.trim().is_empty())
        || policy["rasterization"] != "accepted_as_rendered"
        || policy["rationale"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
        || policy["dependency_lock_sha256"] != lock
        || policy["profile"] != *actual
        || source_contract.len() != 64
        || policy["source_contract_sha256"] != source_contract
    {
        return Err(
            "Shared font decision is missing, stale or belongs to another native profile".into(),
        );
    }
    if policy["caption_tolerance_px"] != 0.5 {
        return Err("The reviewed caption tolerance must remain 0.5px".into());
    }
    Ok(())
}

/// Bind native measurements by authored address, never by text or nearest bounds.
/// Missing, extra and ambiguous visible native text fails, including icon-only cases.
pub fn bind_roles(snapshot: &Value, roles: &Value) -> Result<Value> {
    let roles = roles.as_object().ok_or("Missing family role bindings")?;
    let texts = snapshot["texts"]
        .as_array()
        .ok_or("Missing native text inventory")?;
    let mut result = serde_json::Map::new();
    for (role, address) in roles {
        let found: Vec<_> = texts
            .iter()
            .filter(|text| text["address"] == *address)
            .collect();
        if found.len() != 1 {
            return Err(format!("Missing or ambiguous native role {role}").into());
        }
        result.insert(role.clone(), found[0]["style"].clone());
    }
    if texts.len() != roles.len()
        || texts
            .iter()
            .any(|text| !roles.values().any(|address| *address == text["address"]))
    {
        return Err("Visible native text is not covered by a family binding".into());
    }
    Ok(Value::Object(result))
}

/// Strictly compare content, exact font metrics/color/bytes and shaping coverage.
/// Only allocated caption bounds use the reviewed half-pixel rounding allowance.
pub fn typography(expected: &Value, actual: &Value, policy: &Value) -> Result<Value> {
    let expected = expected.as_object().ok_or("Missing source roles")?;
    let actual = actual.as_object().ok_or("Missing native roles")?;
    let mut errors = Vec::new();
    if expected.keys().ne(actual.keys()) {
        errors.push("Missing or extra native text roles".to_string());
    }
    for (name, expected) in expected {
        let actual = actual.get(name).unwrap_or(&Value::Null);
        let font = &policy["fonts"][expected["font_family"]
            .as_str()
            .ok_or("Missing source font family")?][expected["font_style"]
            .as_str()
            .ok_or("Missing source font style")?];
        if font.is_null() {
            errors.push(format!("{name}: unmapped independent source font"));
        }
        for key in [
            "text",
            "size_px",
            "weight",
            "line_height_px",
            "letter_spacing_px",
            "color",
            "lines",
        ] {
            // JSON integers and floats represent the same exact metric (12 == 12.0).
            let equal = match (actual[key].as_f64(), expected[key].as_f64()) {
                (Some(a), Some(b)) => a == b,
                _ => actual[key] == expected[key] && !actual[key].is_null(),
            };
            if !equal {
                errors.push(format!(
                    "{name}: {key} differs from independent source metadata"
                ));
            }
        }
        if actual["font_path"] != font["path"]
            || actual["font_sha256"] != font["sha256"]
            || font.is_null()
        {
            errors.push(format!("{name}: wrong loaded font asset"));
        }
        if actual["unclipped"] != true {
            errors.push(format!("{name}: missing clipping proof"));
        }
        let glyphs = actual["glyphs"].as_u64();
        let shaping = &actual["shaping"];
        if glyphs.is_none()
            || (actual["text"].as_str().is_some_and(|s| !s.is_empty()) && glyphs == Some(0))
            || shaping["expected_glyphs"].as_u64() != glyphs
            || shaping["missing_glyphs"].as_u64() != Some(0)
            || shaping["coverage_complete"] != true
        {
            errors.push(format!(
                "{name}: incomplete positioned glyphs or shaped text coverage"
            ));
        }
        let rect = actual["rect"].as_array().zip(expected["rect"].as_array());
        if rect.is_none_or(|(a, b)| {
            a.len() != 4
                || b.len() != 4
                || a.iter().zip(b).any(|(a, b)| {
                    a.as_f64()
                        .zip(b.as_f64())
                        .is_none_or(|(a, b)| (a - b).abs() > 0.5)
                })
        }) {
            errors.push(format!("{name}: allocated caption bounds differ"));
        }
    }
    Ok(json!({"status":if errors.is_empty(){"passed"}else{"failed"},"errors":errors}))
}

/// Check that glyph isolation retained measurements and layout, with full paint
/// restored exactly by the producer. Does not waive stability or pixel checks.
pub fn layers(full: &Value, shape: &Value) -> Result<()> {
    if full["ready"] != true
        || shape["ready"] != true
        || full["layer"] != "full"
        || shape["layer"] != "shape"
        || shape["paint_hidden"] != true
        || ["texts", "button", "scope"]
            .iter()
            .any(|key| full[key] != shape[key])
    {
        return Err("Glyph isolation changed text inventory, layout or typography".into());
    }
    Ok(())
}

/// Preserve the raw pixel mismatch; accept only an explicitly reviewed exact
/// source/native pair under the same reviewed profile, source contract and lock.
/// Call only after capture, typography, stability and restoration have passed.
pub fn shape(
    case: &str,
    comparison: &Value,
    capture: &Value,
    source_hash: &Value,
    policy: &Value,
    lock: &str,
    contract: &str,
) -> Result<Value> {
    verify_profile(policy, &capture["profile"], lock, contract)?;
    if comparison["automated_result"] == "matched" {
        return Ok(json!({"status":"passed"}));
    }
    let variance = &policy["accepted_shape_variance"];
    let pair = json!({"figma_sha256":source_hash,"native_sha256":capture["sha256"]});
    if comparison["automated_result"] == "mismatch"
        && variance["review"] == "human_reviewed"
        && variance["rationale"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
        && variance["pairs"][case] == pair
    {
        return Ok(
            json!({"status":"accepted_known_difference","rationale":variance["rationale"],"pair":pair,"pixel_verdict":"mismatch"}),
        );
    }
    Ok(json!({"status":"failed"}))
}
