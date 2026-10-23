mod steam;

use crate::error::CoreResult;
use crate::platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerKind {
    Bash,
    Catalog,
    Steam,
    Flatpak,
}

pub struct ResolvedTarget {
    pub runner_kind: RunnerKind,
    pub launch_target: String,
}

impl RunnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RunnerKind::Bash => "bash",
            RunnerKind::Catalog => "catalog",
            RunnerKind::Steam => "steam",
            RunnerKind::Flatpak => "flatpak",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "bash" => Some(RunnerKind::Bash),
            "catalog" => Some(RunnerKind::Catalog),
            "steam" => Some(RunnerKind::Steam),
            "flatpak" => Some(RunnerKind::Flatpak),
            _ => None,
        }
    }
}

pub fn resolve_add_target(raw_input: &str) -> CoreResult<ResolvedTarget> {
    if let Some(appid) = steam::detect_appid(raw_input) {
        return Ok(ResolvedTarget {
            runner_kind: RunnerKind::Steam,
            launch_target: appid,
        });
    }

    let canonical_script_path = platform::normalize_script_path(raw_input)?;
    Ok(ResolvedTarget {
        runner_kind: RunnerKind::Bash,
        launch_target: canonical_script_path,
    })
}

pub fn launch(runner_kind: RunnerKind, launch_target: &str) -> CoreResult<()> {
    match runner_kind {
        RunnerKind::Bash => platform::launch_script(launch_target),
        RunnerKind::Catalog => crate::catalog_launch::launch(launch_target),
        RunnerKind::Steam => steam::launch(launch_target),
        RunnerKind::Flatpak => crate::flatpak::launch(launch_target),
    }
}
