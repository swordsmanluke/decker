use std::io::{Read, Write, stdout};
use log::{ info, error };
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::File;
use termion::raw::IntoRawMode;
use std::thread;
use crate::rex::{MasterControl, TaskId, ProcessOrchestrator};
use crate::rex::terminal::pane::{Pane, ScrollMode};
use crate::rex::terminal::PaneManager;
use crate::rex::config::load_task_config;
use std::time::{SystemTime, Duration};
use crossbeam_channel::{bounded, unbounded};

mod rex;

fn run() -> anyhow::Result<()> {
    init_logging()?;
    let hex_cfg = load_task_config().unwrap();

    let mut stdin = termion::async_stdin();
    let mut stdout = stdout().into_raw_mode()?;

    let (output_tx, mut output_rx) = bounded(50); // Output to Screen channel
    let (cmd_tx, cmd_rx) = unbounded();             // mcp -> POrc communication
    let (resp_tx, resp_rx) = unbounded();           // POrc -> mcp communication

    let mut orchestrator = ProcessOrchestrator::new(output_tx, cmd_rx, resp_tx);
    let input_tx = orchestrator.input_tx();
    thread::spawn(move || {
        match orchestrator.run() {
            Ok(_) => { info!("main: ProcessOrchestrator stopped"); }
            Err(e) => { error!("main: ProcessOrchestator crashed: {}", e)}
        }
    });

    info!("main: Creating MCP");
    let mut mcp = MasterControl::new(cmd_tx, resp_rx, input_tx);
    let mut pane_manager = PaneManager::new();

    let input_tx = mcp.input_tx();

    // create panes from cfg
    for p in hex_cfg.panes {
        let mut new_pane = Pane::new(&p.task_id, p.x, p.y, p.height, p.width);
        if p.is_main() { new_pane.set_scroll_mode(ScrollMode::Scroll); }
        pane_manager.register(p.task_id, new_pane);
    }

    //  and register tasks from cfg
    for mut task in hex_cfg.tasks {
        task.cache_period(); // TODO: This is an ugly solution
        let pane = pane_manager.find_by_id(&task.id);
        match pane {
            None => {
                mcp.register(task, None)?;
            }
            Some(p) => {
                mcp.register(task.clone(), Some((p.width, p.height)))?;
                mcp.execute(&task.id)?;
            }
        }
    }
    let task_id: TaskId = TaskId::from("zsh");
    mcp.activate_proc(&task_id, pane_manager.find_by_id("main").unwrap())?;
    mcp.execute(&task_id)?;

    println!("\x1b[2J"); // clear screen before we begin

    thread::spawn(move ||{
        info!("main: Starting Output caputure thread");
        let mut last_printed = SystemTime::UNIX_EPOCH;
        // read stdout and display it
        while let Ok(pout) = output_rx.recv() {
            // Capture the output
            pane_manager.push(pout.name, &pout.output);

            // if it's been more than 30 ms, go ahead and render.
            if SystemTime::now().duration_since(last_printed).unwrap().as_millis() > 30 {
                pane_manager.write(&mut stdout).unwrap();
                stdout.flush().unwrap();
            }
        }
        info!("main: Exited top-level output forwarding");
    });

    let mut buffer: [u8; 1] = [0; 1];
    loop {
        // read stdin and forward it to the active proc.
        if let Ok(_) = stdin.read_exact(&mut buffer[..1]) {
            info!("main: Sending input: {:?}", buffer);
            match input_tx.send(String::from(buffer[0] as char)) {
                Ok(_) => {}
                Err(err) => { error!("main: {}", err); break;}
            }
        } else {
            thread::sleep(Duration::from_millis(30) );
        }
    }
    info!("main: Exited top-level input forwarding");

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
        Ok(_) => {},
        Err(err) => { error!("Fatal error {:?}", err.to_string()); }
    }

    println!("\x1B[0m{}", "Shutdown!");
}
