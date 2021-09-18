use crate::rex::terminal::internal::{StreamState, TerminalOutput, VT100};
use crate::rex::terminal::internal::VT100State::{PlainText, FoundEsc};
use crate::rex::terminal::internal::TerminalOutput::{Plaintext, CSI};
use regex::Regex;
use lazy_static::lazy_static;
use std::str::FromStr;

lazy_static! {
    static ref CSI_BEGINNING: Regex = Regex::new(r"\x1b[\[\x9b>=MD]").unwrap();
    static ref VT100_REGEX:  Regex = Regex::new(r"\x1b[\[\x9b>=MD]([0-?]*[ -/]*[@-~>=])").unwrap();
    static ref VT100_SCROLL_REGEX: Regex = Regex::new(r"\x1b[MD]").unwrap();
}

impl StreamState {
    pub fn new() -> StreamState {
        StreamState {
            buffer: String::new(),
            vetted_output: Vec::new(),
            build_state: PlainText,
        }
    }

    pub fn push(&mut self, stdin: &str) {
        for c in stdin.chars() {
            match self.build_state {
                PlainText => {
                    if c == '\x1b' { // start looking for an esc seq
                        self.consume_buffer();
                        self.buffer.push(c);
                        self.build_state = FoundEsc
                    } else {
                        let last_output = self.vetted_output.pop().unwrap_or(Plaintext(String::new()));
                        match last_output {
                            Plaintext(mut plaintext_str) => {
                                plaintext_str.push(c);
                                self.vetted_output.push(Plaintext(plaintext_str));
                            }
                            CSI(csi_str) => {
                                // Whoops - we can't append directly to this one!
                                // Put it back and start a new string
                                self.vetted_output.push(CSI(csi_str));
                                self.vetted_output.push(Plaintext(String::from(c)));
                            }
                        }
                    }
                }

                FoundEsc => {
                    self.buffer.push(c);
                    let not_an_esc_seq = self.buffer.len() == 2 && !self.is_esc_seq();

                    if not_an_esc_seq || self.is_esc_seq_complete() {
                        self.consume_buffer();
                        self.build_state = PlainText;
                    }
                }
            }
        }
    }

    fn consume_buffer(&mut self) {
        let buf_str = self.buffer.clone();

        if self.is_esc_seq_complete() {
            self.vetted_output.push(CSI(VT100::from_str(&buf_str.as_str()).unwrap()));
        } else {
            self.vetted_output.push(Plaintext(buf_str));
        }

        self.buffer.clear();
    }

    pub fn is_esc_seq(&self) -> bool {
        CSI_BEGINNING.is_match(&self.buffer)
    }

    fn is_esc_seq_complete(&self) -> bool {
        self.is_esc_seq() && (
            VT100_REGEX.is_match(&self.buffer) ||
            VT100_SCROLL_REGEX.is_match(&self.buffer))
    }

    pub fn is_complete(&self) -> bool {
        // If we have anything vetted, go consume it!
        let have_vetted_output = self.vetted_output.iter().any(
            |v| match v {
                Plaintext(s) => { !s.is_empty() }
                CSI(_) => { true } // CSIs always have contents
            }
        );
        self.buffer.ends_with('\x1b') || have_vetted_output
    }

    pub fn consume(&mut self) -> Vec<TerminalOutput> {
        if self.buffer.ends_with('\x1b') {
            self.consume_buffer();
            self.build_state = PlainText;
        }
        // reject any empty strings.
        let out = self.vetted_output.iter().filter(|o| !o.is_empty()).cloned().collect();

        self.vetted_output = Vec::new();

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn given_a_blank_stream() -> StreamState {
        StreamState::new()
    }

    fn given_a_stream_with_chars(chars: &str) -> StreamState {
        let mut s = StreamState::new();
        s.push(chars);
        s
    }

    fn as_raw_string(output_vec: &Vec<TerminalOutput>) -> String {
        output_vec.iter().
            map(|c| c.to_string()).
            collect::<Vec<String>>().
            join("")
    }

    #[test]
    fn it_detects_an_esc_seq() {
        let mut s = given_a_blank_stream();
        s.push("\x1b[");

        assert!(s.is_esc_seq());
    }

    #[test]
    fn it_detects_plain_text() {
        let mut s = given_a_blank_stream();
        s.push("hi!");

        assert!(!s.is_esc_seq());
    }

    #[test]
    fn it_does_not_detect_an_esc_seq_when_just_an_esc() {
        let s = given_a_stream_with_chars("\x1b");
        assert!(!s.is_esc_seq());
    }

    #[test]
    fn it_detects_when_esc_seq_is_complete() {
        let mut s = given_a_blank_stream();
        s.push("\x1b[");

        assert!(s.is_esc_seq() && !s.is_complete());

        s.push("33m");
        assert!(s.is_complete());
    }

    #[test]
    fn it_says_normal_text_is_complete() {
        let mut s = given_a_blank_stream();
        s.push("normal");

        assert!(!s.is_esc_seq() && s.is_complete());

        s.push(" text");
        assert!(!s.is_esc_seq() && s.is_complete());
    }

    #[test]
    fn it_clears_data_when_consumed() {
        let mut s = given_a_stream_with_chars("some chars");
        assert!(s.is_complete());
        assert_eq!(as_raw_string(&s.consume()), String::from("some chars"));
        assert!(!s.is_complete())
    }

    #[test]
    fn it_remains_complete_when_an_esc_sequence_comes_in() {
        let mut s = given_a_stream_with_chars("some chars");
        s.push("\x1b[");
        assert!(s.is_complete())
    }

    #[test]
    fn it_buffers_the_esc_sequence_when_an_esc_seq_comes_in() {
        let mut s = given_a_stream_with_chars("some chars");
        s.push("\x1b[");
        let out = s.consume();
        assert_eq!(as_raw_string(&out), String::from("some chars"))
    }

    #[test]
    fn it_releases_the_esc_when_it_is_alone() {
        let mut s = given_a_stream_with_chars("some chars");
        s.push("\x1b");
        let out = s.consume();
        assert_eq!(as_raw_string(&out), String::from("some chars\x1b"))
    }

    #[test]
    fn it_recognizes_scroll_commands() {
        let mut s = given_a_stream_with_chars("\x1bM\x1bD");
        let out = s.consume();
        assert!(out.iter().all(|s| match s {
            CSI(_) => { true }
            _ => { false }
        }), "not all of {:?} are CSIs!", &out);
    }

    #[test]
    fn it_recognizes_unusual_csis() {
        let mut s = given_a_stream_with_chars("\x1b[>\x1b[=\x1b=\x1b>\x1b\\");
        let out = s.consume();
        assert!(out.iter().all(|s| match s {
            CSI(_) => { true }
            _ => { false }
        }), "not all of {:?} are CSIs!", &out);
    }
}