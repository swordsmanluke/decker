use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;


pub(crate) mod child;
mod process_orchestrator;
mod master_control;
pub(crate) mod terminal;

use serde::{Deserialize, Serialize};

pub struct ProcOutput { pub name: String, pub output: String }

pub struct MasterControl {
    // For sending commands/responses to ProcOrc
    proc_orc_cmd_tx: Sender<String>,
    proc_orc_resp_rx: Receiver<String>,
    // For sending stdin to ProcOrc
    proc_orc_stdin_tx: Sender<String>
}

pub type TaskId = String;

#[derive(Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub command: String,
    pub height: u16,
    pub width: u16
}

impl Task {
    pub fn new(id: &str, name: &str, command: &str, height: u16, width: u16) -> Task {
        Task {
            id: id.into(),
            name: name.into(),
            command: command.into(),
            height,
            width
        }
    }

}

//  All of the threaded functionality  lives in the  "real" orchestrator class
//  and then this should become a facade that communicates with the real class
//  via channels. Make it simple for us to use the facade from the main thread
//  without needing mutable references to the backing threads every-damn-where
struct ProcessOrchestrator {
    // Track all of our registered tasks
    tasks: HashMap<String, Task>,

    // Should we keep running?
    shutdown: bool,

    // Channels for command / response operations
    command_rx: Receiver<String>,
    resp_tx: Sender<String>,

    // Channels for aggregated STDIN/OUT forwarding
    output_tx: Sender<ProcOutput>,
    input_tx: Sender<String>,
    input_rx: Receiver<String>,

    // Channels for communicating with individual processes
    proc_io_channels: HashMap::<String, (Sender<String>, Receiver<String>)>,
    proc_command_channels: HashMap::<String, (Sender<String>, Receiver<String>)>,
    active_proc: Option<String>
}
