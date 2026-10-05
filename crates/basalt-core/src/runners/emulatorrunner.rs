use crate::emulation;

pub fn launch(launch_target: &str) -> Result<(), String> {
    emulation::launch_target(launch_target)
}
