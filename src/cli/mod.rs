mod commands;
mod usage;

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help") | Some("-h") | Some("--help") => {
            println!("{}", usage::full_usage());
            Ok(())
        }
        Some("add") => commands::add::run(args),
        Some("remove") => commands::remove::run(args),
        Some("remove-all") => commands::remove_all::run(args),
        Some("list") => commands::list::run(args),
        Some("discover") => commands::discover::run(args),
        Some("install-mattmc") => commands::install_mattmc::run(args),
        Some("launch") => commands::launch::run(args),
        Some("backup-mattmc") => commands::backup_mattmc::run(args),
        Some("sync-mattmc") => commands::sync_mattmc::run(args),
        Some(other) => Err(format!("Unknown command: {}\n\n{}", other, usage::full_usage())),
        None => Err(usage::full_usage()),
    }
}