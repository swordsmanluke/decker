use crate::decker::{ProcessOrchestrator, ProcOutput, TaskId, Task};
use crate::decker::child::ChildProcess;
use std::collections::HashMap;
use std::thread;
use log::{debug, info, error};
use crate::decker::master_control::{RegisterTask, ResizeTask};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use crossbeam_channel::{Sender, Receiver};
use portable_pty::PtySize;
use std::io::{Read, Write};
use std::process::Command;
use anyhow::anyhow;
use std::sync::{Arc, RwLock, LockResult};
use termion::raw::IntoRawMode;

impl ProcessOrchestrator {
    /***
    Create a new ProcessOrchestrator.
    @arg output_tx: A sender to transmit aggregated output
     */
    pub fn new(output_tx: Sender<ProcOutput>, cmd_tx: Sender<String>, cmd_rx: Receiver<String>, resp_tx: Sender<String>, input_rx: Receiver<String>, pane_size: (u16, u16)) -> ProcessOrchestrator {
        let pty = portable_pty::native_pty_system().openpty(PtySize {
            rows: pane_size.1,
            cols: pane_size.0,
            pixel_width: 0,
            pixel_height: 0,
        }).unwrap();

        pty.master.try_clone_writer().unwrap().into_raw_mode().unwrap();

        ProcessOrchestrator {
            tasks: HashMap::new(),
            sizes: HashMap::new(),
            periodic_tasks: Arc::new(RwLock::new(HashMap::new())),
            command_tx: cmd_tx,
            command_rx: cmd_rx,
            resp_tx: resp_tx,
            output_tx,
            input_rx,
            main_pty: pty,
            active_proc: None,
            active_child: None,
            has_active_task: false,
            shutdown: false,
        }
    }

    /***
    Run the processing loop
     */
    pub fn run(&mut self) -> anyhow::Result<()> {
        info!("main: Starting ProcessOrchestrator");
        info!("main: Total tasks: {}", self.tasks.len());

        Self::start_forward_output_loop(self.main_pty.master.try_clone_reader()?, self.output_tx.clone())?;
        Self::start_forward_input_loop(self.input_rx.clone(), self.main_pty.master.try_clone_writer()?, "main".to_string());
        Self::start_period_task_loop(self.periodic_tasks.clone(), self.command_tx.clone());
        self.process_commands()?;
        Ok(())
    }

    fn process_commands(&mut self) -> anyhow::Result<()> {
        while !self.shutdown {
            match self.command_rx.recv() {
                Ok(command) => {
                    info!("Process Orchestrator: Received command {}!", command);
                    let parts = command.split(":").map(|s| s.trim().to_string()).collect::<Vec<String>>();
                    let cmd = parts.first().unwrap(); // command part
                    let data = parts[1..].join(":");

                    self.handle_command(&cmd, &data)?;
                }
                Err(e) => { return Err(e.into()); }
            }
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
                            let child = self.main_pty.slave.spawn_command(new_kid.command_for_pty())?;
                            self.active_child = Some(child);
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
                    sender.send(ProcOutput {    name: pane.clone(), output }).unwrap();
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

        let cmd_result = match command {
            "execute" | "local_execute" => { self.execute(data) }
            "activate" => { self.activate_proc(data) }
            "register" => { self.register_task(data) }
            "resize" => { self.resize_task(data) }
            "running" => { if self.running() { Ok(()) } else { Err(anyhow!("not running")) } }
            _ => {
                info!("Unsupported command: {}", command);
                Ok(())
            }
        };

        if !command.starts_with("local") {
            match cmd_result {
                Err(e) => { self.resp_tx.send(format!("{}: Error - {}", command, e))? }
                Ok(()) => { self.resp_tx.send(format!("{}: Success", command))? }
            }
        }

        Ok(())
    }

    fn running(&mut self) -> bool {
        let child_was_running = self.has_active_task;

        self.has_active_task = match self.active_child.as_mut() {
            None => { false }
            Some(child) => { child.try_wait().unwrap().is_none() }
        };

        if !self.has_active_task {
            // Child is not running. But if it was at the last check, log that it switched off
            if child_was_running {
                info!("main: Active process has stopped");
                self.active_child = None;
                self.active_child = None;
            }
        }

        self.has_active_task
    }

    fn register_task(&mut self, register_str: &str) -> anyhow::Result<()> {
        let register: RegisterTask = serde_json::from_str(register_str)?;
        self.sizes.insert(register.task.id.clone(), register.size);

        if register.task.period_secs.is_some() {
            match self.periodic_tasks.write() {
                Ok(mut period_tasks) => {
                    period_tasks.insert(register.task.id.clone(), register.task.period_secs.unwrap());
                }
                Err(_) => {}
            }
        }

        self.tasks.insert(register.task.id.clone(), register.task);

        Ok(())
    }

    fn resize_task(&mut self, resize_str: &str) -> anyhow::Result<()> {
        let resize: ResizeTask = serde_json::from_str(resize_str)?;
        self.sizes.insert(resize.task_id.clone(), resize.size);

        Ok(())
    }

    fn start_period_task_loop(task_periods: Arc<RwLock<HashMap<TaskId, u64>>>, commander: Sender<String>) {

        let mut last_run_times: HashMap<String, SystemTime> = HashMap::new();

        thread::spawn(move || {
            loop {
                let now = SystemTime::now();
                debug!("PTL: Awake - checking for tasks");

                let ready_task_ids = task_periods.read().unwrap().iter().
                    filter(|(t_id, period)| {
                        let most_recent_run = *last_run_times.get(*t_id).unwrap_or(&UNIX_EPOCH);
                        let time_since = now.duration_since(most_recent_run).unwrap();
                        time_since.as_secs() > **period
                    }).
                    map(|(t_id, _)| t_id.clone()).collect::<Vec<_>>();

                debug!("PTL: Found {} tasks: {:?}", ready_task_ids.len(), ready_task_ids);

                if ready_task_ids.is_empty() {
                    let nap_duration = Duration::from_millis(250);
                    thread::sleep(nap_duration);
                    continue;
                }

                for task_id in ready_task_ids {
                    info!("PTL: Sending local_execute command for: {}", task_id);
                    commander.send(format!("local_execute: {}", task_id.to_owned())).unwrap();
                    last_run_times.insert(task_id, SystemTime::now());
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    fn instance() -> ProcessOrchestrator {
        let (output_tx, _) = unbounded();
        let (cmd_tx, cmd_rx) = unbounded();
        let (resp_tx, _) = unbounded();
        let (_, input_rx) = unbounded();
        let po = ProcessOrchestrator::new(output_tx, cmd_tx, cmd_rx, resp_tx, input_rx, (10, 10));
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