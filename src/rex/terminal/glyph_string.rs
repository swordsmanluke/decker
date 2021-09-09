use lazy_static::lazy_static;
use regex::{Regex};
use std::cmp::{max};
use crate::rex::terminal::pane::PrintState;

pub struct GlyphString {
    glyphs: Vec<Glyph>
}

#[derive(Copy, Clone)]
pub struct Glyph {
    c: char,
    state: PrintState
}

impl Glyph {
    pub fn new(c: char, state: PrintState) -> Self {
        Glyph { c, state }
    }
}

impl Default for Glyph {
    fn default() -> Self {
        Glyph {
            c: ' ',
            state: PrintState::default()
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
        println!("{} extra chars required to insert at {}", extra_chars_reqd, index);
        for _ in 0..extra_chars_reqd {
            let state = self.glyphs.last().unwrap_or(&Glyph::default()).state;
            self.glyphs.push(Glyph::new(' ', state));
        }

        self.glyphs[index] = g;
    }

    // fn build_index_map(s: &str) -> Vec<usize> {
    //     let length = s.len();
    //     let mut out_vec: Vec<usize> = Vec::new();
    //
    //     if s.is_empty() { return out_vec; }
    //
    //     // Merge neighboring VT100s into single ranges, then use the start/end
    //     // to add indices to out_vec
    //     let mut vt100s = VT100String::find_vt100s(s);
    //
    //     let mut ranges: Vec<Range<usize>> = Vec::new();
    //     let mut cur_range = 0..length-1;
    //
    //     // The inverse ranges of our vt100s are our plain text sections.
    //     while let Some(nextVT) = vt100s.pop() {
    //         println!("VT100s: {:?}", nextVT.as_str());
    //         cur_range.end = min(cur_range.end, nextVT.start());
    //         ranges.push(cur_range.clone());
    //         cur_range = nextVT.end()+1..length-1;
    //     }
    //
    //     ranges.push(cur_range.clone());
    //
    //     println!("Ranges!\n{:?}", ranges);
    //
    //     for range in ranges {
    //         for i in range.start..=range.end {
    //             out_vec.push(i);
    //         }
    //     }
    //
    //     out_vec
    // }

    pub fn len(&self) -> usize {
        self.plaintext().len()
    }

    pub fn slice(&self, from: usize, to: usize) -> String {
        self.glyphs[from..to].iter().map(|g| g.c.to_string()).collect::<Vec<String>>().join(" ")
    }

    pub fn plaintext(&self) -> String {
        self.glyphs.iter().map(|g| g.c.to_string()).collect::<Vec<String>>().join(" ")
    }

    pub fn to_str(&self, current_state: &PrintState) -> String {
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


}