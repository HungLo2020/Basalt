use super::runners::RunnerKind;

#[derive(Clone)]
pub struct GameEntry {
    pub name: String,
    pub runner_kind: RunnerKind,
    pub launch_target: String,
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
}

pub const ALL_DISCOVER_RUNNERS: [DiscoverRunner; 2] = [DiscoverRunner::Mattmc, DiscoverRunner::Steam];

pub struct SteamDiscoverReport {
    pub found: usize,
    pub added: usize,
    pub already_exists: usize,
}

pub struct DiscoverReport {
    pub mattmc: Option<DiscoverResult>,
    pub steam: Option<SteamDiscoverReport>,
}
