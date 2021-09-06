mod vt100_translator;
mod vt100_string;

use std::collections::HashMap;
use crate::rex::terminal::vt100_translator::StreamState;
use crate::rex::TaskId;

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