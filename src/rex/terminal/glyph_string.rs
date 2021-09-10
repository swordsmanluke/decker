use lazy_static::lazy_static;
use regex::{Regex};
use std::cmp::{max};
use crate::rex::terminal::pane::PrintStyle;
use std::io::Write;
use log::info;

pub struct GlyphString {
    glyphs: Vec<Glyph>,
}

#[derive(Copy, Clone, Debug)]
pub struct Glyph {
    c: char,
    state: PrintStyle,
}

impl Glyph {
    pub fn new(c: char, state: PrintStyle) -> Self {
        Glyph { c, state }
    }
}

impl Default for Glyph {
    fn default() -> Self {
        Glyph {
            c: ' ',
            state: PrintStyle::default(),
        }
    }
}

lazy_static! {
    static ref VT100_REGEX: Regex = Regex::new(r"((\u001b\[|\u009b)[\u0030-\u003f]*[\u0020-\u002f]*[\u0040-\u007e])+").unwrap();
}

impl GlyphString {
    pub fn new() -> GlyphString {
        GlyphString {
            glyphs: Vec::new()
        }
    }

    pub fn set(&mut self, index: usize, g: Glyph) {
        let extra_chars_reqd = max(0, index as i32 - (self.glyphs.len() as i32 - 1));
        for _ in 0..extra_chars_reqd {
            let state = self.glyphs.last().unwrap_or(&Glyph::default()).state;
            self.glyphs.push(Glyph::new(' ', state));
        }

        self.glyphs[index] = g;
    }

    pub fn push(&mut self, s: &str, style: &PrintStyle) {
        let mut i = self.glyphs.len();
        for c in s.chars() {
            self.set(i, Glyph::new(c, style.clone()));
            i += 1;
        }
    }

    pub fn write(&self, x_offset: u16, y_offset: u16, cur_style: PrintStyle, target: &mut dyn Write) -> anyhow::Result<()> {
        // TODO: Determine if we're dirty before deciding whether to print ourselves!

        // goto the offset for our line
        let mut output = String::new();
        output.push_str(&format!("\x1b[{};{}H", y_offset, x_offset));

        let mut cur_style = cur_style.clone(); // No mutating args!
        for g in &self.glyphs {
            // Make sure to keep the correct style for each glyph
            let diff = cur_style.diff_str(&g.state);

            if diff.len() > 0 {
                cur_style = g.state;
                output.push_str(&diff);
            }

            output.push(g.c);
        }

        write!(target, "{}", output);

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    pub fn slice(&self, from: usize, to: usize) -> String {
        self.glyphs[from..to].iter().map(|g| g.c.to_string()).collect::<Vec<String>>().join(" ")
    }

    pub fn plaintext(&self) -> String {
        self.glyphs.iter().map(|g| g.c.to_string()).collect::<Vec<String>>().join(" ")
    }

    pub fn to_str(&self, current_state: &PrintStyle) -> String {
        let mut current_state = current_state;
        let mut s = String::new();
        for g in &self.glyphs {
            if g.state != *current_state {
                s += &g.state.to_str();
            }
            s.push(g.c);
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_writes_lines_at_offset() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        let mut output = Vec::new();
        g.write(1, 3, ps, &mut output);

        assert_eq!(output, b"\x1b[3;1Ha line of text");
    }

    #[test]
    fn it_respects_glyph_styles() {
        let mut g = GlyphString::new();
        let mut ps = PrintStyle::default();
        ps.apply_vt100("\x1b[32m");

        g.push("a line", &ps);

        ps.apply_vt100("\x1b[37m");

        g.push(" of text", &ps);

        let mut output = Vec::new();
        g.write(1, 3, ps, &mut output);

        assert_eq!(std::str::from_utf8(&output).unwrap(), "\x1b[3;1H\x1b[32ma line\x1b[37m of text");
    }
}