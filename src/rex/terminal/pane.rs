use crate::rex::terminal::glyph_string::{GlyphString, Glyph};
use regex::Regex;
use crate::rex::terminal::internal::StreamState;
use crate::rex::terminal::internal::TerminalOutput::{Plaintext, CSI};
use std::cmp::{min, max};
use std::io::Write;
use log::{info, warn, error};
use anyhow::bail;

pub struct Pane {
    id: String,
    // Location and Dimensions
    x: u16,
    y: u16,
    height: u16,
    width: u16,

    // Cached lines
    lines: Vec<GlyphString>,

    // virtual cursor location
    cursor: Cursor,

    // current print state
    print_state: PrintStyle,

    // Input buffer
    stream_state: StreamState,
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
}

struct Cursor {
    x: u16,
    y: u16,
}

impl Cursor {
    pub fn set_x(&mut self, n: u16) {
        self.x = n
    }

    pub fn set_y(&mut self, n: u16) {
        info!("Changing cursor y from {} to {}", self.y, n);
        self.y = n
    }

    pub fn incr_x(&mut self, offset: u16) {
        self.set_x(self.x + offset)
    }

    pub fn incr_y(&mut self, offset: u16) {
        self.set_y(self.y + offset)
    }

    pub fn decr_x(&mut self, offset: u16) {
        self.set_x(self.x - offset)
    }

    pub fn decr_y(&mut self, offset: u16) {
        self.set_y(self.y - offset)
    }

    pub fn new() -> Self {
        Cursor {
            x: 1, // screen is 1-indexed
            y: 1
        }
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


    pub fn extended_color(args: &[u8]) -> anyhow::Result<Color> {
        match args.first().unwrap() {
            2 => { Ok(Color::RGB(args[1], args[2], args[3])) }
            5 => { Ok(Color::TWOFIFTYSIX(args[1])) }
            _ => { bail!("{} is not a valid SGR extended color argument!", args.first().unwrap()) }
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
            blink: false,
            bold: false,
        }
    }
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
            if other.underline { out += "\x1b[4m" }
            else { out += "\x1b[24m" }
        }

        if self.blink != other.blink {
            if other.blink { out += "\x1b[5m" }
            else { out += "\x1b[25m" }
        }

        if self.italicized != other.italicized {
            if other.italicized { out += "\x1b[3m" }
            else { out += "\x1b[23m" }
        }

        out
    }

    pub fn to_str(&self) -> String {
        // TODO: Assemble a set of numbers to push together into a single command.

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

    pub fn apply_vt100(&mut self, s: &str) -> anyhow::Result<()> {
        let parm_rx = Regex::new("\x1b\\[([0-9;]+)%?m").unwrap();
        match parm_rx.captures(s) {
            None => { bail!("'{}' does not look like an SGR sequence!", s) }
            Some(captures) => {
                let int_parts: Vec<u8> = captures.get(1).unwrap().as_str().
                    split(";").
                    map(|a| a.to_string().parse::<u8>().unwrap()).
                    collect();

                let sgr_code = int_parts.first().unwrap();
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
                    3 => { self.italicized = true;}
                    4 => { self.underline = true; }
                    5 => { self.blink = true; }
                    22 => { self.bold = false; }
                    23 => { self.italicized = false; }
                    24 => { self.underline = false; }
                    25 => { self.blink = false; }
                    30..=37 => { self.foreground = Color::eight_color(*sgr_code); }
                    38 => { self.foreground = Color::extended_color(&int_parts[1..])? }
                    40..=47 => { self.background = Color::eight_color(*sgr_code); }
                    48 => { self.background = Color::extended_color(&int_parts[1..])? }
                    90..=97 => {
                        self.foreground = Color::eight_color(*sgr_code);
                        self.bold = true;
                    }
                    100..=107 => {
                        self.background = Color::eight_color(*sgr_code);
                        self.bold = true;
                    }

                    _ => { panic!("Invalid or unknown SGR code {}", sgr_code) }
                }

                parm_rx.captures(s).unwrap();
                Ok(())
            }
        }
    }
}

impl Pane {
    pub fn new(id: &str, x: u16, y: u16, height: u16, width: u16) -> Pane {
        let lines = (0..height).map(|_| GlyphString::new()).collect::<Vec<GlyphString>>();

        Pane {
            id: String::from(id),
            x,
            y,
            height,
            width,
            lines,
            cursor: Cursor::new(),
            print_state: PrintStyle::default(),
            stream_state: StreamState::new(),
        }
    }

    pub fn push(&mut self, s: &str) -> anyhow::Result<()> {
        self.stream_state.push(s);

        for out in self.stream_state.consume() {
            match out {
                Plaintext(plain) => {
                    info!("Printing plaintext: {:?}", plain);

                    for c in plain.chars() {
                        match c {
                            '\u{7}' => { /* Backspace */
                                let line = self.lines.get_mut((self.cursor.y - 1) as usize).unwrap();
                                if self.cursor.x > 1 {
                                    self.cursor.decr_x(1);
                                    line.delete_at(self.cursor.x as usize);
                                }
                            }
                            '\n' => {
                                // Special char \n creates a new line.
                                // Advance the cursor and reset to the start position.
                                self.cursor.set_x(1);
                                self.cursor.incr_y(1);

                                // If we advance beyond the end of the pane bounds
                                // discard the topmost line of output and add a new
                                // line to the end.
                                if self.cursor.y >= self.height {
                                    self.cursor.set_y(self.height);
                                    self.lines.push(GlyphString::new());
                                }
                            }
                            '\r' => {
                                // Return to the start of this line!
                                self.cursor.set_x(1);
                            }

                            _ => {
                                info!("Pressed key: {:?}", c);
                                let vert_line = self.cursor.y - 1;
                                let line = self.lines.get_mut(vert_line as usize).unwrap();
                                line.set((self.cursor.x - 1) as usize, Glyph::new(c, self.print_state));
                                self.cursor.incr_x(1);
                            }
                        }
                    }
                }
                CSI(vt100_code) => {
                    // Determine the type of escape sequence and either
                    // 1) Update the print state
                    // 2) Move the cursor
                    // 3) Clear some text
                    // 4) Print to the terminal as if it were plaintext
                    info!("Handling CSI: {:?}", vt100_code);
                    let last_char = vt100_code.chars().last().unwrap();
                    match last_char {
                        'm' => { self.print_state.apply_vt100(&vt100_code)? }
                        'H' | 'f' | 'A' | 'B' | 'C' | 'D' => {
                            /* cursor movement */
                            self.move_cursor(&vt100_code)?
                        }
                        'J' | 'K' | 'L' => {
                            /* text deletion */
                            self.delete_text(&vt100_code)?
                        }
                        'h' | 'l' => {
                            /* Loads of control options */
                            match vt100_code.as_str() {
                                // Bracketed paste is safe to just send direct to STDOUT. We
                                // won't be running more than one active program at a time, so
                                // it shouldn't matter.
                                "\x1b[?2004h" |  /* Bracketed paste mode ON */
                                "\x1b[?2004l" |  /* Bracketed paste mode OFF */
                                "\x1b[?25l"   |  /* hide cursor */
                                "\x1b[?34h"      /* underline cursor */
                                => {
                                    // All of these can be managed by the
                                    // top level terminal emulator...
                                    print!("{}", vt100_code);
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
                        'r' => { /* Set top and bottom lines of window. Ignored*/ }
                        'n' => { /* Terminal queries */
                            match vt100_code.as_str() {
                                "\x1b[6n" => {
                                    // Query the (virtual) cursor pos
                                    let response = format!("\x1b[{};{}R", self.cursor.y, self.cursor.x);
                                    // TODO: Send this as input
                                }
                                _ => {
                                    info!("Unhandled query!");
                                }
                            }
                        }
                        _ => {
                            /* Just print these directly... I guess */
                            info!("Unknown CSI {:?}", vt100_code);
                            print!("{}", vt100_code);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn write(&self, target: &mut dyn Write) -> anyhow::Result<()> {
        let mut line_idx = 0;

        self.lines.iter().for_each(|line| {
            let ps = self.print_state.clone();
            line.write(self.x, self.y + line_idx, self.width,ps, target).unwrap();
            line_idx +=1;
        });

        // put cursor where it belongs
        info!("After printing, cursor is at {}x{}y", self.cursor.x, self.cursor.y);
        write!(target, "\x1b[{};{}H", self.cursor.y + self.y - 1, self.cursor.x + self.x - 1)?;

        Ok(())
    }

    fn set_cursor_horz(&mut self, col: u16) {
        self.cursor.set_x(max(1, min(col, self.width - 1)));
    }

    fn set_cursor_vert(&mut self, row: u16) {
        self.cursor.set_y(max(1, min(row, self.height - 1)));
    }

    fn delete_text(&mut self, vt100_code: &str) -> anyhow::Result<()> {
        let last_char = vt100_code.chars().last().unwrap();
        match last_char {
            'L' => {
                /* Erase all characters before me, but don't truncate */
                let line = self.lines.get_mut(self.cursor.y as usize).unwrap();
                line.clear_to(self.cursor.x as usize);
            }
            'K' => {
                match Pane::deletion_type(vt100_code) {
                    None => { /*Delete to end of line*/
                        let line = self.lines.get_mut(self.cursor.y as usize).unwrap();
                        line.clear_after(self.cursor.x as usize);
                    }
                    Some(1) => { /* Delete to start of line */
                        let line = self.lines.get_mut(self.cursor.y as usize).unwrap();
                        line.delete_to(self.cursor.x as usize);
                    }
                    Some(2) => { /* Delete entire line*/
                        let line = self.lines.get_mut(self.cursor.y as usize).unwrap();
                        line.clear();
                    }
                    Some(i) => { /*Invalid*/
                        error!("Unknown 'line delete' type '{}'. Ignoring!", i)
                    }
                }
            }
            'J' => {
                match Pane::deletion_type(vt100_code) {
                    None => { /*Delete to end of screen*/
                        // Clear the current line
                        let line = self.lines.get_mut(self.cursor.y as usize).unwrap();
                        line.clear_after(self.cursor.x as usize);

                        //... and then the remainder of the screen
                        for line_idx in (self.cursor.y + 1)..self.height {
                            let line = self.lines.get_mut(line_idx as usize).unwrap();
                            line.clear();
                        }
                    }
                    Some(1) => { /* Delete to start of screen */
                        // Clear the current line
                        let line = self.lines.get_mut(self.cursor.y as usize).unwrap();
                        line.clear_to(self.cursor.x as usize);

                        //... and then the top of the screen on down
                        for line_idx in 0..self.cursor.y {
                            let line = self.lines.get_mut(line_idx as usize).unwrap();
                            line.clear();
                        }
                    }
                    Some(2) => { /* Clear screen */
                        for line_idx in 0..self.height {
                            let line = self.lines.get_mut(line_idx as usize).unwrap();
                            line.clear();
                        }
                    }
                    Some(i) => { /*Invalid*/
                        error!("Unknown 'screen delete' type '{}'. Ignoring!", i)
                    }
                }
            }
            _ => { /* Not a text deletion */ }
        }
        Ok(())
    }

    fn move_cursor(&mut self, vt100_code: &str) -> anyhow::Result<()> {
        let last_char = vt100_code.chars().last().unwrap();
        match last_char {
            'H' | 'f' => {
                let home_regex = Regex::new("\x1b\\[(\\d*);?(\\d*).")?;
                let captures = home_regex.captures(vt100_code).unwrap();

                let row = match captures.get(1) {
                    None => { 0 }
                    Some(m) => { m.as_str().to_owned().parse::<u16>().unwrap_or(0) }
                };
                let col = match captures.get(2) {
                    None => { 0 }
                    Some(m) => { m.as_str().to_owned().parse::<u16>().unwrap_or(0) }
                };

                self.set_cursor_horz(col);
                self.set_cursor_vert(row);
            }

            'A' => {
                let up = Pane::cursor_move_amount(vt100_code)?;
                self.set_cursor_vert(self.cursor.y - up)
            }
            'B' => {
                let down = Pane::cursor_move_amount(vt100_code)?;
                self.set_cursor_vert(self.cursor.y + down)
            }
            'C' => {
                let right = Pane::cursor_move_amount(vt100_code)?;
                self.set_cursor_horz(self.cursor.x + right)
            }
            'D' => {
                let left = Pane::cursor_move_amount(vt100_code)?;
                self.set_cursor_horz(self.cursor.x - left)
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
        let cur_move_regex = Regex::new("\x1b\\[(\\d*).")?;
        let captures = cur_move_regex.captures(vt100_code).unwrap();
        let out = match captures.get(1) {
            None => { 1 }
            Some(m) => { m.as_str().to_owned().parse::<u16>().unwrap_or(1) }
        };

        Ok(out)
    }

    fn deletion_type(vt100_code: &str) -> Option<u16> {
        let cur_move_regex = Regex::new("\x1b\\[(\\d*).").unwrap();
        let captures = cur_move_regex.captures(vt100_code).unwrap();
        match captures.get(1) {
            None => { None }
            Some(m) => { if m.as_str().is_empty() { None} else { Some(m.as_str().to_owned().parse::<u16>().unwrap()) }}
        }
    }

    // A Handle for testing
    fn plaintext(&self) -> String {
        self.lines.iter().map(|l| l.to_str(&self.print_state).to_owned()).collect::<Vec<String>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_displays_blank_space_on_creation() {
        let pane = Pane::new("p1", 1, 1, 10, 20);
        assert_eq!("\n\n\n\n\n\n\n\n\n", pane.plaintext());
    }

    #[test]
    fn it_displays_pushed_text() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("a line of text").unwrap();
        assert_eq!("a line of text\n\n\n\n\n\n\n\n\n", pane.plaintext());
    }

    #[test]
    fn it_moves_the_cursor_horizontally_after_writing() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("a line of text").unwrap();
        assert_eq!(1, pane.cursor.y);
        assert_eq!(15, pane.cursor.x);
    }

    #[test]
    fn it_moves_the_cursor_vertically_after_newline() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("two lines\nof text").unwrap();
        assert_eq!(2, pane.cursor.y);
        assert_eq!(8, pane.cursor.x);
    }

    /***
    Cursor movement tests
     */
    #[test]
    fn it_moves_the_cursor_using_vt100_codes() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("\x1b[5;7H").unwrap(); // Move to 5, 7

        assert_eq!(5, pane.cursor.y);
        assert_eq!(7, pane.cursor.x);
    }

    #[test]
    fn it_moves_the_cursor_up_using_vt100_codes() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("\x1b[5;7H").unwrap(); // Move to 5, 7
        pane.push("\x1b[2A").unwrap();
        pane.push("\x1b[A").unwrap();
        assert_eq!(2, pane.cursor.y);
        assert_eq!(7, pane.cursor.x);
    }

    #[test]
    fn it_moves_the_cursor_down_using_vt100_codes() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("\x1b[5;7H").unwrap(); // Move to 5, 7
        pane.push("\x1b[2B").unwrap();
        pane.push("\x1b[B").unwrap();
        assert_eq!(8, pane.cursor.y);
        assert_eq!(7, pane.cursor.x);
    }

    #[test]
    fn it_moves_the_cursor_right_using_vt100_codes() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("\x1b[5;7H").unwrap(); // Move to 5, 7
        pane.push("\x1b[2C").unwrap();
        pane.push("\x1b[C").unwrap();
        assert_eq!(5, pane.cursor.y);
        assert_eq!(10, pane.cursor.x);
    }

    #[test]
    fn it_moves_the_cursor_left_using_vt100_codes() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("\x1b[5;7H").unwrap(); // Move to 5, 7
        pane.push("\x1b[2D").unwrap();
        pane.push("\x1b[D").unwrap();
        assert_eq!(5, pane.cursor.y);
        assert_eq!(4, pane.cursor.x);
    }

    #[test]
    fn it_moves_the_cursor_and_still_prints_using_vt100_codes() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);

        // Initial state, let's have a box
        //   AAAAA
        //   BBBBB
        //   CCCCC
        //
        // Then use cursor movements to set the top-left and top right to X, the center to O
        // and the bottom to alternate COCOC

        pane.push("AAAAA\nBBBBB\nCCCCC").unwrap();

        pane.push("\x1b[H").unwrap(); // Home
        pane.push("X").unwrap(); // X in top left
        pane.push("\x1b[1;5H").unwrap();
        pane.push("X").unwrap(); // X in top Right

        // Should have XAAAX in the top row now and cursor is at 1,6
        // Move down and left
        pane.push("\x1b[B").unwrap(); // down one
        pane.push("\x1b[3D").unwrap(); // left 3
        pane.push("O").unwrap();

        // Second row should now be BBOBB and cursor is at 1,4
        // jump to the left and down one
        pane.push("\x1b[3;1f").unwrap(); // row 3, col 1
        pane.push("\x1b[1C").unwrap(); // right one
        pane.push("_").unwrap();
        pane.push("\x1b[C").unwrap(); // right one
        pane.push("_").unwrap();

        assert_eq!("XAAAX\nBBOBB\nC_C_C\n\n\n\n\n\n\n", pane.plaintext());
    }


    /***
    PrintState Tests
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
