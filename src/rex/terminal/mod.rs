mod pane_manager;
mod glyph_string;
pub(crate) mod pane;
mod internal;

use std::collections::HashMap;
use crate::rex::TaskId;
use crate::rex::terminal::pane::Pane;

pub struct PaneManager {
    panes: HashMap<TaskId, Pane>,
}

pub struct TerminalLocation {
    pub top: u16,
    pub left: u16
}

pub struct TerminalSize {
    pub width: u16,
    pub height: u16
}