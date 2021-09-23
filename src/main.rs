use std::io::{Read, Write, stdout, Stdout};
use log::{ info, error };
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::File;
use termion::raw::{IntoRawMode, RawTerminal};
use std::thread;
use crate::decker::{MasterControl, TaskId, ProcessOrchestrator, ProcOutput};
use crate::decker::terminal::{Pane, PaneManager, ScrollMode};
use crate::decker::config::load_task_config;
use std::time::{SystemTime, Duration};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use termion::AsyncReader;

mod decker;

fn run() -> anyhow::Result<()> {
    init_logging()?;
    let deck_cfg = load_task_config().unwrap();

    // base-level stdin/out channels
    let mut stdin = termion::async_stdin();
    let stdout = stdout().into_raw_mode()?;

    // The channels we need for comms
    // input:  StdIn -> Active Process
    // output: Active Process -> StdOut
    // cmd:    MCP commands -> Process Orchestrator
    // resp:   Proc. Orc. command response -> MCP
    // output is 'bounded' to create backpressure that prevents overwhelming the rendering thread.
    let (input_tx, input_rx) = unbounded();
    let (output_tx, output_rx) = bounded(50);
    let (cmd_tx, cmd_rx) = unbounded();
    let (resp_tx, resp_rx) = unbounded();

    // Pane Manager is a glorified hash map. It provides methods for working
    // with panes without having to call .get().unwrap() everywhere.
    let mut pane_manager = PaneManager::new();

    // Register all the configured Panes
    for p in deck_cfg.panes {
        let mut new_pane = Pane::new(&p.task_id, p.x, p.y, p.height, p.width);
        if p.is_main() { new_pane.set_scroll_mode(ScrollMode::Scroll); }
        pane_manager.register(p.task_id, new_pane);
    }

    let main_pane = pane_manager.find_by_id("main").unwrap();

    // Process Orchestrator is in charge of managing all of the processes and forwarding IO
    // It's got to live in a different thread, however, so we communicate with it via the MCP
    let orchestrator = ProcessOrchestrator::new(output_tx, cmd_tx.clone(), cmd_rx, resp_tx, input_rx, (main_pane.width(), main_pane.height()));
    start_orchestrator(orchestrator);

    // MasterControl is the nice, useful frontend that controls Process Orchestrator.
    // It gives us easy methods for registering and executing tasks, etc.
    let mut mcp = MasterControl::new(cmd_tx, resp_rx);

    //  Now we can register all the configured Tasks
    for mut task in deck_cfg.tasks {
        task.cache_period(); // TODO: This is an ugly solution. We don't call 'Task::new', so we don't have the usual hook to do this sorta call
        match pane_manager.find_by_id(&task.id) {
            None => {
                mcp.register(task, None)?;
            }
            Some(p) => {
                mcp.register(task.clone(), Some((p.width(), p.height())))?;
                mcp.execute(&task.id)?;
            }
        }
    }

    // TODO: Pull the default main task from the cfg instead of hardcoding it.
    let task_id: TaskId = TaskId::from("zsh");
    mcp.activate_proc(&task_id, pane_manager.find_by_id("main").unwrap())?;
    mcp.execute(&task_id)?;

    println!("\x1b[2J"); // clear screen before we begin

    start_output_forwarding_thread(stdout, output_rx, pane_manager);
    run_input_forwarding_loop(&mut stdin, input_tx, &mut mcp); // doesn't return until shutdown

    Ok(())
}

fn run_input_forwarding_loop(stdin: &mut AsyncReader, input_tx: Sender<String>, mcp: &mut MasterControl) {
    let mut buffer: [u8; 1] = [0; 1];

    loop {
        // Reading stdin 1 byte at a time. For some reason, calling 'read' leads to
        // receiving errors. Calling read_to_string works, but then we're creating
        // Strings _all the damn time_ for no reason. So instead.... just read one
        // byte at a time. /shrug
        if let Ok(_) = stdin.read_exact(&mut buffer[..1]) {
            info!("main: Processing input: '{}'", buffer[0] as char);
            if buffer[0] == 3 { // Ctrl-C
                if !mcp.running().unwrap() {
                    info!("main: ^C means shutdown!");
                    break;
                };
            }

            match input_tx.send(String::from(buffer[0] as char)) {
                Ok(_) => {}
                Err(err) => {
                    error!("main: {}", err);
                    break;
                }
            }
        } else {
            thread::sleep(Duration::from_millis(30));
        }
    }
    info!("main: Exited top-level input forwarding");
}

fn start_output_forwarding_thread(mut stdout: RawTerminal<Stdout>, output_rx: Receiver<ProcOutput>, mut pane_manager: PaneManager) {
    thread::spawn(move || {
        info!("main: Starting Output caputure thread");
        let last_printed = SystemTime::UNIX_EPOCH;
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
}

fn start_orchestrator(mut orchestrator: ProcessOrchestrator) {
    thread::spawn(move || {
        match orchestrator.run() {
            Ok(_) => { info!("main: ProcessOrchestrator stopped"); }
            Err(e) => { error!("main: ProcessOrchestator crashed: {}", e) }
        }
    });
}

fn init_logging() -> anyhow::Result<()> {
    CombinedLogger::init(
        vec![
            WriteLogger::new(LevelFilter::Info, Config::default(), File::create("log/decker.log")?),
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
