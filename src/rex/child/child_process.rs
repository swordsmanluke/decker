use crate::rex::child::ChildProcess;
use std::sync::mpsc::{Sender, channel, TryRecvError};
use std::io::{Read, Write};
use log::info;
use portable_pty::{CommandBuilder, PtySize, native_pty_system, PtyPair};
use std::time::Duration;

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
            size: size
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
        let mut child = self.launch()?;

        let mut reader = child.master.try_clone_reader()?;
        let sender = self.output_sender.clone();
        let command = self.command.clone();

        std::thread::spawn( move || {
            ChildProcess::forward_output(reader, sender);
            info!("Exited {} output loop!", command)
        });

        if interactive {
            loop {
                let pid = child.master.process_group_leader();
                if pid.is_none() {
                    info!("{}: process exited - leaving", self.command);
                    break;
                }

                // Consume input
                while let Ok(input) = self.input_receiver.recv_timeout(Duration::new(0, 500)) {
                    write!(child.master, "{}", input)?;
                    child.master.flush()?;
                }
            }
        }

        Ok(())
    }

    fn forward_output(mut reader: Box<dyn Read + Send>, sender: Sender<String>) -> anyhow::Result<()>{
        let mut output = [0u8; 1024];
        let mut first_out = true;
        loop {
            let size = reader.read(&mut output)?;
            if size > 0 {
                let prefix = if first_out { "\x1b[2J" } else { "" };
                let child_output = String::from_utf8(output[..size].to_owned())?;
                if first_out { first_out = false }

                sender.send(format!("{}{}", prefix, child_output))?;
            }
        }
    }

    /***
    A non-blocking read that's ok with an empty buffer
     ***/
    fn read_input(&mut self) -> anyhow::Result<String> {
        match self.input_receiver.try_recv() {
            Ok(s) => { Ok(s) },
            Err(TryRecvError::Empty) => Ok(String::new()),
            Err(e) => Err(e.into())
        }
    }

    fn launch(&self) -> anyhow::Result<PtyPair> {
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

        pair.slave.spawn_command(cmd)?;
        Ok(pair)
    }
}