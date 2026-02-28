//! Progress reporting for long-running imports.

use std::io::Write;

/// Trait for receiving import progress callbacks.
pub trait ProgressReporter {
    /// Called after each batch with cumulative counts.
    fn report(&self, imported: u64, rejected: u64, total: u64);
}

/// A reporter that prints progress to stderr.
pub struct StderrProgress;

impl ProgressReporter for StderrProgress {
    fn report(&self, imported: u64, rejected: u64, total: u64) {
        if total > 0 {
            let pct = (imported + rejected) as f64 / total as f64 * 100.0;
            eprint!("\r  imported: {imported}, rejected: {rejected}, total: {total} ({pct:.1}%)");
            let _ = std::io::stderr().flush();
        }
    }
}

/// A reporter that collects reports for testing.
pub struct CollectingProgress {
    reports: std::sync::Mutex<Vec<(u64, u64, u64)>>,
}

impl Default for CollectingProgress {
    fn default() -> Self {
        Self {
            reports: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl CollectingProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reports(&self) -> Vec<(u64, u64, u64)> {
        self.reports.lock().unwrap().clone()
    }
}

impl ProgressReporter for CollectingProgress {
    fn report(&self, imported: u64, rejected: u64, total: u64) {
        self.reports
            .lock()
            .unwrap()
            .push((imported, rejected, total));
    }
}
