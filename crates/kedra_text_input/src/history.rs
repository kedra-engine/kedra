//! Local history retains native Parley state at mutation boundaries, never per frame.
use bevy::text::{EditableText, TextBrush, TextLayout};
use parley::PlainEditor;
use std::collections::VecDeque;

pub(crate) type State = PlainEditor<TextBrush>;
struct Entry {
    before: State,
    after: State,
}
#[derive(Default)]
pub(crate) struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
}
impl History {
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
    pub(crate) fn record(&mut self, before: State, after: State) {
        self.redo.clear();
        if self.undo.len() == 32 {
            self.undo.pop_front();
        }
        self.undo.push_back(Entry { before, after });
    }
    pub(crate) fn undo(&mut self, text: &mut EditableText, layout: &TextLayout) {
        if let Some(entry) = self.undo.pop_back() {
            restore(text, &entry.before, layout);
            self.redo.push(entry);
        }
    }
    pub(crate) fn redo(&mut self, text: &mut EditableText, layout: &TextLayout) {
        if let Some(entry) = self.redo.pop() {
            restore(text, &entry.after, layout);
            self.undo.push_back(entry);
        }
    }
}
pub(crate) fn restore(text: &mut EditableText, state: &State, layout: &TextLayout) {
    // Text/selection/affinity come from the native snapshot. Presentation belongs
    // to the current host, not the window/width/font at the time of the edit.
    let styles = text.editor.get_styles().clone();
    let scale = text.editor.get_scale();
    text.editor = state.clone();
    *text.editor.edit_styles() = styles;
    text.editor.set_scale(scale);
    text.editor.set_width(Some(text.viewport.size.x));
    text.editor.set_alignment(layout.justify.into());
}
