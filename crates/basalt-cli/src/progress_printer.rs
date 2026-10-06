//! Renders core progress updates as one redrawn line per step on stderr.
//!
//! Each new step finishes the previous line, so a run leaves a short log:
//!
//! ```text
//! Checking RetroArch runtime...
//! Downloading NES core  100%  (1.2 MB / 1.2 MB)
//! Installing NES core...
//! ```
//!
//! Nothing is printed when stderr is not a terminal, so scripts see unchanged output.

use std::io::{IsTerminal, Write};
use std::sync::{Arc, Mutex};

use basalt_core::{CancelToken, Progress, ProgressUnit, ProgressUpdate};

#[derive(Default)]
struct LineState {
    message: String,
    rendered: String,
    line_open: bool,
}

pub(super) struct ProgressPrinter {
    state: Option<Arc<Mutex<LineState>>>,
}

impl ProgressPrinter {
    pub(super) fn new() -> Self {
        let state = std::io::stderr()
            .is_terminal()
            .then(|| Arc::new(Mutex::new(LineState::default())));
        Self { state }
    }

    pub(super) fn progress(&self) -> Progress {
        let Some(state) = self.state.clone() else {
            return Progress::none();
        };

        Progress::new(
            move |update| {
                if let Ok(mut state) = state.lock() {
                    draw(&mut state, update);
                }
            },
            CancelToken::new(),
        )
    }

    /// Ends the current progress line so normal output starts on a fresh line.
    pub(super) fn finish(&self) {
        if let Some(Ok(mut state)) = self.state.as_ref().map(|state| state.lock())
            && state.line_open
        {
            eprintln!();
            state.line_open = false;
        }
    }
}

impl Drop for ProgressPrinter {
    fn drop(&mut self) {
        self.finish();
    }
}

fn draw(state: &mut LineState, update: &ProgressUpdate) {
    let rendered = render(update);
    if rendered == state.rendered && state.line_open {
        return;
    }

    let mut stderr = std::io::stderr().lock();
    if state.line_open && update.message != state.message {
        let _ = writeln!(stderr);
    }
    // Carriage return + clear line, then redraw.
    let _ = write!(stderr, "\r\x1b[2K{}", rendered);
    let _ = stderr.flush();

    state.message = update.message.clone();
    state.rendered = rendered;
    state.line_open = true;
}

fn render(update: &ProgressUpdate) -> String {
    match (update.unit, update.total) {
        (ProgressUnit::Bytes, Some(total)) => format!(
            "{}  {:>3}%  ({} / {})",
            update.message,
            (update.fraction().unwrap_or(0.0) * 100.0).round() as u32,
            format_bytes(update.completed),
            format_bytes(total)
        ),
        (ProgressUnit::Bytes, None) if update.completed > 0 => {
            format!("{}  {}", update.message, format_bytes(update.completed))
        }
        (ProgressUnit::Items, Some(total)) => {
            format!("{}  {}/{}", update.message, update.completed, total)
        }
        _ => format!("{}...", update.message),
    }
}

fn format_bytes(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= MB {
        format!("{:.1} MB", bytes as f64 / MB)
    } else {
        format!("{:.0} KB", (bytes as f64 / 1024.0).ceil())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(unit: ProgressUnit, completed: u64, total: Option<u64>) -> ProgressUpdate {
        ProgressUpdate {
            message: "Downloading NES core".to_string(),
            completed,
            total,
            unit,
        }
    }

    #[test]
    fn renders_each_kind_of_update() {
        assert_eq!(
            render(&update(ProgressUnit::Bytes, 524_288, Some(1_048_576))),
            "Downloading NES core   50%  (512 KB / 1.0 MB)"
        );
        assert_eq!(
            render(&update(ProgressUnit::Items, 3, Some(10))),
            "Downloading NES core  3/10"
        );
        assert_eq!(
            render(&update(ProgressUnit::Items, 0, None)),
            "Downloading NES core..."
        );
    }
}
