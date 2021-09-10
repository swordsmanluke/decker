use crate::rex::{ProcessOrchestrator, ProcOutput, Task};
use crate::rex::child::ChildProcess;
use std::sync::mpsc::{Sender, Receiver, channel, TryRecvError};
use std::collections::HashMap;
use std::thread;
use log::info;

impl ProcessOrchestrator {
    /***
    Create a new ProcessOrchestrator.
    @arg output_tx: A sender to transmit aggregated output
     */
    pub fn new(output_tx: Sender<ProcOutput>, cmd_rx: Receiver<String>, resp_tx: Sender<String>) -> ProcessOrchestrator {
        let (input_tx, input_rx) = channel();
        let proc_io_channels = HashMap::<String, (Sender<String>, Receiver<String>)>::new();
        let proc_command_channels = HashMap::<String, (Sender<String>, Receiver<String>)>::new();

        ProcessOrchestrator {
            tasks: HashMap::new(),
            command_rx: cmd_rx,
            resp_tx: resp_tx,
            output_tx,
            input_tx,
            input_rx,
            proc_io_channels,
            proc_command_channels,
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
            self.process_output()?;
            self.process_commands()?;

            if self.shutdown {
                info!("Shutting down Orchestrator");
                break;
            }
        }
        Ok(())
    }

    fn forward_input(&mut self) -> anyhow::Result<()>{
        match &self.input_rx.try_recv() {
            Ok(input) => {
                info!("Received input: {}", input);
                match &self.active_proc {
                    Some(proc_name) => {
                        // Forward these bytes to the active process
                        let (tx, _) = self.proc_io_channels.get_mut(proc_name.as_str()).unwrap();
                        tx.send(input.clone())?;
                    }
                    None => {info!("No active task. Ignoring!");}
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(_) => { /* TODO */ }
        }
        Ok(())
    }

    fn process_output(&mut self) -> anyhow::Result<()>{
        self.proc_io_channels.iter().for_each({|(name, (_, rx))|
            match rx.try_recv() {
                Ok(s) => {
                    let proc_output = ProcOutput{name: name.clone(), output: s};
                    self.output_tx.send(proc_output).unwrap()  ; }
                Err(TryRecvError::Empty) => {}
                Err(_) => { /* TODO */ }
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
    Delete a task by name
     */
    fn delete(&mut self, name: &String) -> anyhow::Result<()> {
        self.tasks.remove(name);
        self.proc_command_channels.remove(name);
        self.proc_io_channels.remove(name);

        Ok(())
    }

    /***
    Execute a task by name
     */
    fn execute(&mut self, name: &str) -> anyhow::Result<()> {
        match self.tasks.get(name) {
            None => {}
            Some(task) => {
                let (out_tx, out_rx) = channel();
                let (status_tx, status_rx) = channel();
                let mut new_kid = ChildProcess::new(task.command.as_str(),
                                                    out_tx, status_tx.clone(),
                                                    (task.height, task.width));

                // TODO: What if this task already has named channels? Should I only create once
                //       and reuse? Or replace them every time?
                self.proc_io_channels.insert(name.to_string(), (new_kid.input_tx(), out_rx));
                self.proc_command_channels.insert(name.to_string(), (status_tx, status_rx));

                thread::spawn( move || {
                    new_kid.run().unwrap();
                });
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
            "execute" => { self.execute(data) }
            "activate" => { self.activate_proc(data) }
            "register" => { self.register_task(data) }
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

    fn register_task(&mut self, task_str: &str) -> anyhow::Result<()>{
        let task: Task = serde_json::from_str(task_str)?;
        self.tasks.insert(task.name.clone(), task);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn instance() -> ProcessOrchestrator {
        let (output_tx, _) = channel();
        let (_, cmd_rx) = channel();
        let (resp_tx, _) = channel();
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