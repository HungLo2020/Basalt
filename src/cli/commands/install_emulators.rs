use crate::core;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt install-emulators".to_string());
    }

    let report = core::install_emulation_runtime()?;

    println!(
        "Emulation runtime ready: {} | cores ready: {}",
        report.runtime_ready, report.cores_ready
    );
    println!("ROM folders: ~/Games/Emulators/roms/nes and ~/Games/Emulators/roms/gba");
    println!("Save folder: ~/Games/Emulators/saves/<system>");

    Ok(())
}
