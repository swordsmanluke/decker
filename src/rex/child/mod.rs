/***
* Child Process wrapper
* Keeps track of all the things we need for trafficing I/O between processes
***/
mod child_process;

use std::sync::mpsc::{Receiver, Sender};
use portable_pty::Child;

pub struct ChildProcess {
    command: String,
    path: String,
    input_receiver: Receiver<String>,
    input_sender: Sender<String>,
    pub output_sender: Sender<String>,
    pub status_sender: Sender<String>,
    size: (u16,u16),
    process: Option<Box<dyn Child + Send>>
}