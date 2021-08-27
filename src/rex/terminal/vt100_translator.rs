use crate::rex::TaskId;
use regex::Regex;
use crate::rex::terminal::vt100_translator::VT100State::{FoundEsc, PlainText};
use crate::rex::terminal::{Vt100Translator, TerminalLocation, View, TerminalSize};
use std::io::Write;
use log::info;

impl Vt100Translator {
    pub fn new() -> Vt100Translator {
        Vt100Translator {
            streams: Default::default()
        }
    }

    pub fn register(&mut self, task_id: TaskId, view: View) {
        let ss = StreamState::new(view.location, view.dimensions);
        self.streams.insert(task_id, ss);
    }

    pub fn write(&mut self, target: &mut dyn Write) {
        for (task_id, stream_state) in self.streams.iter_mut() {
            info!("Writing output for {}", task_id);
            write!(target, "{}", stream_state.consume().clone());
        }
    }

    pub fn push(&mut self, task_id: TaskId, data: &String) {
        match self.streams.get_mut(&task_id) {
            None => {  info!("Received output for unregistered task {}", &task_id); } // Drop data for unknown tasks
            Some(stream_state) => { stream_state.push(data) }
        }
    }
}

impl TerminalLocation {
    pub fn new(top: u16, left: u16) -> TerminalLocation {
        TerminalLocation {
            top,
            left
        }
    }
}

impl TerminalSize {
    pub fn new(height: u16, width: u16) -> TerminalSize {
        TerminalSize {
            height,
            width
        }
    }
}

pub(crate) struct StreamState {
    location: TerminalLocation,
    size: TerminalSize,
    buffer: String,
    vetted_chars: String,
    building_esc_seq: bool,
    build_state: VT100State
}

enum VT100State {
    PlainText,
    FoundEsc
}

impl StreamState {
    pub fn new(location: TerminalLocation, size: TerminalSize) -> StreamState {
        StreamState {
            location,
            size,
            buffer: String::new(),
            vetted_chars: String::new(),
            building_esc_seq: false,
            build_state: PlainText
        }
    }

    pub fn push(&mut self, stdin: &str) {
        for c in stdin.chars() {
            match self.build_state {
                PlainText => {
                    if c == '\x1b' { // start looking for an esc seq
                        self.vet_buffer();
                        self.buffer.push(c);
                        self.build_state = FoundEsc
                    } else {
                        self.vetted_chars.push(c);
                    }
                }
                FoundEsc => {
                    self.buffer.push(c);
                    let not_an_esc_seq = self.buffer.len() == 2 && !self.is_esc_seq();

                    if not_an_esc_seq ||  self.is_esc_seq_complete() {
                        self.vet_buffer();
                        self.build_state = PlainText;
                    }
                }
            }
        }
    }

    fn vet_buffer(&mut self) {
        
        self.vetted_chars.push_str(self.buffer.as_str());
        self.buffer.clear();
    }

    pub fn is_esc_seq(&self) -> bool {
        self.buffer.starts_with("\x1B[")
    }

    fn is_esc_seq_complete(&self) -> bool {
        // TODO: Make this regex static or a constant or something
        let vt100_regex = Regex::new(r"((\x1b\[|\x9b)[\x30-\x3f]*[\x20-\x2f]*[\x40-\x7e])+").unwrap();
        self.is_esc_seq() && vt100_regex.is_match(&self.buffer)
    }

    pub fn is_complete(&self) -> bool {
        // If we have anything vetted, go consume it!
        self.buffer.ends_with('\x1b') || !self.vetted_chars.is_empty()
    }

    pub fn consume(&mut self) -> String {
        if self.buffer.ends_with('\x1b') { self.vet_buffer(); self.build_state = PlainText; }
        let out = self.vetted_chars.clone();
        self.vetted_chars.clear();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn given_a_blank_stream() -> StreamState {
        let l = TerminalLocation::new(1, 1);
        let sz = TerminalSize::new(24, 80);
        StreamState::new(l, sz)
    }

    fn given_a_stream_with_chars(chars: &str) -> StreamState {
        let l = TerminalLocation::new(1, 1);
        let sz = TerminalSize::new(24, 80);
        let mut s = StreamState::new(l, sz);
        s.push(chars);
        s
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
        assert_eq!(s.consume(), String::from("some chars"));
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
        assert_eq!(out, String::from("some chars"))
    }

    #[test]
    fn it_releases_the_esc_when_it_is_alone() {
        let mut s = given_a_stream_with_chars("some chars");
        s.push("\x1b");
        let out = s.consume();
        assert_eq!(out, String::from("some chars\x1b"))
    }
}