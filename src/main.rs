use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread;

use rex::child::ChildProcess;
use std::io::{Read, Write, stdout};
use log::info;
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::File;
use termion::raw::IntoRawMode;
use bytes::Bytes;

mod rex;

/***
A non-blocking read that's ok with an empty buffer
 ***/
fn read_output<T>(rx: &mut Receiver<T>) -> anyhow::Result<Option<T>> {
    match rx.try_recv() {
        Ok(s) => Ok(Some(s)),
        Err(TryRecvError::Empty) => Ok(None),
        Err(e) => Err(e.into())
    }
}

fn run() -> anyhow::Result<()> {
    init_logging()?;

    let (output_tx, mut output_rx) = channel();
    let (status_tx, mut status_rx) = channel();
    let mut proc = ChildProcess::new(output_tx, status_tx, (24, 120));
    let input_tx = proc.input_tx();

    thread::spawn( move || {
        proc.run().expect("Child crashed");
        info!("Process exited! Begin shutdown...");
        proc.shutdown().unwrap();
    });

    let mut stdin = termion::async_stdin();
    let mut stdout = stdout().into_raw_mode()?;

    while None == read_output(&mut status_rx)? {
        // read stdin and forward it to the proc.
        let mut input = String::new();
        stdin.read_to_string(&mut input)?;
        if !input.is_empty() {
            input_tx.send(input)?;
        }

        // read stdout and display it
        let output = String::from_utf8(read_output(&mut output_rx)?.unwrap_or(Bytes::from(&b""[..])).to_vec())?;
        write!(stdout, "{}", output)?;
        stdout.flush()?;
    }

    Ok(())
}

fn init_logging() -> anyhow::Result<()> {
    CombinedLogger::init(
        vec![
            WriteLogger::new(LevelFilter::Info, Config::default(), File::create("log/hex.log")?),
        ]
    )?;

    Ok(())
}

fn main() {
    // Create a master session
    // Spawn a child process in another thread
    //   give it the appropriate halves of Input/Output channels
    // Input Thread: Forward stdin to the child's Input channel
    // Output Thread: Forward stdout from the child to the Output channel
    match run() {
        Ok(_) => { println!("{}", "Shutdown!") },
        Err(err) => { println!("{:?}", err)}
    }
}
