pub mod emulatorrunner;
pub mod steamrunner;

use crate::platform;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunnerKind {
    Bash,
    Steam,
    Emulator,
}

pub struct ResolvedTarget {
    pub runner_kind: RunnerKind,
    pub launch_target: String,
}

impl RunnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RunnerKind::Bash => "bash",
            RunnerKind::Steam => "steam",
            RunnerKind::Emulator => "emulator",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "bash" => Some(RunnerKind::Bash),
            "steam" => Some(RunnerKind::Steam),
            "emulator" => Some(RunnerKind::Emulator),
            _ => None,
        }
    }
}

pub fn resolve_add_target(raw_input: &str) -> Result<ResolvedTarget, String> {
    if let Some(appid) = steamrunner::detect_appid(raw_input) {
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

pub fn launch(runner_kind: RunnerKind, launch_target: &str) -> Result<(), String> {
    match runner_kind {
        RunnerKind::Bash => platform::launch_script(launch_target),
        RunnerKind::Steam => steamrunner::launch(launch_target),
        RunnerKind::Emulator => emulatorrunner::launch(launch_target),
    }
}