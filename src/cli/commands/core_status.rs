use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(usage::usage_core_status());
    }

    let system = args[1].trim();
    if system.is_empty() {
        return Err(usage::usage_core_status());
    }

    let core_installed = core::is_emulation_core_installed_for_system(system)?;
    let save_sync_supported = core::is_emulation_save_sync_supported_for_system(system);

    println!("System: {}", system);
    println!(
        "Core installed: {}",
        if core_installed { "yes" } else { "no" }
    );
    println!(
        "Save sync supported: {}",
        if save_sync_supported { "yes" } else { "no" }
    );

    Ok(())
}
