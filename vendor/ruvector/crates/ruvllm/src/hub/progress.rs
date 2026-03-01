//! Progress tracking for download and upload operations

use std::time::{Duration, Instant};

/// Progress bar styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStyle {
    /// Simple bar: [=====>    ] 50%
    Bar,
    /// Detailed: [=====>    ] 50% (5.2 MB/s, ETA: 30s)
    Detailed,
    /// Minimal: 50% complete
    Minimal,
}

/// Progress indicator for terminal output
pub struct ProgressBar {
    /// Total bytes
    total: u64,
    /// Current bytes
    current: u64,
    /// Start time
    start_time: Instant,
    /// Last update time
    last_update: Instant,
    /// Progress style
    style: ProgressStyle,
    /// Bar width
    width: usize,
    /// Show in terminal
    enabled: bool,
}

impl ProgressBar {
    /// Create a new progress bar
    pub fn new(total: u64) -> Self {
        Self {
            total,
            current: 0,
            start_time: Instant::now(),
            last_update: Instant::now(),
            style: ProgressStyle::Detailed,
            width: 40,
            enabled: true,
        }
    }

    /// Set progress style
    pub fn with_style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }

    /// Set bar width
    pub fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    /// Enable or disable output
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Update progress
    pub fn update(&mut self, current: u64) {
        self.current = current;
        self.last_update = Instant::now();

        if self.enabled {
            self.render();
        }
    }

    /// Increment progress
    pub fn inc(&mut self, delta: u64) {
        self.update(self.current + delta);
    }

    /// Finish progress bar
    pub fn finish(&mut self) {
        self.current = self.total;
        if self.enabled {
            self.render();
            println!(); // New line after completion
        }
    }

    /// Render progress bar to terminal
    fn render(&self) {
        let percentage = if self.total == 0 {
            0.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        };

        match self.style {
            ProgressStyle::Bar => {
                let filled = ((percentage / 100.0) * self.width as f64) as usize;
                let bar = format!(
                    "[{}>{}] {:.0}%",
                    "=".repeat(filled),
                    " ".repeat(self.width.saturating_sub(filled)),
                    percentage
                );
                print!("\r{}", bar);
            }
            ProgressStyle::Detailed => {
                let filled = ((percentage / 100.0) * self.width as f64) as usize;
                let speed = self.calculate_speed();
                let eta = self.calculate_eta();

                let bar = format!(
                    "[{}>{}] {:.0}% ({}, ETA: {})",
                    "=".repeat(filled),
                    " ".repeat(self.width.saturating_sub(filled)),
                    percentage,
                    format_speed(speed),
                    format_duration(eta)
                );
                print!("\r{}", bar);
            }
            ProgressStyle::Minimal => {
                print!("\r{:.0}% complete", percentage);
            }
        }

        use std::io::{self, Write};
        let _ = io::stdout().flush();
    }

    /// Calculate download/upload speed in bytes/sec
    fn calculate_speed(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.current as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Calculate estimated time remaining
    fn calculate_eta(&self) -> Duration {
        let remaining = self.total.saturating_sub(self.current);
        let speed = self.calculate_speed();

        if speed > 0.0 {
            let seconds = remaining as f64 / speed;
            Duration::from_secs_f64(seconds)
        } else {
            Duration::from_secs(0)
        }
    }
}

/// Format speed as human-readable string
fn format_speed(bps: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bps >= GB {
        format!("{:.2} GB/s", bps / GB)
    } else if bps >= MB {
        format!("{:.2} MB/s", bps / MB)
    } else if bps >= KB {
        format!("{:.2} KB/s", bps / KB)
    } else {
        format!("{:.0} B/s", bps)
    }
}

/// Format duration as human-readable string
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Progress callback function type
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Progress indicator trait
pub trait ProgressIndicator {
    /// Update progress
    fn update(&mut self, current: u64, total: u64);
    /// Finish progress
    fn finish(&mut self);
}

impl ProgressIndicator for ProgressBar {
    fn update(&mut self, current: u64, _total: u64) {
        self.update(current);
    }

    fn finish(&mut self) {
        self.finish();
    }
}

/// Multi-progress manager for multiple concurrent operations
pub struct MultiProgress {
    bars: Vec<ProgressBar>,
}

impl MultiProgress {
    /// Create a new multi-progress manager
    pub fn new() -> Self {
        Self { bars: Vec::new() }
    }

    /// Add a progress bar
    pub fn add(&mut self, bar: ProgressBar) -> usize {
        let id = self.bars.len();
        self.bars.push(bar);
        id
    }

    /// Update a specific progress bar
    pub fn update(&mut self, id: usize, current: u64) {
        if let Some(bar) = self.bars.get_mut(id) {
            bar.update(current);
        }
    }

    /// Finish all progress bars
    pub fn finish_all(&mut self) {
        for bar in &mut self.bars {
            bar.finish();
        }
    }
}

impl Default for MultiProgress {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_creation() {
        let pb = ProgressBar::new(1000);
        assert_eq!(pb.total, 1000);
        assert_eq!(pb.current, 0);
    }

    #[test]
    fn test_progress_update() {
        let mut pb = ProgressBar::new(1000).enabled(false);
        pb.update(500);
        assert_eq!(pb.current, 500);
    }

    #[test]
    fn test_progress_increment() {
        let mut pb = ProgressBar::new(1000).enabled(false);
        pb.inc(100);
        pb.inc(100);
        assert_eq!(pb.current, 200);
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(format_speed(500.0), "500 B/s");
        assert_eq!(format_speed(1024.0 * 10.0), "10.00 KB/s");
        assert_eq!(format_speed(1024.0 * 1024.0 * 5.0), "5.00 MB/s");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3700)), "1h 1m");
    }

    #[test]
    fn test_multi_progress() {
        let mut mp = MultiProgress::new();
        let id1 = mp.add(ProgressBar::new(100).enabled(false));
        let id2 = mp.add(ProgressBar::new(200).enabled(false));

        mp.update(id1, 50);
        mp.update(id2, 100);

        assert_eq!(mp.bars[id1].current, 50);
        assert_eq!(mp.bars[id2].current, 100);
    }
}
