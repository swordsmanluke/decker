use std::collections::HashMap;

use crate::rex::TaskId;
use crate::rex::terminal::internal::{StreamState, ViewPort};
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

pub struct Cursor {
    x: i32,
    y: i32,
}

pub struct Pane {
    pub id: String,
    // Location and Dimensions
    pub x: u16,
    pub y: u16,
    pub height: u16,
    pub width: u16,

    scroll_mode: ScrollMode,

    // Viewable lines
    view_port: ViewPort,

    // current print state
    print_state: PrintStyle,

    // Input buffer
    stream_state: StreamState,
}
