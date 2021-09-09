use crate::rex::TaskId;
use regex::Regex;
use crate::rex::terminal::{Vt100Translator, TerminalLocation, View, TerminalSize};
use std::io::Write;
use log::info;
use crate::rex::terminal::internal::StreamState;

impl Vt100Translator {
    pub fn new() -> Vt100Translator {
        Vt100Translator {
            streams: Default::default()
        }
    }

    pub fn register(&mut self, task_id: TaskId, view: View) {
        let ss = StreamState::new();
        self.streams.insert(task_id, ss);
    }

    pub fn write(&mut self, target: &mut dyn Write) {
        for (task_id, stream_state) in self.streams.iter_mut() {
            info!("Writing output for {}", task_id);
            // write!(target, "{}", stream_state.consume().clone());
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