use crossbeam_channel::{Sender, Receiver};
use std::collections::HashMap;


pub(crate) mod child;
mod process_orchestrator;
mod master_control;
pub(crate) mod terminal;
pub(crate) mod config;

use serde::{Deserialize, Serialize};
use crate::decker::master_control::PaneSize;
use lazy_static::lazy_static;
use portable_pty::{PtyPair, Child};
use std::sync::{Arc, RwLock};

pub struct ProcOutput { pub name: String, pub output: String }

pub struct MasterControl {
    // For sending commands/responses to ProcOrc
    proc_orc_cmd_tx: Sender<String>,
    proc_orc_resp_rx: Receiver<String>,
}

pub type TaskId = String;

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub command: String,
    pub path: String,
    pub period: Option<String>,
    period_secs: Option<u64>
}

lazy_static! {
    static ref DIGITS_REGEX: regex::Regex = regex::Regex::new("([0-9]+).*").unwrap();
}

impl Task {
    pub fn cache_period(&mut self) {
        let period = self.period.clone().unwrap_or(String::new());

        if self.period_secs.is_none() && self.period.is_some() {
            // Determine the number of seconds
            let base = DIGITS_REGEX.
                captures(&period).unwrap().
                get(1).unwrap().
                as_str().to_string().
                parse::<u64>().unwrap();
            let period_seconds = match period.chars().last() {
                Some('h') => base * 3600,
                Some('m') => base * 60,
                _ => base
            };

            self.period_secs = Some(period_seconds)
        }
    }
}

//  All of the threaded functionality lives in the process orchestrator class
//  comms are performed via channels with the MCP. Make it simple for us to
//  use the facade from the main thread without needing mutable references to
//  the backing threads every-damn-where
pub struct ProcessOrchestrator {
    // Track all of our registered tasks
    tasks: HashMap<String, Task>,
    sizes: HashMap<String, PaneSize>,
    next_run: Arc<RwLock<HashMap<TaskId, u64>>>,

    // Should we keep running?
    shutdown: bool,

    // Channels for command / response operations
    command_tx: Sender<String>,
    command_rx: Receiver<String>,
    resp_tx: Sender<String>,

    // Channels for aggregated STDIN/OUT forwarding
    output_tx: Sender<ProcOutput>,
    input_rx: Receiver<String>,

    // The PTY for the main window
    main_pty: PtyPair,
    // the name and child process of the activated task
    active_proc: Option<String>,
    active_child: Option<Box<dyn Child + Send>>,
    has_active_task: bool // convenience field
}
