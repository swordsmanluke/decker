use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::io::{Read, Write, stdout};
use std::thread;
use log::info;
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::File;
use termion::raw::IntoRawMode;
use crate::rex::{ProcessOrchestrator, Task};

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

    let mut stdin = termion::async_stdin();
    let mut stdout = stdout().into_raw_mode()?;

    let (output_tx, mut output_rx) = channel();
    let mut po = ProcessOrchestrator::new(output_tx);
    po.register(Task{ name: String::from("bash"), command: String::from("bash") } )?;

    let input_tx = po.input_tx();

    po.execute(&"bash".to_string())?;
    po.activate_proc(&"bash".to_string());

    thread::spawn(move ||
        po.run()
    );

    loop {
        // read stdin and forward it to the proc.
        let mut input = String::new();
        stdin.read_to_string(&mut input)?;
        if !input.is_empty() {
            info!("Sending input: {}", input);
            input_tx.send(input)?;
        }

        // read stdout and display it
        let output = read_output(&mut output_rx)?;
        match output {
            Some((name, output)) => {
                write!(stdout, "{}", output)?;
                stdout.flush()?;
            }
            None => {}
        }
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
