use crate::decker::terminal::internal::TerminalOutput::{CSI, Plaintext};
use std::str::FromStr;
use crate::decker::terminal::internal::VT100::{SGR, PassThrough, MoveCursor, EraseScreen, ClearLine, Unknown, ScrollDown, ScrollUp, MoveCursorApp, HideCursor, ShowCursor, GetCursorPos, EnterApplicationKeyMode, ExitAltKeypadMode, EraseLineAfterCursor, EraseLineBeforeCursor, EnterAltKeypadMode};
use anyhow::Error;
use std::fmt::Debug;
use crate::decker::terminal::internal::glyph_string::GlyphString;
use crate::decker::terminal::{Cursor, ScrollMode, PrintStyle};

pub mod glyph_string;

mod stream_state;
mod view_port;
mod cursor;

enum VT100State {
    PlainText,
    FoundEsc,
}

pub(crate) struct ViewPort {
    pane_id: String,
    visible_lines: Vec<GlyphString>,
    cur_style: PrintStyle,
    scroll_mode: ScrollMode,
    width: u16,
    height: u16,
    cursor: Cursor,
}

/***
Output is either plaintext or a VT100 command sequence instruction
 */
#[derive(Clone, Debug)]
pub enum TerminalOutput {
    Plaintext(String),
    CSI(VT100),
}

/*
Classifications of VT100 codes.
Each contains its own string, but this makes it easy to detect and switch
on different types. No need to inspect the last character at use time.
 */
#[derive(Clone, Debug)]
pub enum VT100 {
    ScrollDown(String),
    ScrollUp(String),
    SGR(String),
    MoveCursor(String),
    MoveCursorApp(String),
    ClearLine(String),
    EraseLineAfterCursor(String),
    EraseLineBeforeCursor(String),
    EraseScreen(String),
    PassThrough(String),
    HideCursor(String),
    ShowCursor(String),
    GetCursorPos(String),
    EnterApplicationKeyMode(String),
    EnterAltKeypadMode(String),
    ExitAltKeypadMode(String),
    Unknown(String),
}

impl VT100 {
    pub fn to_string(&self) -> String {
        match self {
            ScrollDown(s) => { s.clone() }
            ScrollUp(s) => { s.clone() }
            SGR(s) => { s.clone() }
            MoveCursor(s) => { s.clone() }
            MoveCursorApp(s) => { s.clone() }
            ClearLine(s) => { s.clone() }
            EraseLineBeforeCursor(s) => { s.clone() }
            EraseLineAfterCursor(s) => { s.clone() }
            EraseScreen(s) => { s.clone() }
            HideCursor(s) => { s.clone() }
            ShowCursor(s) => { s.clone() }
            PassThrough(s) => { s.clone() }
            GetCursorPos(s) => { s.clone() }
            Unknown(s) => { s.clone() }
            EnterApplicationKeyMode(s) => { s.clone() }
            EnterAltKeypadMode(s) => { s.clone() }
            ExitAltKeypadMode(s) => { s.clone() }
        }
    }
}

impl FromStr for VT100 {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(anyhow::anyhow!("Cannot parse an empty string"));
        }

        let vt100 = match s.chars().last().unwrap() {
            'M' => ScrollDown(s.to_string()),
            'D' => {
                // D can be either ESC D which means Scroll Up
                // OR it can be ESC [#D which means Move left.
                if s == "\x1BD" {
                    ScrollUp(s.to_string())
                } else {
                    MoveCursor(s.to_string())
                }
            }
            'm' => SGR(s.to_string()),
            'H' | 'f' | 'A' | 'B' | 'C' => {
                /* cursor movement */
                if s.get(1..2).unwrap() == "O" {
                    // When alternate mode is set, arrow keys send ESC O[A-D] instead of ESC[[A-D]
                    // This can trip up e.g. vim.
                    MoveCursorApp(s.to_string())
                } else {
                    MoveCursor(s.to_string())
                }
            }
            'J' => EraseScreen(s.to_string()),
            'K' => match s {
                "\x1B[1K" => EraseLineBeforeCursor(s.to_string()),
                "\x1B[2K" => ClearLine(s.to_string()),
                _ => EraseLineAfterCursor(s.to_string())
            }
            'L' => ClearLine(s.to_string()),
            'h' | 'l' | 'n' | 'r' => /* Various control / query options */
                match s {
                    "\x1b[?1h" => EnterApplicationKeyMode(s.to_string()),
                    "\x1b[?25l" => HideCursor(s.to_string()),
                    "\x1b[?25h" => ShowCursor(s.to_string()),
                    "\x1b[6n" => GetCursorPos(s.to_string()),
                    _ => PassThrough(s.to_string())
                }
            _ => {
                if s[0..2] == *"\x1Bk" {
                    ClearLine(s.to_string())
                } else {
                    Unknown(s.to_string())
                }
            }
        };

        Ok(vt100)
    }
}

impl TerminalOutput {
    pub fn to_string(&self) -> String {
        match self {
            Plaintext(s) => { s.clone() }
            CSI(s) => { s.to_string() }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Plaintext(s) => { s.len() == 0 }
            CSI(s) => { s.to_string().len() == 0 }
        }
    }
}

pub(crate) struct StreamState {
    buffer: String,
    vetted_output: Vec<TerminalOutput>,
    build_state: VT100State,
}
