use crate::rex::child::ChildProcess;
use std::sync::mpsc::{Sender, channel, TryRecvError};
use std::io::{Read, Write};
use log::info;
use portable_pty::{CommandBuilder, PtySize, native_pty_system, Child, MasterPty, SlavePty};
use std::time::Duration;

struct PtyProcess {
    master: Box<dyn MasterPty + Send>,
    slave: Box<dyn SlavePty + Send>,
    process: Box<dyn Child + Send>
}

impl ChildProcess {
    pub fn new(command: &str, path: &str, out_tx: Sender<String>, status_tx: Sender<String>, size: (u16,u16)) -> ChildProcess {
        let (in_tx, in_rx) = channel();
        ChildProcess {
            command: command.to_owned(),
            path: path.to_owned(),
            input_receiver: in_rx,
            input_sender: in_tx,
            output_sender: out_tx,
            status_sender: status_tx,
            size: size,
            process: None
        }
    }

    /***
    Get a transmitter to send input to this child
     */
    pub fn input_tx(&self) -> Sender<String> {
        self.input_sender.clone()
    }

    /***
    Launches the child's process and runs until the process exits
    ***/
    pub fn run(&mut self, interactive: bool) -> anyhow::Result<()> {
        info!("Running {}", self.command);
        let mut child_proc = self.launch()?;

        let reader = child_proc.master.try_clone_reader()?;
        let sender = self.output_sender.clone();
        let command = self.command.clone();
        let mut process = child_proc.process;

        std::thread::spawn( move || {
            ChildProcess::forward_output(reader, sender, command.clone()).unwrap();
            info!("{}: Exited output loop!", command)
        });

        while let None = process.try_wait().unwrap() {
            // Consume input
            if interactive {
                while let Ok(input) = self.input_receiver.recv_timeout(Duration::new(0, 500)) {
                    write!(child_proc.master, "{}", input)?;
                    child_proc.master.flush()?;
                }
            }
        }

        info!("{}: Exited input loop!", self.command.clone());
        // Send EOF/^D to kill the PTY
        let bytes_written = child_proc.master.write(&[26, 4])?;
        child_proc.master.flush()?;

        Ok(())
    }

    fn forward_output(mut reader: Box<dyn Read + Send>, sender: Sender<String>, cmd: String) -> anyhow::Result<()>{
        let mut output = [0u8; 1024];
        let mut first_out = true;
        loop {
            let size = reader.read(&mut output)?;

            let prefix = if first_out { "\x1b[2J" } else { "" };
            let child_output = String::from_utf8(output[..size].to_owned())?;
            if first_out { first_out = false }

            // Exit code?
            if output[size-2] == 94 && output[size-1] == 90 {
                break;
            };

            sender.send(format!("{}{}", prefix, child_output))?;
        }

        Ok(())
    }

    fn launch(&self) -> anyhow::Result<PtyProcess> {
        let pty_sys = native_pty_system();
        let pair = pty_sys.openpty(PtySize {
            rows: self.size.0,
            cols: self.size.1,
            // Not all systems support pixel_width, pixel_height,
            // but it is good practice to set it to something
            // that matches the size of the selected font.  That
            // is more complex than can be shown here in this
            // brief example though!
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd_and_args = self.command.split_ascii_whitespace();
        let command = cmd_and_args.next().unwrap();
        let args = cmd_and_args.collect::<Vec<_>>();

        let mut cmd = CommandBuilder::new(command);
        cmd.cwd(self.path.clone());
        if args.len() > 0 { cmd.args(args); }

        let child = pair.slave.spawn_command(cmd)?;
        let process = PtyProcess {
            master: pair.master,
            slave: pair.slave,
            process: child
        };
        Ok(process)
    }
}