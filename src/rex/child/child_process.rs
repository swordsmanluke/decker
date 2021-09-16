use crate::rex::child::ChildProcess;
use portable_pty::CommandBuilder;

impl ChildProcess {
    pub fn new(command: &str, path: &str, size: (u16,u16)) -> ChildProcess {
        ChildProcess {
            command: command.to_owned(),
            path: path.to_owned(),
            size: size,
        }
    }

    pub fn command_for_pty(&self) -> CommandBuilder {
        let mut cmd_and_args = self.command.split_ascii_whitespace();
        let command = cmd_and_args.next().unwrap();
        let args = cmd_and_args.collect::<Vec<_>>();

        let mut cmd = CommandBuilder::new(command);
        cmd.cwd(self.path.clone());
        if args.len() > 0 { cmd.args(args); }

        cmd
    }
}