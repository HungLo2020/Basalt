use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err(usage::usage_settings());
    }

    match args[1].as_str() {
        "get" => run_get(args),
        "set" => run_set(args),
        _ => Err(usage::usage_settings()),
    }
}

fn run_get(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(usage::usage_settings_get());
    }

    let paths = core::load_emulation_remote_paths()?;
    println!("Remote ROM root: {}", paths.roms_root_dir);
    println!("Remote Saves root: {}", paths.saves_root_dir);
    Ok(())
}

fn run_set(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(usage::usage_settings_set());
    }

    let mut roms_root_override: Option<String> = None;
    let mut saves_root_override: Option<String> = None;

    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--roms-root" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(usage::usage_settings_set());
                };
                roms_root_override = Some(value.trim().to_string());
                index += 2;
            }
            "--saves-root" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(usage::usage_settings_set());
                };
                saves_root_override = Some(value.trim().to_string());
                index += 2;
            }
            _ => return Err(usage::usage_settings_set()),
        }
    }

    if roms_root_override.is_none() && saves_root_override.is_none() {
        return Err(usage::usage_settings_set());
    }

    let existing = core::load_emulation_remote_paths()?;
    let roms_root_dir = roms_root_override
        .as_deref()
        .unwrap_or(existing.roms_root_dir.as_str());
    let saves_root_dir = saves_root_override
        .as_deref()
        .unwrap_or(existing.saves_root_dir.as_str());

    let saved = core::save_emulation_remote_paths(roms_root_dir, saves_root_dir)?;
    println!("Saved remote ROM root: {}", saved.roms_root_dir);
    println!("Saved remote Saves root: {}", saved.saves_root_dir);

    Ok(())
}
