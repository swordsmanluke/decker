use crate::rex::terminal::internal::TerminalOutput::{Plaintext, CSI};

mod stream_state;

enum VT100State {
    PlainText,
    FoundEsc,
}

/***
Output is either plaintext or a VT100 command sequence instruction
 */
#[derive(Clone, Debug)]
pub enum TerminalOutput {
    Plaintext(String),
    CSI(String),
}

impl TerminalOutput {
    pub fn to_string(&self) -> String {
        match self {
            Plaintext(s) => { s.clone() }
            CSI(s) => { s.clone() }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Plaintext(s) => { s.len() == 0 }
            CSI(s) => { s.len() == 0 }
        }
    }
}

pub(crate) struct StreamState {
    buffer: String,
    vetted_output: Vec<TerminalOutput>,
    build_state: VT100State,
}