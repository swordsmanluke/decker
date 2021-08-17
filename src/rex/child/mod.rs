/***
* Child Process wrapper
* Keeps track of all the things we need for trafficing I/O between processes
***/
use std::sync::mpsc::{Receiver, Sender, channel, TryRecvError};
use std::io::{Read, Write};
use log::info;
use portable_pty::{CommandBuilder, PtySize, native_pty_system, PtyPair};
use bytes::Bytes;

pub struct ChildProcess {
    shutdown: bool,
    input_receiver: Receiver<String>,
    input_sender: Sender<String>,
    pub output_sender: Sender<Bytes>,
    pub status_sender: Sender<String>,
}

impl ChildProcess {
    pub fn new(out_tx: Sender<Bytes>, status_tx: Sender<String>) -> ChildProcess {
        let (in_tx, in_rx) = channel();
        ChildProcess {
            shutdown: false,
            input_receiver: in_rx,
            input_sender: in_tx,
            output_sender: out_tx,
            status_sender: status_tx
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
     */
    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut child = self.launch()?;

        // forward output
        let mut reader = child.master.try_clone_reader()?;
        let sender = self.output_sender.clone();
        std::thread::spawn( move || {
            loop {
                let mut output = [0u8; 1024];
                let size = reader.read(&mut output).unwrap_or(0);
                sender.send(Bytes::from(output[..size].to_owned())).unwrap();
            };
        });

        loop {
            if self.shutdown {
                info!("received shutdown sequence - leaving");
                break;
            }

            // Still running - process I/O!

            // Consume input
            let input = self.read_input()?;
            info!("rcvd {}", input);
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
            Ok(s) => {
                info!("rcvd input: {}", s.as_str());
                Ok(s)
            },
            Err(TryRecvError::Empty) => Ok(String::new()),
            Err(e) => Err(e.into())
        }
    }

    fn launch(&self) -> anyhow::Result<PtyPair> {
        let pty_sys = native_pty_system();
        let pair = pty_sys.openpty(PtySize {
            rows: 24,
            cols: 80,
            // Not all systems support pixel_width, pixel_height,
            // but it is good practice to set it to something
            // that matches the size of the selected font.  That
            // is more complex than can be shown here in this
            // brief example though!
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let cmd = CommandBuilder::new("/usr/bin/bash");

        pair.slave.spawn_command(cmd)?;
        Ok(pair)
    }
}

/***
Notes:
Interior Mutability - https://doc.rust-lang.org/reference/interior-mutability.html
 */