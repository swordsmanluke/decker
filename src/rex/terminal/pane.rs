use crate::rex::terminal::vt100_string::{VT100String, Glyph};
use regex::{Regex, Captures};

pub struct Pane {
    id: String,
    // Location and Dimensions
    x: u16,
    y: u16,
    height: u16,
    width: u16,

    // Cached lines
    lines: Vec<VT100String>,

    // virtual cursor location
    cur_x: usize,
    cur_y: usize,

    // current print state
    print_state: PrintState,
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

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct PrintState {
    pub foreground: Color,
    pub background: Color,
    pub underline: bool,
    pub blink: bool,
    pub bold: bool,
}

impl PrintState {
    pub fn to_str(&self) -> String {
        // TODO: Assemble a set of numbers to push together into a single command.

        // Add colors first
        let fg_base = if self.bold { 90 } else { 30 };
        let bg_base = if self.bold { 100 } else { 40 };

        let fg_str = match self.foreground {
            Color::TWOFIFTYSIX(num) => { format!("\x1b[38;5;{}m", num) }
            Color::RGB(r, g, b) => { format!("\x1b[38;2;{};{};{}m", r, g, b) }
            color => { format!("\x1b[{}m", fg_base + color.to_offset()) }
        };

        let bg_str = match self.background {
            Color::TWOFIFTYSIX(num) => { format!("\x1b[38;5;{}m", num) }
            Color::RGB(r, g, b) => { format!("\x1b[38;2;{};{};{}m", r, g, b) }
            color => { format!("\x1b[{}m", bg_base + color.to_offset()) }
        };

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

        let mut out = String::from(fg_str);
        out.push_str(&bg_str);
        out.push_str(&blink);
        out.push_str(&underlined);

        out
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
            _ => { panic!("{} is not a valid SGR extended color argument!", args.first().unwrap()) }
        }
    }
}

impl Default for PrintState {
    fn default() -> Self {
        PrintState {
            foreground: Color::White,
            background: Color::Black,
            underline: false,
            blink: false,
            bold: false,
        }
    }
}

impl PrintState {
    pub fn apply_vt100(&mut self, s: &str) -> anyhow::Result<()> {
        let parm_rx = Regex::new("\x1b\\[([0-9;]+)m").unwrap();
        match parm_rx.captures(s) {
            None => { panic!("{} does not look like an SGR sequence!", s) }
            Some(captures) => {
                let mut int_parts: Vec<u8> = captures.get(1).unwrap().as_str().
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
                    4 => { self.underline = true; }
                    5 => { self.blink = true; }
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
        let mut lines = (0..height).map(|_| VT100String::new()).collect::<Vec<VT100String>>();

        Pane {
            id: String::from(id),
            x,
            y,
            height,
            width,
            lines,
            cur_x: 0,
            cur_y: 0,
            print_state: PrintState::default(),
        }
    }

    pub fn push(&mut self, s: &str) -> anyhow::Result<()> {
        let mut vert_line = self.cur_y as usize;

        for c in s.chars() {
            match c {
                '\n' => {
                    self.cur_x = 0;
                    self.cur_y += 1;
                    if self.cur_y >= self.height as usize {
                        self.cur_y = (self.height - 1) as usize;
                        self.lines.remove(0); // discard the topmost line of output
                        self.lines.push(VT100String::new());
                    }
                }
                _ => {
                    let line = self.lines.get_mut(vert_line).unwrap();
                    line.set(self.cur_x, Glyph::new(c, self.print_state));
                    self.cur_x += 1;
                }
            }
        }

        Ok(())
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
        assert_eq!(0, pane.cur_y);
        assert_eq!(14, pane.cur_x);
    }

    #[test]
    fn it_moves_the_cursor_vertically_after_newline() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("two lines\nof text").unwrap();
        assert_eq!(1, pane.cur_y);
        assert_eq!(7, pane.cur_x);
    }

    /***
    PrintState Tests
     */
    #[test]
    fn it_converts_simple_vt100_sgr_to_print_state() {
        let code = "\x1b[33m";
        let mut ps = PrintState::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::Yellow);
    }

    #[test]
    fn it_converts_bold_vt100_sgr_to_print_state() {
        let code = "\x1b[93m";
        let mut ps = PrintState::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::Yellow);
        assert_eq!(ps.bold, true);
    }

    #[test]
    fn it_converts_background_vt100_sgr_to_print_state() {
        let code = "\x1b[43m";
        let mut ps = PrintState::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.background, Color::Yellow);
    }

    #[test]
    fn it_converts_256_color_vt100_sgr_to_print_state() {
        let code = "\x1b[38;5;128m";
        let mut ps = PrintState::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::TWOFIFTYSIX(128));
    }

    #[test]
    fn it_converts_rgb_color_vt100_sgr_to_print_state() {
        let code = "\x1b[38;2;128;42;255m";
        let mut ps = PrintState::default();
        ps.apply_vt100(code).unwrap();
        assert_eq!(ps.foreground, Color::RGB(128, 42, 255));
    }

    #[test]
    fn it_converts_state_back_into_vt100() {
        let fg_code = "\x1b[38;2;128;42;255m";
        let bg_code = "\x1b[47m";
        let mut ps = PrintState::default();
        ps.apply_vt100(fg_code).unwrap();
        ps.apply_vt100(bg_code).unwrap();

        assert_eq!(ps.to_str(), fg_code.to_owned()+bg_code);
    }
}

