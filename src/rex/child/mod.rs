/***
* Child Process wrapper
* Keeps track of all the things we need for trafficing I/O between processes
***/
mod child_process;

use crossbeam_channel::{Receiver, Sender};
use portable_pty::Child;
use crate::rex::ProcOutput;

pub struct ChildProcess {
    command: String,
    path: String,
    input_receiver: Receiver<String>,
    input_sender: Sender<String>,
    pub output_sender: Sender<ProcOutput>,
    size: (u16,u16),
    process: Option<Box<dyn Child + Send>>
}