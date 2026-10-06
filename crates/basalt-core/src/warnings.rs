//! Non-fatal problems that core can't return as errors: a file it couldn't migrate, a
//! controller profile that failed to download, a temp folder it couldn't clean up.
//!
//! Core never prints. It records warnings here, and front ends call [`take_warnings`] after an
//! operation to show them (the CLI on stderr, the GUI in its status line).

use std::sync::Mutex;

static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub(crate) fn warn(message: impl Into<String>) {
    if let Ok(mut pending) = PENDING.lock() {
        pending.push(message.into());
    }
}

/// Returns and clears the warnings recorded so far.
pub fn take_warnings() -> Vec<String> {
    PENDING
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_are_returned_once() {
        // Other tests may warn concurrently, so only check for this test's own message.
        warn("test warning: queued");
        assert!(take_warnings().iter().any(|w| w == "test warning: queued"));
        assert!(!take_warnings().iter().any(|w| w == "test warning: queued"));
    }
}
