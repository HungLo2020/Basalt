//! Progress reporting and cancellation for long-running operations.
//!
//! Front ends pass a [`Progress`] into operations such as core installs, ROM/save syncs, and the
//! MattMC install. Core calls back with [`ProgressUpdate`]s from the worker thread and checks the
//! [`CancelToken`] at safe points: between files, between download chunks, and never once an
//! operation has started changing an installation in place.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{CoreError, CoreResult};

/// What [`ProgressUpdate::completed`] and [`ProgressUpdate::total`] count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressUnit {
    Bytes,
    Items,
}

#[derive(Clone, Debug)]
pub struct ProgressUpdate {
    /// Human-readable description of the current step, e.g. "Downloading MattMC v1.2".
    pub message: String,
    pub completed: u64,
    /// `None` when the size of the step is unknown (e.g. a download without Content-Length).
    pub total: Option<u64>,
    pub unit: ProgressUnit,
}

impl ProgressUpdate {
    /// Completion in `0.0..=1.0`, or `None` when the total is unknown.
    pub fn fraction(&self) -> Option<f32> {
        match self.total {
            Some(0) => Some(1.0),
            Some(total) => Some((self.completed as f64 / total as f64).clamp(0.0, 1.0) as f32),
            None => None,
        }
    }
}

/// Shared flag a front end sets to ask a running operation to stop.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

type ProgressCallback = dyn Fn(&ProgressUpdate) + Send + Sync;

/// Progress sink plus cancellation token handed to long-running operations.
#[derive(Clone, Default)]
pub struct Progress {
    callback: Option<Arc<ProgressCallback>>,
    cancel: CancelToken,
}

impl fmt::Debug for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Progress")
            .field("has_callback", &self.callback.is_some())
            .field("cancel", &self.cancel)
            .finish()
    }
}

impl Progress {
    /// No reporting and no way to cancel.
    pub fn none() -> Self {
        Self::default()
    }

    /// Reports every update to `callback` (called on the worker thread) and stops when
    /// `cancel` is cancelled.
    pub fn new(
        callback: impl Fn(&ProgressUpdate) + Send + Sync + 'static,
        cancel: CancelToken,
    ) -> Self {
        Self {
            callback: Some(Arc::new(callback)),
            cancel,
        }
    }

    /// Shares `other`'s cancellation but reports nothing; for sub-steps whose own progress
    /// would conflict with the parent's.
    pub(crate) fn cancel_only(other: &Progress) -> Self {
        Self {
            callback: None,
            cancel: other.cancel.clone(),
        }
    }

    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }

    pub(crate) fn report(
        &self,
        message: impl Into<String>,
        completed: u64,
        total: Option<u64>,
        unit: ProgressUnit,
    ) {
        if let Some(callback) = &self.callback {
            callback(&ProgressUpdate {
                message: message.into(),
                completed,
                total,
                unit,
            });
        }
    }

    pub(crate) fn report_items(&self, message: impl Into<String>, completed: u64, total: u64) {
        self.report(message, completed, Some(total), ProgressUnit::Items);
    }

    /// A step with no measurable size, shown as indeterminate.
    pub(crate) fn report_step(&self, message: impl Into<String>) {
        self.report(message, 0, None, ProgressUnit::Items);
    }

    /// Returns [`CoreError::Cancelled`] if cancellation was requested.
    pub(crate) fn check_cancelled(&self) -> CoreResult<()> {
        if self.cancel.is_cancelled() {
            Err(CoreError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn progress_forwards_updates_and_observes_cancellation() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let cancel = CancelToken::new();
        let progress = Progress::new(
            move |update| sink.lock().unwrap().push(update.clone()),
            cancel.clone(),
        );

        progress.report_items("Copying", 1, 4);
        assert!(progress.check_cancelled().is_ok());
        cancel.cancel();
        assert!(matches!(
            progress.check_cancelled(),
            Err(CoreError::Cancelled)
        ));

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].fraction(), Some(0.25));
    }

    #[test]
    fn progress_none_is_silent_and_never_cancelled() {
        let progress = Progress::none();
        progress.report_step("anything");
        assert!(progress.check_cancelled().is_ok());
    }
}
