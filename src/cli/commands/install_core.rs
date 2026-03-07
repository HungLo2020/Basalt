use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(usage::usage_install_core());
    }

    let system = args[1].trim();
    if system.is_empty() {
        return Err(usage::usage_install_core());
    }

    core::install_emulation_core_for_system(system)?;
    println!("Installed emulator core for system '{}'.", system);
    Ok(())
}
