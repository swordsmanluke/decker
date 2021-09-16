use crate::rex::{ProcessOrchestrator, ProcOutput};
use crate::rex::child::ChildProcess;
use std::collections::HashMap;
use std::thread;
use log::{info, error};
use crate::rex::master_control::{RegisterTask, ResizeTask};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use crossbeam_channel::{Sender, Receiver, TryRecvError, unbounded};

impl ProcessOrchestrator {
    /***
    Create a new ProcessOrchestrator.
    @arg output_tx: A sender to transmit aggregated output
     */
    pub fn new(output_tx: Sender<ProcOutput>, cmd_rx: Receiver<String>, resp_tx: Sender<String>) -> ProcessOrchestrator {
        let (input_tx, input_rx) = unbounded();
        let proc_io_channels = HashMap::<String, Sender<String>>::new();

        ProcessOrchestrator {
            tasks: HashMap::new(),
            sizes: HashMap::new(),
            next_run: HashMap::new(),
            command_rx: cmd_rx,
            resp_tx: resp_tx,
            output_tx,
            input_tx,
            input_rx,
            active_pty_channel: proc_io_channels,
            active_proc: None,
            shutdown: false
        }
    }

    /***
    Get a Sender<String> clone on which to forward data from stdin
     */
    pub fn input_tx(&self) -> Sender<String> {
        self.input_tx.clone()
    }

    /***
    Run the processing loop
     */
    pub fn run(&mut self) -> anyhow::Result<()>{
        loop {
            self.forward_input()?;
            self.process_commands()?;
            self.run_periodic_tasks()?;

            if self.shutdown {
                info!("Shutting down Orchestrator");
                break;
            }
        }
        Ok(())
    }

    fn forward_input(&mut self) -> anyhow::Result<()>{

        let tx= match &self.active_proc {
            Some(proc_name) => {
                // Forward these bytes to the active process
                self.active_pty_channel.get_mut(proc_name.as_str())
            }
            None => {info!("No active task. Ignoring!"); None }
        };

        if tx.is_none() {
            // Nothing to do, ATM.
            return Ok(())
        }

        let tx = tx.unwrap().clone();
        let input_rx = self.input_rx.clone();

        thread::spawn(move || {
            while let Ok(input) = input_rx.recv() {
                // TODO: Update tx?
                tx.send(input.clone()).unwrap();
            }
        });

        Ok(())
    }

    fn process_commands(&mut self) -> anyhow::Result<()>{
        match self.command_rx.try_recv() {
            Ok(command) => {
                let parts = command.split(":").map(|s| s.trim().to_string()).collect::<Vec<String>>();
                let cmd = parts.first().unwrap(); // command part
                let data = parts[1..].join(":");

                self.handle_command(&cmd, &data)?;
            }
            Err(TryRecvError::Empty) => {}
            Err(e) => { return Err(e.into()); }
        }

        Ok(())
    }


    /***
    Execute a task by name
     */
    fn execute(&mut self, task_id: &str) -> anyhow::Result<()> {
        match self.tasks.get(task_id) {
            None => {
                info!("Could not find task {} to execute in {:?}", task_id, self.tasks.keys());
            }
            Some(task) => {
                match task.period_secs {
                    None => {}
                    Some(period) => {
                        let next_run = SystemTime::now().checked_add(Duration::new(period, 0)).unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs();
                        self.next_run.insert(task_id.to_string(), next_run);
                    }
                }

                let size = self.sizes.get(task_id);

                match size.unwrap() {
                    None => {
                        info!("Cannot run {} - no terminal size was assigned! Does this have a pane?", task_id);
                    }
                    Some((width, height)) => {
                        let mut new_kid = ChildProcess::new(task.command.as_str(),
                                                            task.path.as_str(),
                                                            self.output_tx.clone(),
                                                            (*height, *width));

                        self.active_pty_channel.insert(task_id.to_string(), new_kid.input_tx());

                        let run_interactively = match self.active_proc.clone() {
                            None => { false }
                            Some(active_task) => { task_id == active_task }
                        };

                        let pane_id = match self.active_proc.clone() {
                            None => { task_id }
                            Some(active_task) => { if active_task == task_id { "main" } else { task_id} }
                        }.to_string();

                        thread::spawn( move || {
                            new_kid.run(pane_id, run_interactively).unwrap();
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /***
    Activate a child process
     */
    fn activate_proc(&mut self, name: &str) -> anyhow::Result<()> {
        // FIXME: Verify this name is in 'tasks'
        self.active_proc = Some(name.to_string());
        Ok(())
    }

    /***
    Handle a requested execution
     */
    fn handle_command(&mut self, command: &str, data: &str) -> anyhow::Result<()> {
        info!("Commanded to {}: {}", command, data);

        match match command {
            "execute"  => { self.execute(data) }
            "activate" => { self.activate_proc(data) }
            "register" => { self.register_task(data) }
            "resize"   => { self.resize_task(data) }
            _ => {
                info!("Unsupported command: {}", command);
                Ok(())
            }
        } {
            Err(e) => { self.resp_tx.send(format!("{}: Error - {}", command, e))? }
            Ok(()) => { self.resp_tx.send(format!("{}: Success", command))? }
        }

        Ok(())
    }

    fn register_task(&mut self, register_str: &str) -> anyhow::Result<()>{
        let register: RegisterTask = serde_json::from_str(register_str)?;
        self.sizes.insert(register.task.id.clone(), register.size);
        self.tasks.insert(register.task.id.clone(), register.task);

        Ok(())
    }

    fn resize_task(&mut self, resize_str: &str) -> anyhow::Result<()>{
        let resize: ResizeTask = serde_json::from_str(resize_str)?;
        self.sizes.insert(resize.task_id.clone(), resize.size);

        Ok(())
    }

    fn run_periodic_tasks(&mut self) -> anyhow::Result<()> {
        let now_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        if self.next_run.values().any(|timestamp| *timestamp < now_timestamp) {
            // Only need to check if there's at least one timestamp that's ready to go
            let ready_task_ids = self.tasks.iter().
                filter(|(_, t)| t.period.is_some()). // period tasks
                filter(|(id, t)| {          // which are redy to run
                    match self.next_run.get(*id) {
                        Some(next_run_timestamp) => { now_timestamp >= *next_run_timestamp }
                        None => { false }
                    }
                }).map(|(id, _)| id.clone()).
                collect::<Vec<String>>();

            //  Separate loops to satisfy borrow checker
            for id in ready_task_ids {
                self.execute(&id)?;
            }
        }

        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    fn instance() -> ProcessOrchestrator {
        let (output_tx, _) = unbounded();
        let (_, cmd_rx) = unbounded();
        let (resp_tx, _) = unbounded();
        let po = ProcessOrchestrator::new(output_tx, cmd_rx, resp_tx);
        po
    }

    #[test]
    fn no_active_proc_after_creation() {
        let po = instance();
        assert_eq!(po.active_proc, None);
    }

    #[test]
    fn setting_active_proc_works() {
        let mut po = instance();
        po.activate_proc(&"a handle".to_owned()).unwrap();
        assert_eq!(po.active_proc, Some(String::from("a handle")));
    }
}