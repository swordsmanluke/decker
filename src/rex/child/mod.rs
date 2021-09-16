/***
* Child Process wrapper
* Keeps track of all the things we need for trafficing I/O between processes
***/
mod child_process;

pub struct ChildProcess {
    pub command: String,
    pub path: String,
    pub size: (u16,u16),
}