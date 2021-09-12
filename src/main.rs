use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::io::{Read, Write, stdout};
use log::info;
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::File;
use termion::raw::IntoRawMode;
use crate::rex::{MasterControl, TaskId};
use crate::rex::terminal::pane::Pane;
use crate::rex::terminal::PaneManager;
use crate::rex::config::load_task_config;

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
    let hex_cfg = load_task_config().unwrap();

    let mut stdin = termion::async_stdin();
    let mut stdout = stdout().into_raw_mode()?;

    let (output_tx, mut output_rx) = channel();
    let mut mcp = MasterControl::new(output_tx);
    let mut pane_manager = PaneManager::new();

    let input_tx = mcp.input_tx();

    // create panes from cfg
    for p in hex_cfg.panes {
        let new_pane = Pane::new(&p.task_id, p.x, p.y, p.height, p.width);
        pane_manager.register(p.task_id, new_pane);
    }

    //  and register tasks from cfg
    for task in hex_cfg.tasks {
        let pane = pane_manager.find_by_id(&task.id);
        match pane {
            None => {
                mcp.register(task, None)?;
            }
            Some(p) => {
                mcp.register(task.clone(), Some((p.width, p.height)))?;
            }
        }
    }
    let task_id: TaskId = TaskId::from("zsh");
    mcp.activate_proc(&task_id, pane_manager.find_by_id("main").unwrap())?;
    mcp.execute(&task_id)?;

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
                pane_manager.write(&mut stdout)?;
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
