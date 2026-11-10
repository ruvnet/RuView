use std::collections::VecDeque;
use crate::vital_signs::VitalSigns;

pub const VITAL_MEDIAN_WINDOW: usize = 21;
pub const VITAL_EMA_ALPHA: f64 = 0.02;
pub const HR_MAX_JUMP: f64 = 8.0;
pub const BR_MAX_JUMP: f64 = 2.0;
pub const HR_DEAD_BAND: f64 = 2.0;
pub const BR_DEAD_BAND: f64 = 0.5;

/// Trimmed mean: drop top/bottom 25%, average middle 50%.
pub fn trimmed_mean(buf: &VecDeque<f64>) -> f64 {
    if buf.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = buf.iter().copied().collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let trim = n / 4;
    let middle = &sorted[trim..n - trim.max(0)];
    if middle.is_empty() {
        sorted[n / 2]
    } else {
        middle.iter().sum::<f64>() / middle.len() as f64
    }
}

/// Smooth vital signs components with outlier rejection, trimmed mean, and deadband EMA.
pub fn smooth_vitals_parts(
    smoothed_hr: &mut f64,
    smoothed_br: &mut f64,
    smoothed_hr_conf: &mut f64,
    smoothed_br_conf: &mut f64,
    hr_buffer: &mut VecDeque<f64>,
    br_buffer: &mut VecDeque<f64>,
    raw: &VitalSigns,
) -> VitalSigns {
    let raw_hr = raw.heart_rate_bpm.unwrap_or(0.0);
    let raw_br = raw.breathing_rate_bpm.unwrap_or(0.0);

    let hr_ok = *smoothed_hr < 1.0 || (raw_hr - *smoothed_hr).abs() < HR_MAX_JUMP;
    let br_ok = *smoothed_br < 1.0 || (raw_br - *smoothed_br).abs() < BR_MAX_JUMP;

    if hr_ok && raw_hr > 0.0 {
        hr_buffer.push_back(raw_hr);
        if hr_buffer.len() > VITAL_MEDIAN_WINDOW {
            hr_buffer.pop_front();
        }
    }
    if br_ok && raw_br > 0.0 {
        br_buffer.push_back(raw_br);
        if br_buffer.len() > VITAL_MEDIAN_WINDOW {
            br_buffer.pop_front();
        }
    }

    let trimmed_hr = trimmed_mean(hr_buffer);
    let trimmed_br = trimmed_mean(br_buffer);

    if trimmed_hr > 0.0 {
        if *smoothed_hr < 1.0 {
            *smoothed_hr = trimmed_hr;
        } else if (trimmed_hr - *smoothed_hr).abs() > HR_DEAD_BAND {
            *smoothed_hr = *smoothed_hr * (1.0 - VITAL_EMA_ALPHA) + trimmed_hr * VITAL_EMA_ALPHA;
        }
    }

    if trimmed_br > 0.0 {
        if *smoothed_br < 1.0 {
            *smoothed_br = trimmed_br;
        } else if (trimmed_br - *smoothed_br).abs() > BR_DEAD_BAND {
            *smoothed_br = *smoothed_br * (1.0 - VITAL_EMA_ALPHA) + trimmed_br * VITAL_EMA_ALPHA;
        }
    }

    *smoothed_hr_conf = *smoothed_hr_conf * 0.92 + raw.heartbeat_confidence * 0.08;
    *smoothed_br_conf = *smoothed_br_conf * 0.92 + raw.breathing_confidence * 0.08;

    VitalSigns {
        breathing_rate_bpm: if *smoothed_br > 1.0 {
            Some(*smoothed_br)
        } else {
            None
        },
        heart_rate_bpm: if *smoothed_hr > 1.0 {
            Some(*smoothed_hr)
        } else {
            None
        },
        breathing_confidence: *smoothed_br_conf,
        heartbeat_confidence: *smoothed_hr_conf,
        signal_quality: raw.signal_quality,
    }
}

/// Standalone vitals smoothing filter.
#[derive(Debug, Clone)]
pub struct VitalsFilter {
    pub smoothed_hr: f64,
    pub smoothed_br: f64,
    pub smoothed_hr_conf: f64,
    pub smoothed_br_conf: f64,
    pub hr_buffer: VecDeque<f64>,
    pub br_buffer: VecDeque<f64>,
}

impl Default for VitalsFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl VitalsFilter {
    pub fn new() -> Self {
        Self {
            smoothed_hr: 0.0,
            smoothed_br: 0.0,
            smoothed_hr_conf: 0.0,
            smoothed_br_conf: 0.0,
            hr_buffer: VecDeque::with_capacity(VITAL_MEDIAN_WINDOW),
            br_buffer: VecDeque::with_capacity(VITAL_MEDIAN_WINDOW),
        }
    }

    pub fn smooth(&mut self, raw: &VitalSigns) -> VitalSigns {
        smooth_vitals_parts(
            &mut self.smoothed_hr,
            &mut self.smoothed_br,
            &mut self.smoothed_hr_conf,
            &mut self.smoothed_br_conf,
            &mut self.hr_buffer,
            &mut self.br_buffer,
            raw,
        )
    }
}

pub fn smooth_vitals_node(
    ns: &mut crate::domain::node::NodeState,
    raw: &VitalSigns,
) -> VitalSigns {
    smooth_vitals_parts(
        &mut ns.smoothed_hr,
        &mut ns.smoothed_br,
        &mut ns.smoothed_hr_conf,
        &mut ns.smoothed_br_conf,
        &mut ns.hr_buffer,
        &mut ns.br_buffer,
        raw,
    )
}


