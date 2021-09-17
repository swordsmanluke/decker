use crate::rex::{ProcessOrchestrator, ProcOutput};
use crate::rex::child::ChildProcess;
use std::collections::HashMap;
use std::thread;
use log::info;
use crate::rex::master_control::{RegisterTask, ResizeTask};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use crossbeam_channel::{Sender, Receiver, TryRecvError };
use portable_pty::PtySize;
use std::io::{Read, Write};
use std::process::Command;

impl ProcessOrchestrator {
    /***
    Create a new ProcessOrchestrator.
    @arg output_tx: A sender to transmit aggregated output
     */
    pub fn new(output_tx: Sender<ProcOutput>, cmd_rx: Receiver<String>, resp_tx: Sender<String>, input_rx: Receiver<String>, pane_size: (u16, u16)) -> ProcessOrchestrator {
        let pty = portable_pty::native_pty_system().openpty(PtySize {
            rows: pane_size.0,
            cols: pane_size.1,
            pixel_width: 0,
            pixel_height: 0,
        }).unwrap();

        info!("Created pty");

        ProcessOrchestrator {
            tasks: HashMap::new(),
            sizes: HashMap::new(),
            next_run: HashMap::new(),
            command_rx: cmd_rx,
            resp_tx: resp_tx,
            output_tx,
            input_rx,
            main_pty: pty,
            active_proc: None,
            shutdown: false,
        }
    }

    /***
    Run the processing loop
     */
    pub fn run(&mut self) -> anyhow::Result<()> {
        info!("main: Starting ProcessOrchestrator");
        Self::start_forward_output_loop(self.main_pty.master.try_clone_reader()?, self.output_tx.clone())?;
        Self::start_forward_input_loop(self.input_rx.clone(), self.main_pty.master.try_clone_writer()?, "main".to_string());

        loop {
            self.process_commands()?;
            self.run_periodic_tasks()?;

            if self.shutdown {
                info!("Shutting down Orchestrator");
                break;
            }
        }
        Ok(())
    }

    fn process_commands(&mut self) -> anyhow::Result<()> {
        match self.command_rx.try_recv() {
            Ok(command) => {
                info!("Process Orchestrator: Received command {}!", command);
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
                        let new_kid = ChildProcess::new(task.command.as_str(),
                                                            task.path.as_str(),
                                                            (*height, *width));

                        let run_interactively = match self.active_proc.clone() {
                            None => { false }
                            Some(active_task) => { task_id == active_task }
                        };

                        let pane_id = if run_interactively { "main" } else { task_id }.to_string();

                        info!("{}: Running interactively: {}", pane_id, run_interactively);

                        if run_interactively {
                            self.main_pty.slave.spawn_command(new_kid.command_for_pty()).unwrap();
                        } else {
                            let output_tx = self.output_tx.clone();
                            thread::spawn(move || {
                                Self::capture_output(output_tx, new_kid, pane_id).unwrap();
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn start_forward_output_loop(mut reader: Box<dyn Read + Send>, sender: Sender<ProcOutput>) -> anyhow::Result<()> {
        thread::spawn(move || {
            let pane = "main".to_string(); // Always the same name
            let mut output = [0u8; 1024];
            loop {
                info!("main: Reading from output reader");
                let size = reader.read(&mut output).unwrap_or(0);
                info!("main: Read {} bytes", size);
                if size > 0 {
                    let output = String::from_utf8(output[..size].to_owned()).unwrap();
                    info!("main: Sending {} to MCP", output);
                    sender.send(ProcOutput { name: pane.clone(), output }).unwrap();
                }
            }
        });

        Ok(())
    }

    fn capture_output(sender: Sender<ProcOutput>, child: ChildProcess, pane: String) -> anyhow::Result<()> {
        info!("{}: Running {} non-interactively", pane, child.command);

        let mut cmd_and_args = child.command.split_ascii_whitespace();
        let command = cmd_and_args.next().unwrap();
        let args = cmd_and_args.collect::<Vec<_>>();

        let mut cmd = Command::new(command);
        cmd.current_dir(child.path.clone());
        if args.len() > 0 { cmd.args(args); }

        let stdout = String::from_utf8(cmd.output()?.stdout)?;
        let stderr = String::from_utf8(cmd.output()?.stderr)?;

        if !stdout.is_empty() {
            info!("{}: Sending {}", pane, stdout);
            sender.send(ProcOutput { name: pane.clone(), output: format!("\x1B[2J{}", stdout) })?;
        }

        if !stderr.is_empty() {
            info!("{}: Sending (Err) {}", pane, stderr);
            sender.send(ProcOutput { name: pane, output: stderr })?;
        }
        Ok(())
    }

    fn start_forward_input_loop(input_rx: Receiver<String>, mut input_tx: Box<dyn Write + Send>, pane: String) {
        thread::spawn(move || {
            while let Ok(input) = input_rx.recv() {
                write!(input_tx, "{}", input).unwrap();
                input_tx.flush().unwrap();
            }

            info!("{}: Exited input loop!", pane);
            // Send EOF/^D to kill the PTY
            input_tx.write(&[26, 4]).unwrap();
            input_tx.flush().unwrap();
        });
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
            "resize" => { self.resize_task(data) }
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

    fn register_task(&mut self, register_str: &str) -> anyhow::Result<()> {
        let register: RegisterTask = serde_json::from_str(register_str)?;
        self.sizes.insert(register.task.id.clone(), register.size);
        self.tasks.insert(register.task.id.clone(), register.task);

        Ok(())
    }

    fn resize_task(&mut self, resize_str: &str) -> anyhow::Result<()> {
        let resize: ResizeTask = serde_json::from_str(resize_str)?;
        self.sizes.insert(resize.task_id.clone(), resize.size);

        Ok(())
    }

    fn run_periodic_tasks(&mut self) -> anyhow::Result<()> {
        let now_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        if self.next_run.values().any(|timestamp| *timestamp < now_timestamp) {
            // Only need to check if there's at least one timestamp that's ready to go
            let ready_task_ids = self.tasks.iter().
                filter(|(_, t)| t.period.is_some()). // periodic tasks
                filter(|(id, _)| {                  // which are ready to run
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