//! Native operation ownership in a headless Bevy host; not OS input/render acceptance.
mod support;
mod contracts {
    mod composition;
    mod local_history;
    mod ordered_edits;
}
