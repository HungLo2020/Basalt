mod commands;
mod router;
mod usage;

use router::CliCommand;

pub fn run(args: &[String]) -> Result<(), String> {
    let command = CliCommand::parse(args)?;
    command.execute(args)
}
