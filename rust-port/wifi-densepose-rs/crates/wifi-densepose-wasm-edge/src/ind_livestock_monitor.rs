//! Livestock monitoring — ADR-041 Category 5 Industrial module.
//!
//! Animal presence and health monitoring in agricultural settings using
//! WiFi CSI sensing.
//!
//! Features:
//! - Presence detection for animals in pens/barns
//! - Abnormal stillness detection (possible illness)
//! - Labored breathing detection (species-configurable BPM ranges)
//! - Escape alert (sudden presence loss after confirmed occupancy)
//!
//! Species breathing ranges (BPM):
//! - Cattle:  12-30
//! - Sheep:   12-20
//! - Poultry: 15-30
//!
//! Budget: L (<2 ms per frame).  Event IDs 530-533.

/// Minimum motion energy to be considered "active".
const MIN_MOTION_ACTIVE: f32 = 0.03;

/// Abnormal stillness threshold (frames at 20 Hz).
/// 5 minutes = 6000 frames.  Animals rarely stay completely motionless
/// for this long unless ill.
const STILLNESS_FRAMES: u32 = 6000;

/// Escape detection: sudden absence after N frames of confirmed presence.
/// 10 seconds of confirmed presence before escape counts.
const MIN_PRESENCE_FOR_ESCAPE: u32 = 200;

/// Absence frames before triggering escape alert (1 second at 20 Hz).
const ESCAPE_ABSENCE_FRAMES: u32 = 20;

/// Labored breathing debounce (frames).
const LABORED_DEBOUNCE: u8 = 20;

/// Stillness alert debounce (fire once, then cooldown).
const STILLNESS_COOLDOWN: u32 = 6000;

/// Escape alert cooldown (frames).
const ESCAPE_COOLDOWN: u16 = 400;

/// Presence report interval (frames, ~10 seconds).
const PRESENCE_REPORT_INTERVAL: u32 = 200;

/// Event IDs (530-series: Industrial/Livestock).
pub const EVENT_ANIMAL_PRESENT: i32 = 530;
pub const EVENT_ABNORMAL_STILLNESS: i32 = 531;
pub const EVENT_LABORED_BREATHING: i32 = 532;
pub const EVENT_ESCAPE_ALERT: i32 = 533;

/// Species type for breathing range configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Species {
    Cattle,
    Sheep,
    Poultry,
    Custom { min_bpm: f32, max_bpm: f32 },
}

impl Species {
    /// Normal breathing range (min, max) in BPM.
    pub const fn breathing_range(&self) -> (f32, f32) {
        match self {
            Species::Cattle => (12.0, 30.0),
            Species::Sheep => (12.0, 20.0),
            Species::Poultry => (15.0, 30.0),
            Species::Custom { min_bpm, max_bpm } => (*min_bpm, *max_bpm),
        }
    }
}

/// Livestock monitor.
pub struct LivestockMonitor {
    /// Configured species.
    species: Species,
    /// Whether animal is currently detected (debounced).
    animal_present: bool,
    /// Consecutive frames with presence.
    presence_frames: u32,
    /// Consecutive frames without presence (after confirmed).
    absence_frames: u32,
    /// Consecutive frames without motion.
    still_frames: u32,
    /// Labored breathing debounce counter.
    labored_debounce: u8,
    /// Stillness alert fired flag.
    stillness_alerted: bool,
    /// Escape cooldown counter.
    escape_cooldown: u16,
    /// Frame counter.
    frame_count: u32,
    /// Last reported breathing BPM.
    last_bpm: f32,
}

impl LivestockMonitor {
    pub const fn new() -> Self {
        Self {
            species: Species::Cattle,
            animal_present: false,
            presence_frames: 0,
            absence_frames: 0,
            still_frames: 0,
            labored_debounce: 0,
            stillness_alerted: false,
            escape_cooldown: 0,
            frame_count: 0,
            last_bpm: 0.0,
        }
    }

    /// Create with a specific species.
    pub const fn with_species(species: Species) -> Self {
        Self {
            species,
            animal_present: false,
            presence_frames: 0,
            absence_frames: 0,
            still_frames: 0,
            labored_debounce: 0,
            stillness_alerted: false,
            escape_cooldown: 0,
            frame_count: 0,
            last_bpm: 0.0,
        }
    }

    /// Process one frame.
    ///
    /// # Arguments
    /// - `presence`: host-reported presence flag (0/1)
    /// - `breathing_bpm`: host-reported breathing rate
    /// - `motion_energy`: host-reported motion energy
    /// - `variance`: mean CSI variance
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        presence: i32,
        breathing_bpm: f32,
        motion_energy: f32,
        _variance: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        if self.escape_cooldown > 0 {
            self.escape_cooldown -= 1;
        }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        let raw_present = presence > 0 || motion_energy > MIN_MOTION_ACTIVE;

        // --- Step 1: Presence tracking ---
        if raw_present {
            self.presence_frames += 1;
            self.absence_frames = 0;
            if !self.animal_present && self.presence_frames >= 10 {
                self.animal_present = true;
                self.still_frames = 0;
                self.stillness_alerted = false;
            }
        } else {
            self.absence_frames += 1;
            // Only reset presence after sustained absence.
            if self.absence_frames >= ESCAPE_ABSENCE_FRAMES {
                let was_present = self.animal_present;
                let had_enough_presence = self.presence_frames >= MIN_PRESENCE_FOR_ESCAPE;
                self.animal_present = false;

                // Escape alert: was present for a while, then suddenly gone.
                if was_present && had_enough_presence
                    && self.escape_cooldown == 0
                    && n_events < 4
                {
                    self.escape_cooldown = ESCAPE_COOLDOWN;
                    let minutes_present = self.presence_frames as f32 / (20.0 * 60.0);
                    unsafe { EVENTS[n_events] = (EVENT_ESCAPE_ALERT, minutes_present); }
                    n_events += 1;
                }

                self.presence_frames = 0;
            }
        }

        // --- Step 2: Periodic presence report ---
        if self.animal_present
            && self.frame_count % PRESENCE_REPORT_INTERVAL == 0
            && n_events < 4
        {
            unsafe { EVENTS[n_events] = (EVENT_ANIMAL_PRESENT, breathing_bpm); }
            n_events += 1;
        }

        // --- Step 3: Stillness detection (only when animal is present) ---
        if self.animal_present {
            if motion_energy < MIN_MOTION_ACTIVE {
                self.still_frames += 1;
            } else {
                self.still_frames = 0;
                self.stillness_alerted = false;
            }

            if self.still_frames >= STILLNESS_FRAMES
                && !self.stillness_alerted
                && n_events < 4
            {
                self.stillness_alerted = true;
                let minutes_still = self.still_frames as f32 / (20.0 * 60.0);
                unsafe { EVENTS[n_events] = (EVENT_ABNORMAL_STILLNESS, minutes_still); }
                n_events += 1;
            }
        }

        // --- Step 4: Labored breathing detection ---
        if self.animal_present && breathing_bpm > 0.5 {
            self.last_bpm = breathing_bpm;
            let (min_bpm, max_bpm) = self.species.breathing_range();

            // Labored: either too fast or too slow.
            let is_labored = breathing_bpm < min_bpm * 0.7
                || breathing_bpm > max_bpm * 1.3;

            if is_labored {
                self.labored_debounce = self.labored_debounce.saturating_add(1);
                if self.labored_debounce >= LABORED_DEBOUNCE && n_events < 4 {
                    unsafe { EVENTS[n_events] = (EVENT_LABORED_BREATHING, breathing_bpm); }
                    n_events += 1;
                    self.labored_debounce = 0; // Reset to allow repeated alerts.
                }
            } else {
                self.labored_debounce = 0;
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Whether an animal is currently detected.
    pub fn is_animal_present(&self) -> bool {
        self.animal_present
    }

    /// Configured species.
    pub fn species(&self) -> Species {
        self.species
    }

    /// Minutes of stillness (at 20 Hz frame rate).
    pub fn stillness_minutes(&self) -> f32 {
        self.still_frames as f32 / (20.0 * 60.0)
    }

    /// Last observed breathing BPM.
    pub fn last_breathing_bpm(&self) -> f32 {
        self.last_bpm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state() {
        let mon = LivestockMonitor::new();
        assert!(!mon.is_animal_present());
        assert_eq!(mon.frame_count, 0);
        assert!((mon.stillness_minutes() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_species_breathing_ranges() {
        assert_eq!(Species::Cattle.breathing_range(), (12.0, 30.0));
        assert_eq!(Species::Sheep.breathing_range(), (12.0, 20.0));
        assert_eq!(Species::Poultry.breathing_range(), (15.0, 30.0));

        let custom = Species::Custom { min_bpm: 8.0, max_bpm: 25.0 };
        assert_eq!(custom.breathing_range(), (8.0, 25.0));
    }

    #[test]
    fn test_animal_presence_detection() {
        let mut mon = LivestockMonitor::new();

        // Feed presence frames.
        for _ in 0..20 {
            mon.process_frame(1, 20.0, 0.1, 0.05);
        }

        assert!(mon.is_animal_present(), "animal should be detected after sustained presence");
    }

    #[test]
    fn test_labored_breathing_cattle() {
        let mut mon = LivestockMonitor::with_species(Species::Cattle);

        // Establish presence.
        for _ in 0..20 {
            mon.process_frame(1, 20.0, 0.1, 0.05);
        }

        // Feed abnormally high breathing (>30*1.3 = 39 BPM for cattle).
        let mut labored_detected = false;
        for _ in 0..30 {
            let events = mon.process_frame(1, 45.0, 0.1, 0.05);
            for &(et, val) in events {
                if et == EVENT_LABORED_BREATHING {
                    labored_detected = true;
                    assert!((val - 45.0).abs() < 0.01);
                }
            }
        }

        assert!(labored_detected, "labored breathing should be detected for cattle at 45 BPM");
    }

    #[test]
    fn test_normal_breathing_no_alert() {
        let mut mon = LivestockMonitor::with_species(Species::Cattle);

        // Establish presence with normal breathing.
        for _ in 0..100 {
            let events = mon.process_frame(1, 20.0, 0.1, 0.05);
            for &(et, _) in events {
                assert!(et != EVENT_LABORED_BREATHING, "no labored breathing at 20 BPM for cattle");
            }
        }
    }

    #[test]
    fn test_escape_alert() {
        let mut mon = LivestockMonitor::new();

        // Establish strong presence (>MIN_PRESENCE_FOR_ESCAPE frames).
        for _ in 0..250 {
            mon.process_frame(1, 20.0, 0.1, 0.05);
        }
        assert!(mon.is_animal_present());

        // Suddenly no presence.
        let mut escape_detected = false;
        for _ in 0..40 {
            let events = mon.process_frame(0, 0.0, 0.0, 0.001);
            for &(et, _) in events {
                if et == EVENT_ESCAPE_ALERT {
                    escape_detected = true;
                }
            }
        }

        assert!(escape_detected, "escape alert should fire after sudden absence");
    }

    #[test]
    fn test_sheep_low_breathing_labored() {
        let mut mon = LivestockMonitor::with_species(Species::Sheep);

        // Establish presence.
        for _ in 0..20 {
            mon.process_frame(1, 16.0, 0.1, 0.05);
        }

        // Feed very low breathing for sheep (<12*0.7 = 8.4 BPM).
        let mut labored_detected = false;
        for _ in 0..30 {
            let events = mon.process_frame(1, 6.0, 0.1, 0.05);
            for &(et, _) in events {
                if et == EVENT_LABORED_BREATHING {
                    labored_detected = true;
                }
            }
        }

        assert!(labored_detected, "labored breathing should be detected for sheep at 6 BPM");
    }

    #[test]
    fn test_abnormal_stillness() {
        let mut mon = LivestockMonitor::new();

        // Establish presence with motion.
        for _ in 0..20 {
            mon.process_frame(1, 20.0, 0.1, 0.05);
        }

        // Animal present but no motion for a long time.
        let mut stillness_detected = false;
        for _ in 0..6100 {
            // Keep presence via breathing BPM check, but no motion.
            let events = mon.process_frame(1, 18.0, 0.001, 0.05);
            for &(et, _) in events {
                if et == EVENT_ABNORMAL_STILLNESS {
                    stillness_detected = true;
                }
            }
        }

        assert!(stillness_detected, "abnormal stillness should be detected after 5 minutes");
    }
}
