use std::cmp::{max, min};
use std::io::Write;
use log::{debug, info};
use std::fmt::{Debug, Formatter};
use crate::decker::terminal::{PrintStyle, VirtualCoord};

#[derive(Clone)]
pub struct GlyphString {
    pub glyphs: Vec<Glyph>,
    string_rep: String,
    dirty: bool
}

#[derive(Copy, Clone, Debug)]
pub struct Glyph {
    pub c: char,
    pub style: PrintStyle,
    pub dirty: bool,
}

impl Glyph {
    pub fn new(c: char, state: PrintStyle) -> Self {
        Glyph { c, style: state, dirty: true }
    }
}

impl Default for Glyph {
    fn default() -> Self {
        Glyph::new(' ', PrintStyle::default())
    }
}

impl Debug for GlyphString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.plaintext().as_str())
    }
}

impl GlyphString {
    pub fn new() -> GlyphString {
        GlyphString {
            glyphs: Vec::new(),
            string_rep: String::new(),
            dirty: true
        }
    }

    pub fn last_style(&self) -> PrintStyle {
        match self.glyphs.last() {
            None => { PrintStyle::default() }
            Some(g) => { g.style }
        }
    }

    pub fn dirty(&self) -> bool {
        self.dirty || self.glyphs.iter().any(|g| g.dirty)
    }

    pub fn make_dirty(&mut self) {
        self.dirty = true
    }

    pub fn set(&mut self, index: VirtualCoord, c: char, style: &PrintStyle) {
        let extra_chars_reqd = max(0, index as i32 - (self.glyphs.len() as i32 - 1));
        let default_style = self.glyphs.last().unwrap_or(&Glyph::default()).style;
        for _ in 0..extra_chars_reqd {
            self.glyphs.push(Glyph::new(' ', default_style.clone()));
        }

        self.glyphs[index as usize] = Glyph::new(c, style.clone());
        self.make_dirty()
    }

    pub fn push(&mut self, s: &str, style: &PrintStyle) {
        let mut i = self.glyphs.len();
        for c in s.chars() {
            self.set(i as VirtualCoord, c, style);
            i += 1;
        }
    }

    pub fn clear_to(&mut self, idx: usize) {
        for i in 0..idx {
            self.set(i as VirtualCoord, ' ', &PrintStyle::default());
        }
    }

    pub fn clear_at(&mut self, idx: usize) {
        self.set(idx as VirtualCoord, ' ', &PrintStyle::default());
    }

    pub fn delete_to(&mut self, idx: usize) {
        let start = min(self.len(), idx);
        self.glyphs = self.glyphs[start..self.len()].to_owned();
        self.make_dirty()
    }

    pub fn clear_after(&mut self, idx: usize) {
        info!("main: CSI ClearAfter({}). Before: \"{:?}\"", idx, self);
        for i in idx..self.len() {
            self.clear_at(i);
        }
        info!("main: CSI ClearAfter({}). After : \"{:?}\"", idx, self);
    }

    pub fn clear(&mut self) {
        info!("Clearing string: '{}'", self.plaintext());
        self.glyphs.clear();
        self.make_dirty()
    }

    pub fn write(&mut self, x_offset: u16, y_offset: u16, width: u16, style: &PrintStyle, target: &mut dyn Write) -> anyhow::Result<()> {
        // write our line at the appropriate offset, style and size!
        let line_style = style.diff_str(&self.glyphs.first().unwrap_or(&Glyph::default()).style);
        let reset_style = self.glyphs.last().unwrap_or(&Glyph::default()).style.diff_str(style);

        let set_cursor = format!("\x1b[{};{}H", y_offset, x_offset);
        let output = format!("{}{}{}{}",
                                 set_cursor,
                                 line_style,
                                 self.str_with_width(width as usize),
                                 reset_style);

        let pad_width = if self.len() < width as usize {
            // Have to pad using the formatted output string length, 'cause the writer doesn't handle
            // VT100 sequences.
            let extra_padding_reqd = width - self.len() as u16;
            output.len() + extra_padding_reqd as usize
        } else {
            width as usize
        };

        write!(target, "{0: <1$}", output, pad_width)?;
        self.dirty = false;

        Ok(())
    }

    fn str_with_width(&mut self, width: usize) -> String {
        info!("Printing string with width {}", width);

        let mut output = String::new();
        let mut cur_style = self.glyphs.first().unwrap_or(&Glyph::default()).style.clone(); // No mutating args!

        self.glyphs.iter_mut().take(width).for_each(|g| {
            g.dirty = false; // We've printed you now!

            // Make sure to keep the correct style for each glyph
            let diff = cur_style.diff_str(&g.style);

            if diff.len() > 0 {
                debug!("Updating style. FG/BG: {}/{} Str: {}", g.style.foreground, g.style.background, g.c);
                cur_style = g.style;
                output.push_str(&diff);
            }

            output.push(g.c);
        });

        info!("output: {}, glyph len: {}", output, self.glyphs.len());

        output
    }

    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    pub fn plaintext(&self) -> String {
        self.glyphs.iter().map(|g| g.c.to_string()).collect::<Vec<String>>().join("")
    }

    pub fn to_str(&self, current_state: &PrintStyle) -> String {
        let mut current_state = *current_state;
        let mut s = String::new();
        for g in &self.glyphs {
            if g.style != current_state {
                s += &g.style.to_str();
                current_state = g.style.clone();
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
        g.write(1, 3, 14, &ps, &mut output).unwrap();

        assert_eq!(output, b"\x1b[3;1Ha line of text");
    }

    #[test]
    fn it_right_pads_with_spaces() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        let mut output = Vec::new();
        g.write(1, 3, 15, &ps, &mut output).unwrap();

        assert_eq!(output, b"\x1b[3;1Ha line of text ");
    }

    #[test]
    fn it_respects_glyph_styles() {
        let mut g = GlyphString::new();
        let mut ps = PrintStyle::default();
        ps.apply_vt100("\x1b[32m").unwrap();

        g.push("a line", &ps);

        ps.apply_vt100("\x1b[37m").unwrap();

        g.push(" of text", &ps);

        let mut output = Vec::new();
        g.write(1, 3, 14, &ps, &mut output).unwrap();

        assert_eq!(std::str::from_utf8(&output).unwrap(), "\x1b[3;1H\x1b[32ma line\x1b[37m of text");
    }

    #[test]
    fn it_clears_leading_chars() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        g.clear_to(6);

        assert_eq!(g.to_str(&ps), "       of text")
    }

    #[test]
    fn it_deletes_leading_chars() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        g.delete_to(6);

        assert_eq!(g.to_str(&ps), " of text")
    }

    #[test]
    fn it_clears_all_chars() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        g.clear();

        assert_eq!(g.to_str(&ps), "");
    }

    #[test]
    fn it_clears_following_chars() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        g.clear_after(6);

        assert_eq!(g.to_str(&ps), "a line        ");
    }

}