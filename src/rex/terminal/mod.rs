mod vt100_translator;
mod glyph_string;
mod pane;
mod internal;

use std::collections::HashMap;
use crate::rex::TaskId;
use crate::rex::terminal::internal::StreamState;

pub struct Vt100Translator {
    streams: HashMap<TaskId, StreamState>,
}

pub struct View {
    pub location: TerminalLocation,
    pub dimensions: TerminalSize
}

pub struct TerminalLocation {
    pub top: u16,
    pub left: u16
}

pub struct TerminalSize {
    pub width: u16,
    pub height: u16
}