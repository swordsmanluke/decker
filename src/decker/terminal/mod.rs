use std::collections::HashMap;

use crate::decker::TaskId;
use crate::decker::terminal::internal::{StreamState, ViewPort};

mod pane_manager;
mod pane;
mod internal;

pub struct PaneManager {
    panes: HashMap<TaskId, Pane>,
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    TWOFIFTYSIX(u8),
    RGB(u8, u8, u8),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct PrintStyle {
    pub foreground: Color,
    pub background: Color,
    pub italicized: bool,
    pub underline: bool,
    pub blink: bool,
    pub bold: bool,
    pub invert: bool,
}


#[derive(Eq, PartialEq)]
pub enum ScrollMode {
    Scroll,
    Fixed
}

#[derive(Debug)]
pub enum DeletionType {
    ClearLine,
    ClearLineToCursor,
    ClearLineAfterCursor,
    ClearScreen,
    ClearScreenToCursor,
    ClearScreenAfterCursor,
    Unknown(String)
}

pub struct Cursor {
    x: i32,
    y: i32,
    x_max: i32,
    y_max: i32
}

pub struct Pane {
    pub id: String,
    // Location and Dimensions
    pub x: u16,
    pub y: u16,

    // Viewable area
    view_port: ViewPort,

    // Input buffer
    stream_state: StreamState,
}