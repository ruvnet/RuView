//! Handler for `--benchmark` CLI flag.

use crate::vital_signs;

/// Run vital sign detection benchmark on synthetic frames.
pub fn run_benchmark(frames: usize) {
    eprintln!("Running vital sign detection benchmark ({frames} frames)...");
    let (total, per_frame) = vital_signs::run_benchmark(frames);
    eprintln!();
    eprintln!("Summary: {total:?} total, {per_frame:?} per frame");
}

