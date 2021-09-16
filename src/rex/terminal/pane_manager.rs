use crate::rex::TaskId;
use crate::rex::terminal::{PaneManager, Pane};
use std::io::Write;
use log::{info, error};

impl PaneManager {
    pub fn new() -> PaneManager {
        PaneManager {
            panes: Default::default()
        }
    }

    pub fn register(&mut self, task_id: TaskId, pane: Pane) {
        self.panes.insert(task_id, pane);
    }

    pub fn find_by_id(&mut self, id: &str) -> Option<&Pane> {
        match self.panes.iter().find(|(task_id, _) | **task_id == id) {
            None => { None }
            Some((_, pane)) => { Some(pane) }
        }
    }

    pub fn write(&mut self, target: &mut dyn Write) -> anyhow::Result<()>{
        for (task_id, pane) in self.panes.iter_mut() {
            pane.write(target).unwrap();
        }
        // send the cursor to the main pane's location
        let main_pane = self.find_by_id("main").unwrap();
        main_pane.take_cursor(target)?;
        Ok(())
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