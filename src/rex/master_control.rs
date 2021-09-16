use crate::rex::{MasterControl, ProcOutput, ProcessOrchestrator, Task, TaskId};
use std::thread;
use log::{info, warn, error};
use std::time::Duration;
use std::ops::Deref;
use simple_error::bail;
use serde::{Serialize, Deserialize};
use crate::rex::terminal::pane::Pane;
use crossbeam_channel::{Sender, bounded, unbounded};

pub type PaneSize = Option<(u16, u16)>;

#[derive(Serialize, Deserialize)]
pub struct RegisterTask {
    pub(crate) task: Task,
    pub(crate) size: PaneSize
}

#[derive(Serialize, Deserialize)]
pub struct ResizeTask {
    pub(crate) task_id: TaskId,
    pub(crate) size: PaneSize
}

impl MasterControl {
    pub fn new(output_tx: Sender<ProcOutput>) -> MasterControl {
        let (cmd_tx, cmd_rx) = unbounded();
        let (resp_tx, resp_rx) = unbounded();
        let mut orchestrator = ProcessOrchestrator::new(output_tx, cmd_rx, resp_tx);
        let proc_orc_stdin_tx= orchestrator.input_tx();

        thread::spawn(move || {
            info!("Starting ProcessOrchestrator");
            match orchestrator.run() {
                Ok(_) => { info!("ProcessOrchestrator stopped"); }
                Err(e) => { error!("ProcessOrchestator crashed: {}", e)}
            }
        });

        MasterControl {
            proc_orc_cmd_tx: cmd_tx,
            proc_orc_resp_rx: resp_rx,
            proc_orc_stdin_tx
        }
    }

    /***
    Get a Sender<String> clone on which to forward data from stdin
     */
    pub fn input_tx(&self) -> Sender<String> {
        self.proc_orc_stdin_tx.clone()
    }

    /***
    Register a new task with the orchestrator
     */
    pub fn register(&mut self, task: Task, size: PaneSize) -> anyhow::Result<()> {
        let metadata = RegisterTask { task, size };

        self.send_command("register", &serde_json::to_string(&metadata)?)?;
        let resp = self.await_response("register")?;
        if resp.trim() == "Success" {
            Ok(())
        } else {
            bail!(simple_error::simple_error!(resp));
        }
    }

    pub fn resize(&mut self, task_id: &TaskId, size: PaneSize) -> anyhow::Result<()> {
        let metadata = ResizeTask { task_id: task_id.to_owned(), size };

        self.send_command("resize", &serde_json::to_string(&metadata)?)?;
        let resp = self.await_response("resize")?;
        if resp.trim() == "Success" {
            Ok(())
        } else {
            bail!(simple_error::simple_error!(resp));
        }
    }

    /***
    Select a child process to forward stdin to 
     */
    pub fn activate_proc(&mut self, task_id: &TaskId, pane: &Pane) -> anyhow::Result<()> {
        // TODO: Finish wiring this up.
        //  Probably need to track tasks within ProcessOrchestrator again
        let resize_task = ResizeTask { task_id: task_id.clone(), size: Some((pane.width, pane.height)) };
        self.send_command("resize", &serde_json::to_string(&resize_task)?)?;
        self.await_response("resize")?;

        self.send_command("activate", task_id)?;
        self.await_response("activate")?;

        Ok(())
    }

    /***
    Execute a task by name
     */
    pub fn execute(&mut self, name: &str) -> anyhow::Result<()> {
        self.send_command("execute", name)?;
        self.await_response("execute")?;
        Ok(())
    }

    fn send_command(&self, command: &str, metadata: &str) -> anyhow::Result<()>{
        let data = format!("{}: {}", command, metadata);
        self.proc_orc_cmd_tx.send(data)?;
        Ok(())
    }

    fn await_response(&self, expected_response_type: &str) -> anyhow::Result<String> {
        let half_sec = Duration::new(0, 500_000_000);
        let mut received_response = String::new();
        loop {
            let resp = self.proc_orc_resp_rx.recv_timeout(half_sec)?;
            let parts = resp.split(":").collect::<Vec<&str>>();
            match parts.first() {
                None => { break; } // empty string?! Shouldn't happen.
                Some(response_type) => {
                    if response_type.deref() == expected_response_type {
                        received_response = parts[1..].join(":");
                        break;
                    } else {
                        warn!("Received unexpected response type {}", response_type)
                    }
                }
            }
        }

        Ok(received_response)
    }
}