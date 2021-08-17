use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread;

use rex::child::ChildProcess;
use std::io::Read;

mod rex;

/***
    A non-blocking read that's ok with an empty buffer
     ***/
fn read_output(rx: &mut Receiver<String>) -> anyhow::Result<String> {
    match rx.try_recv() {
        Ok(s) => Ok(s),
        Err(TryRecvError::Empty) => Ok(String::new()),
        Err(e) => Err(e.into())
    }
}

fn run() -> anyhow::Result<()> {
    let (output_tx, mut output_rx) = channel();
    let mut proc = ChildProcess::new(output_tx);
    let input_tx = proc.input_tx();

    thread::spawn( move || {
        proc.run().expect("Child crashed!");
    });

    let mut stdin = termion::async_stdin();
    loop {
        // read stdin and forward it to the proc.
        let mut input = String::new();
        stdin.read_to_string(&mut input)?;
        if !input.is_empty() { input_tx.send(input)?; }

        // read stdout and display it
        print!("{}", read_output(&mut output_rx)?)
    }
}

fn main() {

    // Create a master session
    // Spawn a child process in another thread
    //   give it the appropriate halves of Input/Output channels
    // Input Thread: Forward stdin to the child's Input channel
    // Output Thread: Forward stdout from the child to the Output channel
    match run() {
        Ok(_) => { print!("{}", "Shutting down!") },
        Err(err) => { print!("{:?}", err)}
    }
}
