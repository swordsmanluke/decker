use std::ops::{Deref, Range};
use lazy_static::lazy_static;
use regex::{Regex, Match};
use std::cmp::{min, max};
use std::borrow::Cow;

pub struct VT100String {
    plain_str: String,
    index_map: Vec<usize>
}

impl Deref for VT100String {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        return &self.plain_str;
    }
}
lazy_static! {
    static ref VT100_REGEX: Regex = Regex::new(r"((\u001b\[|\u009b)[\u0030-\u003f]*[\u0020-\u002f]*[\u0040-\u007e])+").unwrap();
}

impl VT100String {

    pub fn new(s: &str) -> VT100String {
        VT100String {
            plain_str: String::from(s),
            index_map: VT100String::build_index_map(s)
        }
    }

    pub fn set(&mut self, index: usize, c: char) {
        let extra_chars_reqd = max(0, index as i32 - (self.index_map.len() as i32 - 1));
        println!("{} extra chars required to insert at {}", extra_chars_reqd, index);
        for _ in 0..extra_chars_reqd {
            self.plain_str.push(' ');
        }

        if extra_chars_reqd > 0 {
            self.index_map = VT100String::build_index_map(&self.plain_str);
        }

        let real_idx = self.index_map[index];
        self.plain_str.replace_range(real_idx..real_idx, &c.to_string());
    }

    fn build_index_map(s: &str) -> Vec<usize> {
        let length = s.len();
        let mut out_vec: Vec<usize> = Vec::new();

        if s.is_empty() { return out_vec; }

        // Merge neighboring VT100s into single ranges, then use the start/end
        // to add indices to out_vec
        let mut vt100s = VT100String::find_vt100s(s);

        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut cur_range = 0..length-1;

        // The inverse ranges of our vt100s are our plain text sections.
        while let Some(nextVT) = vt100s.pop() {
            println!("VT100s: {:?}", nextVT.as_str());
            cur_range.end = min(cur_range.end, nextVT.start());
            ranges.push(cur_range.clone());
            cur_range = nextVT.end()+1..length-1;
        }

        ranges.push(cur_range.clone());

        println!("Ranges!\n{:?}", ranges);

        for range in ranges {
            for i in range.start..=range.end {
                out_vec.push(i);
            }
        }

        out_vec
    }

    pub fn len(&self) -> usize {
        self.plaintext().len()
    }

    pub fn slice(&self, from: usize, to: usize) -> &str {
        if to == self.len() {
            let true_start = self.index_map[from];
            &self.plain_str[true_start..]
        } else {
            let true_start = self.index_map[from];
            let true_end = self.index_map[to];
            &self.plain_str[true_start..true_end]
        }
    }

    pub fn plaintext(&self) -> Cow<str> {
        VT100_REGEX.replace_all(self.plain_str.as_str(), "")
    }

    pub fn vt100codes(&self) -> Vec<&str> {
        VT100String::find_vt100s(&self.plain_str).iter().map(|c| c.as_str()).collect()
    }

    fn find_vt100s(s: &str) -> Vec<Match> {
        VT100_REGEX.find_iter(s).collect()
    }

    fn esc_aware_slice(&self, n: usize) -> (usize, String) {
        let s = self.plain_str.as_str();
        if n >= s.len() { return (s.len(), format!("{:width$}", s, width = s.len())) }

        // Early return - no VT100 to skip!
        let vt100s = VT100String::find_vt100s(&self.plain_str);
        if vt100s.last().is_none() { return (n, s[0..n].to_string()); }

        let mut captured_chars = 0;
        let mut end = 0;

        for c in vt100s.iter() {
            if (captured_chars + 1) < n {  // cc+1 to avoid subtraction with overflow
                let next_block_of_text_size = c.start() - end;
                let next_incr = if captured_chars + next_block_of_text_size >= n {
                    (captured_chars + next_block_of_text_size) - n
                } else {
                    next_block_of_text_size
                };

                captured_chars += next_incr;
                end = c.end();
            }
        };

        if captured_chars < n {
            end += n - captured_chars; // grab any remaining characters we need
        }

        let slice_end = min(s.len(), end);
        let sliced_str = s[0..slice_end].to_string();

        (end, sliced_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_does_nothing_to_non_vt100_plain_text() {
        let plaintext = VT100String::new("TEST");
        assert_eq!(plaintext.index_map, vec![0, 1, 2, 3])
    }

    #[test]
    fn it_keeps_preceding_vt100_values() {
        let plaintext = VT100String::new("\x1b[33mTEST");
        assert_eq!(plaintext.index_map, vec![0, 6, 7, 8])
    }

    #[test]
    fn it_creates_indices_for_each_char() {
        let plaintext = VT100String::new("\x1b[33mTEST");
        assert_eq!(plaintext[plaintext.index_map[0]..(plaintext.index_map[1])], *"\x1b[33mT");
        assert_eq!(plaintext[plaintext.index_map[1]..(plaintext.index_map[2])], *"E");
        assert_eq!(plaintext[plaintext.index_map[2]..(plaintext.index_map[3])], *"S");
        assert_eq!(plaintext[plaintext.index_map[3]..], *"T");
    }

    #[test]
    fn it_indices_mid_vt100_as_expected() {
        let plaintext = VT100String::new("TE\x1b[33mST");
        assert_eq!(plaintext.index_map, vec![0, 1, 2, 8])
    }

    #[test]
    fn it_merges_multiple_vt100_sequences() {
        let plaintext = VT100String::new("TE\x1b[33m\x1b[HST");
        assert_eq!(plaintext.index_map, vec![0, 1, 2, 11])
    }

    #[test]
    fn it_slices_taking_vt100_into_account() {
        let vt100text = VT100String::new("TE\x1b[33m\x1b[HST");
        assert_eq!(vt100text.slice(2, vt100text.len()), "\x1b[33m\x1b[HST");
        assert_eq!(vt100text.slice(3, vt100text.len()), "T");
    }

    #[test]
    fn it_replaces_chars_in_mid_str() {
        let mut vt100text = VT100String::new("TE\x1b[33m\x1b[HST");
        vt100text.set(2, 's');
        assert_eq!(vt100text.plain_str, "TE\x1b[33m\x1b[HsT")
    }
}