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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiscoverResult {
    Added,
    AlreadyExists,
    NotFound,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiscoverRunner {
    Catalog,
    Steam,
}

pub const ALL_DISCOVER_RUNNERS: [DiscoverRunner; 3] = [
    DiscoverRunner::Catalog,
    DiscoverRunner::Steam,
];

pub struct SteamDiscoverReport {
    pub found: usize,
    pub added: usize,
    pub already_exists: usize,
}

pub struct DiscoverReport {
    pub catalog: Option<DiscoverResult>,
    pub steam: Option<SteamDiscoverReport>,
}
