use super::runners::RunnerKind;

#[derive(Clone)]
pub struct GameEntry {
    pub name: String,
    pub runner_kind: RunnerKind,
    pub launch_target: String,
}

#[derive(Clone)]
pub struct Playlist {
    pub name: String,
    pub game_names: Vec<String>,
}

pub enum DiscoverResult {
    Added,
    AlreadyExists,
    NotFound,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiscoverRunner {
    Mattmc,
    Steam,
    Emulators,
}

pub const ALL_DISCOVER_RUNNERS: [DiscoverRunner; 3] = [
    DiscoverRunner::Mattmc,
    DiscoverRunner::Steam,
    DiscoverRunner::Emulators,
];

pub struct SteamDiscoverReport {
    pub found: usize,
    pub added: usize,
    pub already_exists: usize,
}

pub struct EmulatorDiscoverReport {
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub already_exists: usize,
}

pub struct DiscoverReport {
    pub mattmc: Option<DiscoverResult>,
    pub steam: Option<SteamDiscoverReport>,
    pub emulators: Option<EmulatorDiscoverReport>,
}
