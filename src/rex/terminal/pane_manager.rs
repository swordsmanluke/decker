use crate::rex::TaskId;
use crate::rex::terminal::PaneManager;
use std::io::Write;
use log::{info, error};
use crate::rex::terminal::pane::Pane;

impl PaneManager {
    pub fn new() -> PaneManager {
        PaneManager {
            panes: Default::default()
        }
    }

    pub fn register(&mut self, task_id: TaskId, pane: Pane) {
        self.panes.insert(task_id, pane);
    }

    pub fn write(&mut self, target: &mut dyn Write) {
        for (task_id, pane) in self.panes.iter_mut() {
            info!("Writing output for {}", task_id);
            pane.write(target).unwrap();
        }
    }

    pub fn push(&mut self, task_id: TaskId, data: &String) {
        match self.panes.get_mut(&task_id) {
            None => {  info!("Received output for unregistered task {}", &task_id); } // Drop data for unknown tasks
            Some(pane) => {
                match pane.push(data) {
                    Ok(_) => {}
                    Err(e) => { error!("Error: {}", e.to_string()) }
                } }
        }
    }
}