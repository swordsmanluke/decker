use crate::rex::child::ChildProcess;
use std::sync::mpsc::{Sender, Receiver, channel, TryRecvError};
use std::collections::HashMap;
use std::thread;
use log::info;

pub(crate) mod child;

pub type ProcOutput = (String, String);

// FIXME: All of the threaded stuff should be moved into a "real orchestrator" class
//        and then this should become a facade that communicates with the real class
//        via channels. Make it simple for us to use the facade from the main thread
//        without needing mutable references to the backing threads every-damn-where
pub struct ProcessOrchestrator {
    output_tx: Sender<ProcOutput>,
    input_tx: Sender<String>,
    input_rx: Receiver<String>,
    tasks: HashMap<String, Task>,
    proc_io_channels: HashMap::<String, (Sender<String>, Receiver<String>)>,
    proc_command_channels: HashMap::<String, (Sender<String>, Receiver<String>)>,
    active_proc: Option<String>
}

pub struct Task {
    pub(crate) name: String,
    pub(crate) command: String
}

impl ProcessOrchestrator {
    /***
    Create a new ProcessOrchestrator.
    @arg output_tx: A sender to transmit aggregated output
     */
    pub fn new(output_tx: Sender<ProcOutput>) -> ProcessOrchestrator {
        let (input_tx, input_rx) = channel();
        let processes = HashMap::<String, Task>::new();
        let proc_io_channels = HashMap::<String, (Sender<String>, Receiver<String>)>::new();
        let proc_command_channels = HashMap::<String, (Sender<String>, Receiver<String>)>::new();

        ProcessOrchestrator {
            output_tx,
            input_tx,
            input_rx,
            tasks: processes,
            proc_io_channels,
            proc_command_channels,
            active_proc: None
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()>{
        loop {
            self.forward_input()?;
            self.process_output()?;
        }
        Ok(())
    }

    fn process_output(&mut self) -> anyhow::Result<()>{
        self.proc_io_channels.iter().for_each({|(name, (_, rx))|
            match rx.try_recv() {
                Ok(s) => { self.output_tx.send((name.clone(), s)).unwrap()  ; }
                Err(TryRecvError::Empty) => {}
                Err(_) => { /* TODO */ }
            }
        });

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

    /***
    Get a Sender<String> clone on which to forward data from stdin
     */
    pub fn input_tx(&self) -> Sender<String> {
        self.input_tx.clone()
    }

    /***
    Register a new task with the orchestrator
     */
    pub fn register(&mut self, task: Task) -> anyhow::Result<()> {
        self.tasks.insert(task.name.clone(), task);

        Ok(())
    }

    /***
    Delete a task by name
     */
    pub fn delete(&mut self, name: &String) -> anyhow::Result<()> {
        self.tasks.remove(name);
        self.proc_command_channels.remove(name);
        self.proc_io_channels.remove(name);

        Ok(())
    }

    /***
    Execute a task by name
     */
    pub fn execute(&mut self, name: &String) -> anyhow::Result<()> {
        match self.tasks.get(name.as_str()) {
            None => {}
            Some(task) => {
                let (out_tx, out_rx) = channel();
                let (status_tx, status_rx) = channel();

                let mut new_kid = ChildProcess::new(
                    task.command.as_str(),
                    out_tx, status_tx.clone(),
                    (24, 80));

                // TODO: What if this task already has named channels? Should I only create once
                //       and reuse? Or replace them every time?
                self.proc_io_channels.insert(task.name.clone(), (new_kid.input_tx(), out_rx));
                self.proc_command_channels.insert(task.name.clone(), (status_tx, status_rx));

                thread::spawn( move || {
                    new_kid.run().unwrap();
                });
            }
        }

        Ok(())
    }

    /***
    List the current processes (interactive or not) by name
     */
    pub fn tasks(&self) -> Vec<String> {
        self.tasks.keys().map( |c| c.clone() ).collect()
    }

    /***
    Activate a child process
     */
    pub fn activate_proc(&mut self, handle: &String) -> anyhow::Result<()> {
        self.active_proc = Some(handle.clone());
        Ok(())
    }

    /***
    What is the currently activated process?
     */
    pub fn active_proc(&self) -> Option<String> {
        self.active_proc.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn instance() -> ProcessOrchestrator {
        let (output_tx, _) = channel();
        let po = ProcessOrchestrator::new(output_tx);
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

    #[test]
    fn registering_task_stores_task() {
        let mut po = instance();
        let name = String::from("a Task");
        let task = Task{
            name: name.clone(),
            command: "echo 'hello world!'".into()
        };

        po.register(task).unwrap();

        assert!(po.tasks().contains(&name))
    }
}