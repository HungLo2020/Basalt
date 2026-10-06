use crate::emulation;
use crate::error::CoreResult;

pub fn launch(launch_target: &str) -> CoreResult<()> {
    emulation::launch_target(launch_target)
}
