use super::commands;
use super::usage;

#[derive(Clone, Copy)]
pub(super) enum CliCommand {
    Help,
    Add,
    AddToPlaylist,
    Remove,
    RemoveFromPlaylist,
    RemoveAll,
    List,
    Discover,
    InstallEmulators,
    InstallMattmc,
    Launch,
    BackupMattmc,
    SyncMattmc,
}

impl CliCommand {
    pub(super) fn parse(args: &[String]) -> Result<Self, String> {
        match args.first().map(String::as_str) {
            Some("help") | Some("-h") | Some("--help") => Ok(Self::Help),
            Some("add") => Ok(Self::Add),
            Some("add-to-playlist") => Ok(Self::AddToPlaylist),
            Some("remove") => Ok(Self::Remove),
            Some("remove-from-playlist") => Ok(Self::RemoveFromPlaylist),
            Some("remove-all") => Ok(Self::RemoveAll),
            Some("list") => Ok(Self::List),
            Some("discover") => Ok(Self::Discover),
            Some("install-emulators") => Ok(Self::InstallEmulators),
            Some("install-mattmc") => Ok(Self::InstallMattmc),
            Some("launch") => Ok(Self::Launch),
            Some("backup-mattmc") => Ok(Self::BackupMattmc),
            Some("sync-mattmc") => Ok(Self::SyncMattmc),
            Some(other) => Err(format!("Unknown command: {}\n\n{}", other, usage::full_usage())),
            None => Err(usage::full_usage()),
        }
    }

    pub(super) fn execute(self, args: &[String]) -> Result<(), String> {
        match self {
            Self::Help => {
                println!("{}", usage::full_usage());
                Ok(())
            }
            Self::Add => commands::add::run(args),
            Self::AddToPlaylist => commands::add_to_playlist::run(args),
            Self::Remove => commands::remove::run(args),
            Self::RemoveFromPlaylist => commands::remove_from_playlist::run(args),
            Self::RemoveAll => commands::remove_all::run(args),
            Self::List => commands::list::run(args),
            Self::Discover => commands::discover::run(args),
            Self::InstallEmulators => commands::install_emulators::run(args),
            Self::InstallMattmc => commands::install_mattmc::run(args),
            Self::Launch => commands::launch::run(args),
            Self::BackupMattmc => commands::backup_mattmc::run(args),
            Self::SyncMattmc => commands::sync_mattmc::run(args),
        }
    }
}
