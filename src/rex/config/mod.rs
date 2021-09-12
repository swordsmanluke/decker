use crate::rex::Task;
use std::fs::File;
use std::io::Read;
use serde::Deserialize;
use toml::de::Error;

#[derive(Deserialize, Clone)]
pub struct HexConfig {
    pub tasks: Vec<Task>,
    pub panes: Vec<PaneDefinition>
}

#[derive(Deserialize, Clone)]
pub struct PaneDefinition {
    pub task_id: String,
    pub x: u16,
    pub y: u16,
    pub height: u16,
    pub width: u16
}

impl PaneDefinition {
    pub fn is_main(&self) -> bool {
        &self.task_id == "main"
    }
}

pub fn load_task_config() -> Option<HexConfig> {
    let mut tasks_file = File::open("config/tasks.toml").unwrap();
    let mut toml_tasks = String::new();
    tasks_file.read_to_string(&mut toml_tasks).unwrap();
    let config: Result<HexConfig, Error> = toml::from_str(&toml_tasks);

    match config {
        Ok(conf) => {
            match how_many_mains(&conf.panes) {
                0 => { panic!("No 'main' layout! Make one of your panes' task_id = \"main\""); },
                1 => { Some(conf) }, // perfect!
                _ => { panic!("More than one pane with 'main' task_id in tasks.toml!"); }
            }
        },
        Err(err) => {
            println!("Configuration error: {}", err);
            None
        }
    }
}

fn how_many_mains(panes: &Vec<PaneDefinition>) -> usize {
    panes.iter().filter(|p| p.is_main()).count()
}