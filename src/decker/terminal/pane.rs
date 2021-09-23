use regex::Regex;
use crate::decker::terminal::internal::{StreamState, VT100, ViewPort};
use crate::decker::terminal::internal::TerminalOutput::{Plaintext, CSI};
use std::io::Write;
use log::{info};
use anyhow::bail;
use std::fmt::{Display, Formatter};
use lazy_static::lazy_static;
use crate::decker::terminal::{ScrollMode, Pane, Color, PrintStyle, DeletionType, ScreenCoord, VirtualCoord};

impl Display for Color {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{:?}", self).as_str())
    }
}

impl Color {
    pub fn eight_color(base: u8) -> Color {
        match base % 10 {
            0 => { Color::Black }
            1 => { Color::Red }
            2 => { Color::Green }
            3 => { Color::Yellow }
            4 => { Color::Blue }
            5 => { Color::Magenta }
            6 => { Color::Cyan }
            _ => { Color::White }
        }
    }

    pub fn to_offset(&self) -> u8 {
        match self {
            Color::Black => { 0 }
            Color::Red => { 1 }
            Color::Green => { 2 }
            Color::Yellow => { 3 }
            Color::Blue => { 4 }
            Color::Magenta => { 5 }
            Color::Cyan => { 6 }
            Color::White => { 7 }
            _ => { 255 }
        }
    }


    pub fn extended_color(args: &mut Vec<u8>) -> anyhow::Result<Color> {
        match args.remove(0) {
            2 => { Ok(Color::RGB(args.remove(0), args.remove(0), args.remove(0))) }
            5 => { Ok(Color::TWOFIFTYSIX(args.remove(0))) }
            c => { bail!("{} is not a valid SGR extended color argument!", c) }
        }
    }
}

impl Default for PrintStyle {
    fn default() -> Self {
        PrintStyle {
            foreground: Color::White,
            background: Color::Black,
            italicized: false,
            underline: false,
            invert: false,
            blink: false,
            bold: false,
        }
    }
}

lazy_static! {
    static ref PARAM_REGEX: Regex = Regex::new("\x1b\\[([0-9;]*)%?m").unwrap();
    static ref HOME_REGEX: Regex = Regex::new("\x1b\\[(\\d*);?(\\d*).").unwrap();
    static ref CUR_MOVE_REGEX: Regex = Regex::new("\x1b\\[(\\d*).").unwrap();
}

impl PrintStyle {
    /****
    Returns the VT100 codes required to transform self -> other, but does not mutate
     */
    pub fn diff_str(&self, other: &PrintStyle) -> String {
        let mut out = String::new();

        if self.foreground != other.foreground {
            out += &other.foreground_string();
        }

        if self.background != other.background {
            out += &other.background_string();
        }

        if self.underline != other.underline {
            if other.underline { out += "\x1b[4m" } else { out += "\x1b[24m" }
        }

        if self.blink != other.blink {
            if other.blink { out += "\x1b[5m" } else { out += "\x1b[25m" }
        }

        if self.italicized != other.italicized {
            if other.italicized { out += "\x1b[3m" } else { out += "\x1b[23m" }
        }

        if self.invert != other.invert {
            if other.invert { out += "\x1b[7m" } else { out += "\x1b[27m" }
        }

        out
    }

    pub fn to_str(&self) -> String {
        // Check colors first
        let fg_str = self.foreground_string();
        let bg_str = self.background_string();

        let blink = if self.blink {
            "\x1b[5m"
        } else {
            ""
        };

        let underlined = if self.underline {
            "\x1b[4m"
        } else {
            ""
        };

        let italicized = if self.italicized {
            "\x1b[3m"
        } else {
            ""
        };

        let mut out = String::from(fg_str);
        out.push_str(&bg_str);
        out.push_str(&blink);
        out.push_str(&underlined);
        out.push_str(&italicized);

        out
    }

    fn background_string(&self) -> String {
        let bg_base = if self.bold { 100 } else { 40 };
        let bg_str = match self.background {
            Color::TWOFIFTYSIX(num) => { format!("\x1b[38;5;{}m", num) }
            Color::RGB(r, g, b) => { format!("\x1b[38;2;{};{};{}m", r, g, b) }
            color => { format!("\x1b[{}m", bg_base + color.to_offset()) }
        };
        bg_str
    }

    fn foreground_string(&self) -> String {
        let fg_base = if self.bold { 90 } else { 30 };
        let fg_str = match self.foreground {
            Color::TWOFIFTYSIX(num) => { format!("\x1b[38;5;{}m", num) }
            Color::RGB(r, g, b) => { format!("\x1b[38;2;{};{};{}m", r, g, b) }
            color => { format!("\x1b[{}m", fg_base + color.to_offset()) }
        };
        fg_str
    }

    pub fn reset(&mut self) -> anyhow::Result<()> {
        // Keep this in sync with Self::default()
        self.foreground = Color::White;
        self.background = Color::Black;
        self.italicized = false;
        self.underline = false;
        self.invert = false;
        self.blink = false;
        self.bold = false;
        Ok(())
    }

    pub fn apply_vt100(&mut self, s: &str) -> anyhow::Result<()> {
        info!("Attempting to apply SGR command '{:?}'", s);

        match PARAM_REGEX.captures(s) {
            None => { bail!("'{:?}' does not look like an SGR sequence!", s) }
            Some(captures) => {
                let mut int_parts: Vec<u8> = captures.get(1).unwrap().as_str().
                    split(";").
                    map(|a| a.to_string().parse::<u8>()).
                    filter_map(|p| p.ok()).
                    collect();

                if int_parts.is_empty() {
                    // Special case - this is shorthand for reset
                    self.reset()?;
                }

                // until int_parts is empty, consume and apply the settings
                while !int_parts.is_empty() {
                    let sgr_code = int_parts.remove(0);

                    match sgr_code {
                        0 => {
                            /* reset */
                            self.foreground = Color::White;
                            self.background = Color::Black;
                            self.blink = false;
                            self.underline = false;
                            self.bold = false;
                        }
                        1 => { self.bold = true; }
                        2 => { self.bold = false; }
                        3 => { self.italicized = true; }
                        4 => { self.underline = true; }
                        5 => { self.blink = true; }
                        7 => { self.invert = true; }
                        22 => { self.bold = false; }
                        23 => { self.italicized = false; }
                        24 => { self.underline = false; }
                        25 => { self.blink = false; }
                        27 => { self.invert = false; }
                        30..=37 => { self.foreground = Color::eight_color(sgr_code); }
                        38 => { self.foreground = Color::extended_color(&mut int_parts)? }
                        39 => { self.foreground = Color::White }
                        40..=47 => { self.background = Color::eight_color(sgr_code); }
                        48 => { self.background = Color::extended_color(&mut int_parts)? }
                        49 => { self.foreground = Color::Black }
                        90..=97 => {
                            self.foreground = Color::eight_color(sgr_code);
                            self.bold = true;
                        }
                        100..=107 => {
                            self.background = Color::eight_color(sgr_code);
                            self.bold = true;
                        }

                        _ => { panic!("Invalid or unknown SGR code {}", sgr_code) }
                    }

                    PARAM_REGEX.captures(s).unwrap();
                }
            }
        }

        Ok(())
    }
}

impl Pane {
    pub fn new(id: &str, x: u16, y: u16, height: u16, width: u16) -> Pane {
        let view_port = ViewPort::new(id.to_string(), width, height, ScrollMode::Fixed);

        Pane {
            id: String::from(id),
            x,
            y,
            view_port,
            stream_state: StreamState::new(),
        }
    }

    pub fn width(&self) -> u16 {
        self.view_port.width()
    }

    pub fn height(&self) -> u16 {
        self.view_port.height()
    }

    pub fn set_scroll_mode(&mut self, mode: ScrollMode) {
        self.view_port.set_scroll_mode(mode);
    }

    pub fn push(&mut self, s: &str) -> anyhow::Result<()> {
        self.stream_state.push(s);

        for out in self.stream_state.consume() {
            match out {
                Plaintext(plain) => {
                    info!("{}: Processing TXT {:?} {:?}", self.id, self.view_port.cursor_loc(), plain);
                    if plain.contains("\x1B") {
                        info!("{}: plaintext contains ESC! {:?}", self.id, plain);
                    }

                    for c in plain.chars() {
                        match c {
                            '\u{8}' => {
                                /* Backspace */
                                self.view_port.cursor_left(1);
                            }
                            '\n' => {
                                info!("main: New line for \\n");
                                self.view_port.newline();
                            }
                            '\t' => {
                                // Replace tabs with 4 spaces
                                let line = self.view_port.cur_line();
                                line.push("    ", &line.last_style());
                                self.view_port.cursor_right(4);
                            }
                            '\r' => {
                                self.view_port.cursor_home();
                            }
                            '\x7F' => { /* Delete */ }
                            _ => {
                                // check to see if this is a printable character or not
                                match c as u8 {
                                    0x20..=0xFF => {
                                        // Visible characters
                                        let index = self.view_port.cursor().x();
                                        let style = self.view_port.style();
                                        let line = self.view_port.cur_line();
                                        line.set(index, c, &style);
                                        self.view_port.cursor_right(1);
                                    }
                                    _ => {
                                        // Special chars that don't have fill
                                        info!("main: Unhandled char: {:?}({})", c, c as u8);
                                        print!("{}", c);
                                    }
                                }
                            }
                        }
                    }
                }
                CSI(vt100_code) => {
                    info!("{}: Processing CSI {:?}: {:?}", self.id, self.view_port.cursor_loc(), vt100_code);
                    match vt100_code {
                        VT100::SGR(code) => { self.view_port.apply_style(&code)? }
                        VT100::ScrollDown(_) => { self.view_port.cursor_up(1); }
                        VT100::ScrollUp(_) => { self.view_port.cursor_down(1); }
                        VT100::MoveCursor(code) |
                        VT100::MoveCursorApp(code)=> {
                            /* cursor movement */
                            self.move_cursor(&code)?
                        }
                        VT100::ClearLine(code) |
                        VT100::EraseLineBeforeCursor(code) |
                        VT100::EraseLineAfterCursor(code) |
                        VT100::EraseScreen(code) => {
                            /* text deletion */
                            self.delete_text(&code)?
                        }
                        VT100::HideCursor(code) => { print!("{}", code) }
                        VT100::ShowCursor(code) => { print!("{}", code) }
                        VT100::GetCursorPos(code) => { print!("{}", code) }
                        VT100::EnterApplicationKeyMode(code) => { print!("{}", code) }
                        VT100::ExitAltKeypadMode(code) => { print!("{}", code) }
                        VT100::PassThrough(code) => {
                            /* Loads of control options */
                            match code.as_str() {
                                // Bracketed paste is safe to just send direct to STDOUT. We
                                // won't be running more than one active program at a time, so
                                // it shouldn't matter.
                                "\x1b[?2004h" | /* Bracketed paste mode ON */
                                "\x1b[?2004l" | /* Bracketed paste mode OFF */
                                "\x1b[?34h"      /* underline cursor */
                                => {
                                    // All of these can be managed by the
                                    // top level terminal emulator...
                                    // if vt100_code != "\x1b[?25l" {  /* hide cursor */
                                    print!("{}", code);
                                    // }
                                }
                                // Alternate screen
                                "\x1b[?1049h" => {
                                    /* Alternate screen ON */
                                    self.delete_text("\x1b[2J").unwrap(); // clear screen
                                }
                                "\x1b[?1049l" => {
                                    /* Alternate screen OFF */
                                    self.delete_text("\x1b[2J").unwrap(); // clear screen
                                }
                                _ => {}
                            }
                        }
                        VT100::Unknown(code) => {
                            /* Just print these directly... I guess */
                            info!("{}: Unknown CSI {:?}", self.id, code);
                            print!("{}", code);
                        }

                        // FIXME: Not yet handled
                        VT100::EnterAltKeypadMode(_) => {}
                    }
                }
            }
        }

        info!("{}: Processing line ({}): \"{:?}\"", self.id, self.view_port.cursor().y(), self.view_port.cur_line());

        Ok(())
    }

    pub fn write(&mut self, target: &mut dyn Write) -> anyhow::Result<()> {
        let mut line_idx = 0;

        let ps = self.view_port.style().clone();
        // Values cloned to avoid having immutable references inside a mutable reference to self
        let x_off = self.x;
        let y_off = self.y;
        let width = self.width();
        let pane_id = self.id.as_str();
        let mut chunks: Vec<u8> = Vec::with_capacity(1024);

        self.view_port.take_visible_lines().iter_mut().for_each(|line| {
            if line.dirty() {
                info!("{}: Printing plaintext@({}): {:?}", pane_id, line_idx, line.plaintext());
                info!("{}: glyphs: {}", pane_id, line.glyphs.len());
                line.write(x_off, y_off + line_idx, width, &ps, &mut chunks).unwrap();
            }
            line_idx += 1;
        });

        if chunks.len() > 0 {
            info!("Writing {} bytes", chunks.len());
            write!(target, "{}", String::from_utf8(chunks)?)?;
        }

        Ok(())
    }

    pub fn take_cursor(&self, target: &mut dyn Write) -> anyhow::Result<()> {
        // put cursor where it belongs (Note that screen coordinates are 1-based instead of zero based.
        let row = self.view_port.cursor().row();
        let col = self.view_port.cursor().col();

        let global_y = row + self.y as i32 - 1;
        let global_x = col + self.x as i32 - 1;

        info!("{}: Putting cursor at {}x{}y (global: {},{})", self.id, col, row, global_x, global_y);
        write!(target, "\x1b[{};{}H", global_y, global_x)?;
        Ok(())
    }

    fn delete_text(&mut self, vt100_code: &str) -> anyhow::Result<()> {
        let last_char = vt100_code.chars().last().unwrap();

        let deletion_type = match last_char {
            'L' => DeletionType::ClearLineToCursor,
            'K' => {
                match Pane::deletion_type(vt100_code) {
                    None => DeletionType::ClearLineAfterCursor,
                    Some(1) => DeletionType::ClearLineToCursor,
                    Some(2) => DeletionType::ClearLine,
                    _ => DeletionType::Unknown(vt100_code.to_string())
                }},
            'J' => {
                match Pane::deletion_type(vt100_code) {
                    None => DeletionType::ClearScreenAfterCursor,
                    Some(1) => DeletionType::ClearScreenToCursor,
                    Some(2) => DeletionType::ClearScreen,
                    _ => DeletionType::Unknown(vt100_code.to_string())
                }
            }
            _ => {
                /* Should be a 'k' string */
                match &vt100_code[0..2] {
                    "\x1Bk" => DeletionType::ClearLineAfterCursor,
                    _ => DeletionType::Unknown(vt100_code.to_string())
                }
            }
        };

        self.view_port.clear(deletion_type);

        Ok(())
    }

    fn move_cursor(&mut self, vt100_code: &str) -> anyhow::Result<()> {
        let last_char = vt100_code.chars().last().unwrap();
        match last_char {
            'H' | 'f' => {
                let captures = HOME_REGEX.captures(vt100_code).unwrap();

                let row = match captures.get(1) {
                    None => { 1 }
                    Some(m) => { m.as_str().to_owned().parse::<ScreenCoord>().unwrap_or(1) }
                };
                let col = match captures.get(2) {
                    None => { 1 }
                    Some(m) => { m.as_str().to_owned().parse::<ScreenCoord>().unwrap_or(1) }
                };

                // Subtract one to move into zero-based indices
                self.view_port.cursor_goto(row, col);
            }

            'A' => {
                let up = Pane::cursor_move_amount(vt100_code)?;
                self.view_port.cursor_up(up)
            }
            'B' => {
                let down = Pane::cursor_move_amount(vt100_code)?;
                self.view_port.cursor_down(down)
            }
            'C' => {
                let right = Pane::cursor_move_amount(vt100_code)?;
                self.view_port.cursor_right(right)
            }
            'D' => {
                let left = Pane::cursor_move_amount(vt100_code)?;
                self.view_port.cursor_left(left)
            }
            /*****
            TODO: Save/Restore cursor states
             */
            // ^[s/^[u => save/restore cursor position
            // ^7/^8 => save/restore cursor pos + print state
            _ => {} // No movement to do!
        }

        Ok(())
    }

    fn cursor_move_amount(vt100_code: &str) -> anyhow::Result<u16> {
        let captures = CUR_MOVE_REGEX.captures(vt100_code).unwrap();
        let out = match captures.get(1) {
            None => { 1 }
            Some(m) => { m.as_str().to_owned().parse::<u16>().unwrap_or(1) }
        };

        Ok(out)
    }

    fn deletion_type(vt100_code: &str) -> Option<u16> {
        let captures = CUR_MOVE_REGEX.captures(vt100_code).unwrap();
        match captures.get(1) {
            None => { None }
            Some(m) => { if m.as_str().is_empty() { None } else { Some(m.as_str().to_owned().parse::<u16>().unwrap()) } }
        }
    }

    // A Handle for testing
    fn plaintext(&mut self) -> String {
        let state = self.view_port.style();
        self.view_port.take_visible_lines().iter().
            map(|l| l.to_str(&state).to_owned()).
            collect::<Vec<String>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_displays_blank_space_on_creation() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        assert_eq!("", pane.plaintext());
    }

    #[test]
    fn it_displays_pushed_text() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("a line of text").unwrap();
        assert_eq!("a line of text\n\n\n\n\n\n\n\n\n", pane.plaintext());
    }

    #[test]
    fn it_displays_line_at_bottom_of_screen() {
        let mut pane = Pane::new("p1", 1, 1, 5, 10);
        pane.set_scroll_mode(ScrollMode::Fixed);
        pane.push("\x1B[5;1H").unwrap(); // Go to the last line
        pane.push("some text").unwrap();
        assert_eq!("\n\n\n\nsome text", pane.plaintext());
    }

    /***
    PrintStyle Tests
     */
    #[test]
    fn it_converts_simple_vt100_sgr_to_print_state() {
        let code = "\x1b[33m";
        let mut ps = PrintStyle::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::Yellow);
    }

    #[test]
    fn it_converts_bold_vt100_sgr_to_print_state() {
        let code = "\x1b[93m";
        let mut ps = PrintStyle::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::Yellow);
        assert_eq!(ps.bold, true);
    }

    #[test]
    fn it_converts_background_vt100_sgr_to_print_state() {
        let code = "\x1b[43m";
        let mut ps = PrintStyle::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.background, Color::Yellow);
    }

    #[test]
    fn it_converts_256_color_vt100_sgr_to_print_state() {
        let code = "\x1b[38;5;128m";
        let mut ps = PrintStyle::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::TWOFIFTYSIX(128));
    }

    #[test]
    fn it_converts_rgb_color_vt100_sgr_to_print_state() {
        let code = "\x1b[38;2;128;42;255m";
        let mut ps = PrintStyle::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::RGB(128, 42, 255));
    }

    #[test]
    fn it_converts_state_back_into_vt100() {
        let fg_code = "\x1b[38;2;128;42;255m";
        let bg_code = "\x1b[47m";
        let mut ps = PrintStyle::default();
        ps.apply_vt100(fg_code).unwrap();
        ps.apply_vt100(bg_code).unwrap();

        assert_eq!(ps.to_str(), fg_code.to_owned() + bg_code);
    }

    #[test]
    fn it_applies_multiple_codes_at_once() {
        let code = "\x1b[;1;33;42m";
        let mut ps = PrintStyle::default();
        ps.apply_vt100(code).unwrap();

        assert_eq!(ps.foreground, Color::Yellow);
        assert_eq!(ps.background, Color::Green);
    }

    #[test]
    fn it_finds_diff_between_states() {
        let mut red_on_black = PrintStyle::default();
        red_on_black.apply_vt100("\x1b[33m").unwrap();

        let mut red_on_cyan = PrintStyle::default();
        red_on_cyan.apply_vt100("\x1b[33m").unwrap();
        red_on_cyan.apply_vt100("\x1b[46m").unwrap();

        assert_eq!(red_on_black.diff_str(&red_on_cyan), "\x1b[46m");
    }

    #[test]
    fn it_turns_off_underline() {
        let default = PrintStyle::default();
        let mut underlined = PrintStyle::default();
        underlined.apply_vt100("\x1b[4m").unwrap();

        assert_eq!(underlined.diff_str(&default), "\x1b[24m".to_owned());
    }

    #[test]
    fn it_turns_off_blink() {
        let default = PrintStyle::default();
        let mut blinking = PrintStyle::default();
        blinking.apply_vt100("\x1b[5m").unwrap();

        assert_eq!(blinking.diff_str(&default), "\x1b[25m".to_owned());
    }

    #[test]
    fn it_turns_off_italics() {
        let default = PrintStyle::default();
        let mut blinking = PrintStyle::default();
        blinking.apply_vt100("\x1b[3m").unwrap();

        assert_eq!(blinking.diff_str(&default), "\x1b[23m".to_owned());
    }
}

