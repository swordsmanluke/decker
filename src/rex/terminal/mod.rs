use std::collections::HashMap;

use crate::rex::TaskId;
use crate::rex::terminal::internal::glyph_string::GlyphString;
use crate::rex::terminal::internal::StreamState;
use crate::rex::terminal::pane::PrintStyle;

mod pane_manager;
mod pane;
mod internal;

pub struct PaneManager {
    panes: HashMap<TaskId, Pane>,
}

#[derive(Eq, PartialEq)]
pub enum ScrollMode {
    Scroll,
    Fixed
}

struct Cursor {
    x: u16,
    y: u16,
}

pub struct Pane {
    pub id: String,
    // Location and Dimensions
    pub x: u16,
    pub y: u16,
    pub height: u16,
    pub width: u16,

    scroll_mode: ScrollMode,

    // Cached lines
    lines: Vec<GlyphString>,

    // virtual cursor location
    cursor: Cursor,

    // current print state
    print_state: PrintStyle,

    // Input buffer
    stream_state: StreamState,
}
