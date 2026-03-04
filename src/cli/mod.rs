mod commands;
mod router;
mod usage;

use router::CliCommand;

pub fn run(args: &[String]) -> Result<(), String> {
    let command = CliCommand::parse(args)?;
    command.execute(args)
}

pub fn run_install_mattmc_command() -> Result<(), String> {
    let args = vec!["install-mattmc".to_string()];
    commands::install_mattmc::run(&args)
}