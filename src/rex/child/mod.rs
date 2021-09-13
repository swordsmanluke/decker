/***
* Child Process wrapper
* Keeps track of all the things we need for trafficing I/O between processes
***/
use std::sync::mpsc::{Receiver, Sender, channel, TryRecvError};
use std::io::{Read, Write};
use log::info;
use portable_pty::{CommandBuilder, PtySize, native_pty_system, PtyPair};

pub struct ChildProcess {
    command: String,
    path: String,
    shutdown: bool,
    input_receiver: Receiver<String>,
    input_sender: Sender<String>,
    pub output_sender: Sender<String>,
    pub status_sender: Sender<String>,
    size: (u16,u16)
}

impl ChildProcess {
    pub fn new(command: &str, path: &str, out_tx: Sender<String>, status_tx: Sender<String>, size: (u16,u16)) -> ChildProcess {
        let (in_tx, in_rx) = channel();
        ChildProcess {
            command: command.to_owned(),
            path: path.to_owned(),
            shutdown: false,
            input_receiver: in_rx,
            input_sender: in_tx,
            output_sender: out_tx,
            status_sender: status_tx,
            size: size
        }
    }

    pub fn shutdown(&mut self) -> anyhow::Result<()> {
        self.status_sender.send("shutdown".to_owned())?;
        self.shutdown = true;
        Ok(())
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
    pub fn run(&mut self) -> anyhow::Result<()> {
        info!("Running {}", self.command);
        let mut child = self.launch()?;

        // forward output
        let mut reader = child.master.try_clone_reader()?;
        let sender = self.output_sender.clone();
        let (stop_tx, stop_rx) = channel();
        let command = self.command.clone();

        sender.send(String::from("\x1b[2J")); // Clear the screen when we launch

        let out_loop = std::thread::spawn( move || {
            let mut output = [0u8; 1024];

            while let Err(TryRecvError::Empty) = stop_rx.try_recv() {
                let size = reader.read(&mut output).unwrap_or(0);
                sender.send(String::from_utf8(output[..size].to_owned()).unwrap()).unwrap();
            };
            info!("Exited {} output loop!", command)
        });

        loop {
            if self.shutdown {
                info!("received shutdown sequence - exiting input loop");
                stop_tx.send("staaahp")?;
                // FIXME: Apparently I can't use '?' here because the type isn't sized. But unwrap is fine?
                out_loop.join().unwrap();
                break;
            }

            let pid = child.master.process_group_leader();
            if pid.is_none() {
                info!("received shutdown sequence - leaving");
                break;
            }

            // Consume input
            let input = self.read_input()?;
            write!(child.master, "{}", input)?;
            child.master.flush()?;
        }

        Ok(())
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

/***
Notes:
Interior Mutability - https://doc.rust-lang.org/reference/interior-mutability.html
 */