//! Gamepad input for couch navigation, read on a background thread with gilrs.
//!
//! Qt 6 has no gamepad module, so controller events are translated into simple navigation
//! commands that the QML library grid handles like arrow keys and Enter.

use std::thread;
use std::time::Duration;

use gilrs::{Axis, Button, EventType, Gilrs};

const STICK_PRESS_THRESHOLD: f32 = 0.6;
const STICK_RELEASE_THRESHOLD: f32 = 0.3;
const POLL_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerCommand {
    /// Move the selection by (columns, rows).
    Navigate(i32, i32),
    /// Launch the selected game.
    Activate,
}

/// Starts the controller thread. Returns false when no gamepad backend is available.
pub fn spawn(on_command: impl Fn(ControllerCommand) + Send + 'static) -> bool {
    // Gilrs is not Send, so probe here and create the real instance on the thread.
    if Gilrs::new().is_err() {
        return false;
    }

    thread::spawn(move || {
        let Ok(mut gilrs) = Gilrs::new() else {
            return;
        };
        let mut sticks = StickState::default();

        loop {
            let Some(event) = gilrs.next_event_blocking(Some(POLL_TIMEOUT)) else {
                continue;
            };
            if let Some(command) = sticks.translate(event.event) {
                on_command(command);
            }
        }
    });

    true
}

/// Turns analog stick movement into single steps: one step per push past the threshold.
#[derive(Default)]
struct StickState {
    x_held: bool,
    y_held: bool,
}

impl StickState {
    fn translate(&mut self, event: EventType) -> Option<ControllerCommand> {
        match event {
            EventType::ButtonPressed(button, _) => button_command(button),
            EventType::AxisChanged(axis, value, _) => self.axis_command(axis, value),
            _ => None,
        }
    }

    fn axis_command(&mut self, axis: Axis, value: f32) -> Option<ControllerCommand> {
        match axis {
            Axis::LeftStickX => {
                step(&mut self.x_held, value).map(|dx| ControllerCommand::Navigate(dx, 0))
            }
            // Stick Y is positive when pushed up; rows grow downwards.
            Axis::LeftStickY => {
                step(&mut self.y_held, value).map(|dy| ControllerCommand::Navigate(0, -dy))
            }
            _ => None,
        }
    }
}

fn button_command(button: Button) -> Option<ControllerCommand> {
    match button {
        Button::DPadLeft => Some(ControllerCommand::Navigate(-1, 0)),
        Button::DPadRight => Some(ControllerCommand::Navigate(1, 0)),
        Button::DPadUp => Some(ControllerCommand::Navigate(0, -1)),
        Button::DPadDown => Some(ControllerCommand::Navigate(0, 1)),
        Button::South => Some(ControllerCommand::Activate),
        _ => None,
    }
}

fn step(held: &mut bool, value: f32) -> Option<i32> {
    if value.abs() < STICK_RELEASE_THRESHOLD {
        *held = false;
        return None;
    }
    if *held || value.abs() < STICK_PRESS_THRESHOLD {
        return None;
    }

    *held = true;
    Some(if value > 0.0 { 1 } else { -1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_moves_once_per_push_and_maps_up_to_previous_row() {
        let mut sticks = StickState::default();

        assert_eq!(
            sticks.axis_command(Axis::LeftStickY, 0.9),
            Some(ControllerCommand::Navigate(0, -1))
        );
        // Still held: no repeat.
        assert_eq!(sticks.axis_command(Axis::LeftStickY, 0.95), None);
        // Released, then pushed down.
        sticks.axis_command(Axis::LeftStickY, 0.0);
        assert_eq!(
            sticks.axis_command(Axis::LeftStickY, -0.8),
            Some(ControllerCommand::Navigate(0, 1))
        );
        assert_eq!(
            sticks.axis_command(Axis::LeftStickX, 0.7),
            Some(ControllerCommand::Navigate(1, 0))
        );
    }

    #[test]
    fn buttons_map_to_navigation_and_launch() {
        assert_eq!(
            button_command(Button::DPadLeft),
            Some(ControllerCommand::Navigate(-1, 0))
        );
        assert_eq!(
            button_command(Button::South),
            Some(ControllerCommand::Activate)
        );
        assert_eq!(button_command(Button::East), None);
    }
}
