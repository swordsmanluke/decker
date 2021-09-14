use lazy_static::lazy_static;
use regex::{Regex};
use std::cmp::{max, min};
use crate::rex::terminal::pane::PrintStyle;
use std::io::Write;
use log::{debug, info};
use std::fmt::{Debug, Formatter};

#[derive(Clone)]
pub struct GlyphString {
    glyphs: Vec<Glyph>,
}

#[derive(Copy, Clone, Debug)]
struct Glyph {
    pub c: char,
    pub state: PrintStyle,
    pub dirty: bool,
    fill: bool
}

impl Glyph {
    pub fn new(c: char, state: PrintStyle) -> Self {
        let fill = (0x20_u8..0x7E_u8).contains(&(c as u8));
        Glyph { c, state, fill, dirty: true }
    }

    pub fn is_fill(&self) -> bool {
        self.fill
    }
}

impl Default for Glyph {
    fn default() -> Self {
        Glyph::new(' ', PrintStyle::default())
    }
}

lazy_static! {
    static ref VT100_REGEX: Regex = Regex::new(r"((\u001b\[|\u009b)[\u0030-\u003f]*[\u0020-\u002f]*[\u0040-\u007e])+").unwrap();
}

impl Debug for GlyphString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.plaintext().as_str())
    }
}

impl GlyphString {
    pub fn new() -> GlyphString {
        GlyphString {
            glyphs: Vec::new()
        }
    }

    pub fn dirty(&self) -> bool {
        self.glyphs.iter().any(|g| g.dirty)
    }

    pub fn empty(&self) -> bool {
        // Short lines are probably blanks
        // otherwise, if all the glyphs are spaces, we're empty.
        self.glyphs.len() < 2 ||
            self.glyphs.iter().all(|g| g.c == ' ')
    }

    pub fn make_dirty(&mut self) {
        match self.glyphs.get_mut(0){
            None => {}
            Some(g) => { g.dirty = true; }
        }
    }

    pub fn set(&mut self, index: usize, c: char, style: &PrintStyle) {
        let extra_chars_reqd = max(0, index as i32 - (self.glyphs.len() as i32 - 1));
        let default_style = self.glyphs.last().unwrap_or(&Glyph::default()).state;
        for _ in 0..extra_chars_reqd {
            self.glyphs.push(Glyph::new(' ', default_style));
        }

        self.glyphs[index] = Glyph::new(c, style.clone());
    }

    pub fn push(&mut self, s: &str, style: &PrintStyle) {
        let mut i = self.glyphs.len();
        for c in s.chars() {
            self.set(i, c, style);
            i += 1;
        }
    }

    pub fn clear_to(&mut self, idx: usize) {
        for i in 0..idx {
            match self.glyphs.get_mut(i) {
                None => {}
                Some(g) => {g.c = ' '}
            }
        }
    }

    fn visible_idx(&self, idx: usize) -> usize {
        let mut i = 0;
        let mut j = 0;
        loop {
            let visible = match self.glyphs.get(j) { None => true, Some(g) => g.is_fill() };
            if visible { i += 1; }
            println!("j: {}, i: {} Vis: {}", j, i, visible);
            if i > idx { break; }
            j += 1;
        }

        println!("Mapping {}->{}", idx, j);
        j
    }

    fn get(&mut self, idx: usize) -> Option<&mut Glyph> {
        let idx = self.visible_idx(idx);
        self.glyphs.get_mut(idx)
    }

    pub fn clear_at(&mut self, idx: usize) {
        self.glyphs.get_mut(idx).unwrap().c = ' '
    }

    pub fn delete_to(&mut self, idx: usize) {
        let start = min(self.len(), idx);
        self.glyphs = self.glyphs[start..self.len()].to_owned();
    }

    pub fn delete_at(&mut self, idx: usize) {
        if idx < self.len() {
            self.glyphs.remove(idx);
        }
    }

    pub fn clear_after(&mut self, idx: usize) {
        for i in idx..self.len() {
            self.clear_at(i);
        }
    }

    pub fn clear(&mut self) {
        self.glyphs.clear();
    }

    pub fn write(&mut self, x_offset: u16, y_offset: u16, width: u16, style: PrintStyle, target: &mut dyn Write) -> anyhow::Result<()> {
        // goto the offset for our line
        let mut output = String::new();
        output.push_str(&format!("\x1b[{};{}H", y_offset, x_offset));

        let mut cur_style = style.clone(); // No mutating args!
        let visible_width = min(self.len(), width as usize);

        self.glyphs.iter_mut().take(visible_width).for_each(|g| {
            g.dirty = false; // We've printed you now!

            // Make sure to keep the correct style for each glyph
            let diff = cur_style.diff_str(&g.state);

            if diff.len() > 0 {
                debug!("Updating style. FG/BG: {}/{} Str: {}", g.state.foreground, g.state.background, g.c);
                cur_style = g.state;
                output.push_str(&diff);
            }

            output.push(g.c);
        });

        // reset to the og style
        let diff = cur_style.diff_str(&style);
        if diff.len() > 0 {
            output.push_str(&diff);
        }

        let mut pad_width = width as usize;
        if self.len() < pad_width {
            // Have to pad the final output string length, 'cause the writer doesn't handle
            // VT100 sequences.
            pad_width = output.len() + (pad_width - self.len());
        }

        write!(target, "{0: <1$}", output, pad_width)?;

        Ok(())
    }

    pub fn len(&self) -> usize {
        // TODO: Cache
        self.glyphs.iter().filter(|g| g.is_fill()).count()
    }

    pub fn raw_len(&self) -> usize {
        self.glyphs.len()
    }

    pub fn plaintext(&self) -> String {
        self.glyphs.iter().map(|g| g.c.to_string()).collect::<Vec<String>>().join("")
    }

    pub fn to_str(&self, current_state: &PrintStyle) -> String {
        let mut current_state = *current_state;
        let mut s = String::new();
        for g in &self.glyphs {
            if g.state != current_state {
                s += &g.state.to_str();
                current_state = g.state.clone();
            }
            s.push(g.c);
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /***
    GlyphString tests
     */

    #[test]
    fn it_respects_non_printable_chars_when_reporting_length() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("\u{f}pi\u{f}:~/\u{f} $", &ps);
        println!("{:?}", g.plaintext());
        assert_eq!(g.raw_len(), 10);
        assert_eq!(g.len(), 6)
    }

    #[test]
    fn it_calculates_offsets_for_non_visible_chars() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("\u{f}pi\u{f}:~/\u{f} $", &ps);

        let idxes = (0..g.len()).map(|i| g.visible_idx(i)).collect::<Vec<_>>();

        assert_eq!(idxes, vec![1,2,4,6,8,9]);
    }

    #[test]
    fn it_respects_non_printable_chars_when_indexing() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("\u{f}pi\u{f}:~/\u{f} $", &ps);

        assert_eq!(g.get(0).unwrap().c, 'p');
        assert_eq!(g.get(1).unwrap().c, 'i');
        assert_eq!(g.get(5).unwrap().c, '$');
    }

    #[test]
    fn it_writes_lines_at_offset() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        let mut output = Vec::new();
        g.write(1, 3, 14, ps, &mut output).unwrap();

        assert_eq!(output, b"\x1b[3;1Ha line of text");
    }

    #[test]
    fn it_right_pads_with_spaces() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        let mut output = Vec::new();
        g.write(1, 3, 15, ps, &mut output).unwrap();

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
        g.write(1, 3, 14, ps, &mut output).unwrap();

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
    fn it_clears_following_chars() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        g.clear_after(6);

        assert_eq!(g.to_str(&ps), "a line        ");
    }

    #[test]
    fn it_clears_all_chars() {
        let mut g = GlyphString::new();
        let ps = PrintStyle::default();

        g.push("a line of text", &ps);

        g.clear();

        assert_eq!(g.to_str(&ps), "");
    }

    /***
    Glyph tests
     */
    #[test]
    fn it_recognizes_non_fill_chars() {
        let g = Glyph::new('\x0F', PrintStyle::default());
        assert_eq!(g.fill, false)
    }

    #[test]
    fn it_recognizes_esc_as_a_non_fill_char() {
        let g = Glyph::new('\x1B', PrintStyle::default());
        assert_eq!(g.fill, false)
    }

    #[test]
    fn it_recognizes_fill_chars() {
        let g = Glyph::new(' ', PrintStyle::default());
        assert_eq!(g.fill, true)
    }

    #[test]
    fn it_recognizes_alhpa_fill_chars() {
        let g = Glyph::new('a', PrintStyle::default());
        assert_eq!(g.fill, true)
    }
}