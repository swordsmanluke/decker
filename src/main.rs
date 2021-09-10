use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::io::{Read, Write, stdout};
use log::info;
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::File;
use termion::raw::IntoRawMode;
use crate::rex::{Task, MasterControl, TaskId};
use crate::rex::terminal::pane::Pane;
use crate::rex::terminal::PaneManager;

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

    let input_tx = mcp.input_tx();

    let task_id: TaskId = "bash".into();
    let height: u16 = 24;
    let width: u16 = 80;
    let pane = Pane::new(&task_id, 5, 5, height, width);

    mcp.register(Task::new(&task_id, &task_id, "bash", height, width) )?;
    mcp.execute(&task_id.to_string())?;
    mcp.activate_proc(&task_id)?;

    let mut pane_manager = PaneManager::new();
    pane_manager.register(task_id, pane);
    let mut input = String::new();

    println!("\x1b[2J"); // clear screen before we begin

    loop {
        // read stdin and forward it to the active proc.
        stdin.read_to_string(&mut input)?;
        if !input.is_empty() {
            info!("Sending input: {:?}", input);
            input_tx.send(input.clone())?;
            input.clear();
        }

        // read stdout and display it
        let output = read_output(&mut output_rx)?;
        match output {
            Some(pout) => {
                pane_manager.push(pout.name, &pout.output);
                pane_manager.write(&mut stdout);
                stdout.flush()?;
            }
            None => {}
        }
    }
    // uncomment after making an exit function
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
