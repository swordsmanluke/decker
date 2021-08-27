use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::io::{Read, Write, stdout};
use log::info;
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::File;
use termion::raw::IntoRawMode;
use crate::rex::{Task, MasterControl, TaskId};
use crate::rex::terminal::{Vt100Translator, View, TerminalLocation, TerminalSize};

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
    let mut mcp = MasterControl::new(output_tx);
    mcp.register(Task::new("bash", "bash", "bash") )?;

    let input_tx = mcp.input_tx();

    mcp.execute(&"bash".to_string())?;
    mcp.activate_proc(&"bash".to_string())?;

    loop {
        // read stdin and forward it to the proc.
        let mut input = String::new();
        let mut translator = Vt100Translator::new();
        let task_id: TaskId = "bash".into();
        let v = View { location: TerminalLocation::new(3, 5), dimensions: TerminalSize::new(24, 80) };
        translator.register(task_id, v);
        stdin.read_to_string(&mut input)?;
        if !input.is_empty() {
            info!("Sending input: {}", input);
            input_tx.send(input)?;
        }

        // read stdout and display it
        let output = read_output(&mut output_rx)?;
        match output {
            Some(pout) => {
                translator.push(pout.name, &pout.output);
                translator.write(&mut stdout);
                stdout.flush()?;
            }
            None => {}
        }
    }
    // Ok(())
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
