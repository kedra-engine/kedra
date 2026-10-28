// Portions adapted from Bevy 0.20.0 crates/bevy_text/src/text_edit.rs.
// Copyright Bevy contributors. SPDX-License-Identifier: MIT OR Apache-2.0.
// See ../README.md and ../LICENSE-MIT for provenance and retained license notice.
use crate::{Refusal, session::validate};
use bevy::text::TextBrush;
use parley::PlainEditorDriver;

pub(crate) fn insert(
    driver: &mut PlainEditorDriver<'_, TextBrush>,
    value: &str,
    limit: usize,
) -> Result<(), Refusal> {
    check(driver.editor, value, limit)?;
    driver.insert_or_replace_selection(value);
    Ok(())
}
pub(crate) fn check(
    editor: &parley::PlainEditor<TextBrush>,
    value: &str,
    limit: usize,
) -> Result<(), Refusal> {
    validate(value, limit)?;
    let selected = editor
        .selected_text()
        .map_or(0, |text| text.chars().count());
    if editor.text().chars().count() - selected + value.chars().count() > limit {
        return Err(Refusal::ScalarLimit);
    }
    Ok(())
}
