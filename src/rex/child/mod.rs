/***
* Child Process wrapper
* Keeps track of all the things we need for trafficing I/O between processes
***/
use std::sync::mpsc::{Receiver, Sender, channel, TryRecvError};
use std::process::{Command, Child};
use std::io::{Write, Read};

pub struct ChildProcess {
    input_receiver: Receiver<String>,
    input_sender: Sender<String>,
    output_sender: Sender<String>,
}

impl ChildProcess {
    pub fn new(out_tx: Sender<String>) -> ChildProcess {
        let (in_tx, in_rx) = channel();
        ChildProcess {
            input_receiver: in_rx,
            input_sender: in_tx,
            output_sender: out_tx,
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
     */
    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut child: Child = self.launch()?;

        while let None = child.try_wait()? {
            // Still running - process I/O!

            // Send input
            let input = self.read_input()?;
            if !input.is_empty() {
                if child.stdin.is_some() {
                    child.stdin.take().unwrap().write(input.as_bytes())?;
                }
            }

            // forward output
            let mut output = String::new();
            if child.stdout.is_some() {
                child.stdout.take().unwrap().read_to_string(&mut output)?;
            }
            self.output_sender.send(output)?;
        }

        Ok(())
    }

    /***
    A non-blocking read that's ok with an empty buffer
     ***/
    fn read_input(&mut self) -> anyhow::Result<String> {
        match self.input_receiver.try_recv() {
            Ok(s) => Ok(s),
            Err(TryRecvError::Empty) => Ok(String::new()),
            Err(e) => Err(e.into())
        }
    }

    fn launch(&self) -> anyhow::Result<Child>{
        let child = Command::new("/usr/bin/bash")
            .current_dir("/home/lucas")
            .spawn()
            .expect("Failed to launch child");

        Ok(child)
    }

}

/***
Notes:
Interior Mutability - https://doc.rust-lang.org/reference/interior-mutability.html
 */