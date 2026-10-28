//! Opt-in ordered text operations and local history, not a second text engine.
//!
//! Bevy's `TextInput` still owns keyboard/pointer interaction and rendering;
//! `EditableText`/Parley still own the only buffer, selection and composition.
//! Managed inputs drain native edits before Bevy's stock batch processor. Only
//! mutation admission/clipboard sequencing is adapted; navigation stays upstream.
//! See the crate README for provenance, lifecycle and current acceptance limits.
mod history;
mod keyboard;
mod mutation;
mod session;
mod update;
use bevy::prelude::*;
pub use keyboard::{handle_keyboard, history_shortcut};
pub use session::{Command, InputEvent, Outcome, Refusal, TextSession};

/// Installs opt-in processing for inputs carrying [`TextSession`].
///
/// Requires Bevy's `TextPlugin`, `TextInputPlugin` and clipboard resource. It does
/// not replace other inputs or install a second focus/IME/clipboard service.
pub struct TextSessionPlugin;
impl Plugin for TextSessionPlugin {
    fn build(&self, app: &mut App) {
        assert!(
            app.is_plugin_added::<bevy::text::TextPlugin>()
                && app.is_plugin_added::<bevy::ui_widgets::TextInputPlugin>(),
            "TextSessionPlugin requires Bevy TextPlugin and TextInputPlugin"
        );
        app.add_systems(
            PostUpdate,
            update::apply
                .before(bevy::text::EditableTextSystems)
                .after(bevy::ui::widget::sync_editable_text_viewports),
        );
    }
}
