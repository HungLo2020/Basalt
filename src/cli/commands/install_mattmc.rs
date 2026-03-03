use std::env;
use std::fs;
use std::path::Path;

const SOURCE_SYNC_SCRIPT_PATH: &str = "/mnt/storage/Storage/Sync/MattMC/SyncGameData.sh";

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt install_mattmc".to_string());
    }

    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    let target_dir = Path::new(&home).join("Documents").join("MattMC");
    let target_script_path = target_dir.join("SyncGameData.sh");

    if target_script_path.exists() {
        println!("SyncGameData.sh already exists at {}", target_script_path.display());
        return Ok(());
    }

    fs::create_dir_all(&target_dir)
        .map_err(|err| format!("Failed to create MattMC directory: {}", err))?;

    let source_path = Path::new(SOURCE_SYNC_SCRIPT_PATH);
    if !source_path.exists() || !source_path.is_file() {
        return Err(format!(
            "Source script does not exist or is not a file: {}",
            SOURCE_SYNC_SCRIPT_PATH
        ));
    }

    fs::copy(source_path, &target_script_path)
        .map_err(|err| format!("Failed to copy SyncGameData.sh: {}", err))?;

    println!(
        "Installed SyncGameData.sh to {}",
        target_script_path.display()
    );
    Ok(())
}