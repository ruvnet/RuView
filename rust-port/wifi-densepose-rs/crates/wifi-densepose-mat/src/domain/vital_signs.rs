//! Vital signs value objects for survivor detection.

use chrono::{DateTime, Utc};

/// Confidence score for a detection (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfidenceScore(f64);

impl ConfidenceScore {
    /// Create a new confidence score, clamped to [0.0, 1.0]
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Get the raw value
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Check if confidence is high (>= 0.8)
    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }

    /// Check if confidence is medium (>= 0.5)
    pub fn is_medium(&self) -> bool {
        self.0 >= 0.5
    }

    /// Check if confidence is low (< 0.5)
    pub fn is_low(&self) -> bool {
        self.0 < 0.5
    }
}

impl Default for ConfidenceScore {
    fn default() -> Self {
        Self(0.0)
    }
}

/// Complete vital signs reading at a point in time
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VitalSignsReading {
    /// Breathing pattern if detected
    pub breathing: Option<BreathingPattern>,
    /// Heartbeat signature if detected
    pub heartbeat: Option<HeartbeatSignature>,
    /// Movement profile
    pub movement: MovementProfile,
    /// Timestamp of reading
    pub timestamp: DateTime<Utc>,
    /// Overall confidence in the reading
    pub confidence: ConfidenceScore,
}

impl VitalSignsReading {
    /// Create a new vital signs reading
    pub fn new(
        breathing: Option<BreathingPattern>,
        heartbeat: Option<HeartbeatSignature>,
        movement: MovementProfile,
    ) -> Self {
        // Calculate combined confidence
        let confidence = Self::calculate_confidence(&breathing, &heartbeat, &movement);

        Self {
            breathing,
            heartbeat,
            movement,
            timestamp: Utc::now(),
            confidence,
        }
    }

    /// Calculate combined confidence from individual detections
    fn calculate_confidence(
        breathing: &Option<BreathingPattern>,
        heartbeat: &Option<HeartbeatSignature>,
        movement: &MovementProfile,
    ) -> ConfidenceScore {
        let mut total = 0.0;
        let mut count = 0.0;

        if let Some(b) = breathing {
            total += b.confidence();
            count += 1.5; // Weight breathing higher
        }

        if let Some(h) = heartbeat {
            total += h.confidence();
            count += 1.0;
        }

        if movement.movement_type != MovementType::None {
            total += movement.confidence();
            count += 1.0;
        }

        if count > 0.0 {
            ConfidenceScore::new(total / count)
        } else {
            ConfidenceScore::new(0.0)
        }
    }

    /// Check if any vital sign is detected
    pub fn has_vitals(&self) -> bool {
        self.breathing.is_some()
            || self.heartbeat.is_some()
            || self.movement.movement_type != MovementType::None
    }

    /// Check if breathing is detected
    pub fn has_breathing(&self) -> bool {
        self.breathing.is_some()
    }

    /// Check if heartbeat is detected
    pub fn has_heartbeat(&self) -> bool {
        self.heartbeat.is_some()
    }

    /// Check if movement is detected
    pub fn has_movement(&self) -> bool {
        self.movement.movement_type != MovementType::None
    }
}

/// Breathing pattern detected from CSI analysis
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BreathingPattern {
    /// Breaths per minute (normal adult: 12-20)
    pub rate_bpm: f32,
    /// Signal amplitude/strength
    pub amplitude: f32,
    /// Pattern regularity (0.0-1.0)
    pub regularity: f32,
    /// Type of breathing pattern
    pub pattern_type: BreathingType,
}

impl BreathingPattern {
    /// Check if breathing rate is normal
    pub fn is_normal_rate(&self) -> bool {
        self.rate_bpm >= 12.0 && self.rate_bpm <= 20.0
    }

    /// Check if rate is critically low
    pub fn is_bradypnea(&self) -> bool {
        self.rate_bpm < 10.0
    }

    /// Check if rate is critically high
    pub fn is_tachypnea(&self) -> bool {
        self.rate_bpm > 30.0
    }

    /// Get confidence based on signal quality
    pub fn confidence(&self) -> f64 {
        (self.amplitude * self.regularity) as f64
    }
}

/// Types of breathing patterns
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BreathingType {
    /// Normal, regular breathing
    Normal,
    /// Shallow, weak breathing
    Shallow,
    /// Deep, labored breathing
    Labored,
    /// Irregular pattern
    Irregular,
    /// Agonal breathing (pre-death gasping)
    Agonal,
    /// Apnea (no breathing detected)
    Apnea,
}

/// Heartbeat signature from micro-Doppler analysis
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeartbeatSignature {
    /// Heart rate in beats per minute (normal: 60-100)
    pub rate_bpm: f32,
    /// Heart rate variability
    pub variability: f32,
    /// Signal strength
    pub strength: SignalStrength,
}

impl HeartbeatSignature {
    /// Check if heart rate is normal
    pub fn is_normal_rate(&self) -> bool {
        self.rate_bpm >= 60.0 && self.rate_bpm <= 100.0
    }

    /// Check if rate indicates bradycardia
    pub fn is_bradycardia(&self) -> bool {
        self.rate_bpm < 50.0
    }

    /// Check if rate indicates tachycardia
    pub fn is_tachycardia(&self) -> bool {
        self.rate_bpm > 120.0
    }

    /// Get confidence based on signal strength
    pub fn confidence(&self) -> f64 {
        match self.strength {
            SignalStrength::Strong => 0.9,
            SignalStrength::Moderate => 0.7,
            SignalStrength::Weak => 0.4,
            SignalStrength::VeryWeak => 0.2,
        }
    }
}

/// Signal strength levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SignalStrength {
    /// Strong, clear signal
    Strong,
    /// Moderate signal
    Moderate,
    /// Weak signal
    Weak,
    /// Very weak, borderline
    VeryWeak,
}

/// Movement profile from CSI analysis
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MovementProfile {
    /// Type of movement detected
    pub movement_type: MovementType,
    /// Intensity of movement (0.0-1.0)
    pub intensity: f32,
    /// Frequency of movement patterns
    pub frequency: f32,
    /// Whether movement appears voluntary/purposeful
    pub is_voluntary: bool,
}

impl Default for MovementProfile {
    fn default() -> Self {
        Self {
            movement_type: MovementType::None,
            intensity: 0.0,
            frequency: 0.0,
            is_voluntary: false,
        }
    }
}

impl MovementProfile {
    /// Get confidence based on movement characteristics
    pub fn confidence(&self) -> f64 {
        match self.movement_type {
            MovementType::None => 0.0,
            MovementType::Gross => 0.9,
            MovementType::Fine => 0.7,
            MovementType::Tremor => 0.6,
            MovementType::Periodic => 0.5,
        }
    }

    /// Check if movement indicates consciousness
    pub fn indicates_consciousness(&self) -> bool {
        self.is_voluntary && self.movement_type == MovementType::Gross
    }
}

/// Types of movement detected
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MovementType {
    /// No movement detected
    None,
    /// Large body movements (limbs, torso)
    Gross,
    /// Small movements (fingers, head)
    Fine,
    /// Involuntary tremor/shaking
    Tremor,
    /// Periodic movement (possibly breathing-related)
    Periodic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_score_clamping() {
        assert_eq!(ConfidenceScore::new(1.5).value(), 1.0);
        assert_eq!(ConfidenceScore::new(-0.5).value(), 0.0);
        assert_eq!(ConfidenceScore::new(0.7).value(), 0.7);
    }

    #[test]
    fn test_breathing_pattern_rates() {
        let normal = BreathingPattern {
            rate_bpm: 16.0,
            amplitude: 0.8,
            regularity: 0.9,
            pattern_type: BreathingType::Normal,
        };
        assert!(normal.is_normal_rate());
        assert!(!normal.is_bradypnea());
        assert!(!normal.is_tachypnea());

        let slow = BreathingPattern {
            rate_bpm: 8.0,
            amplitude: 0.5,
            regularity: 0.6,
            pattern_type: BreathingType::Shallow,
        };
        assert!(slow.is_bradypnea());

        let fast = BreathingPattern {
            rate_bpm: 35.0,
            amplitude: 0.7,
            regularity: 0.5,
            pattern_type: BreathingType::Labored,
        };
        assert!(fast.is_tachypnea());
    }

    #[test]
    fn test_vital_signs_reading() {
        let breathing = BreathingPattern {
            rate_bpm: 16.0,
            amplitude: 0.8,
            regularity: 0.9,
            pattern_type: BreathingType::Normal,
        };

        let reading = VitalSignsReading::new(
            Some(breathing),
            None,
            MovementProfile::default(),
        );

        assert!(reading.has_vitals());
        assert!(reading.has_breathing());
        assert!(!reading.has_heartbeat());
        assert!(!reading.has_movement());
    }

    #[test]
    fn test_signal_strength_confidence() {
        let strong = HeartbeatSignature {
            rate_bpm: 72.0,
            variability: 0.1,
            strength: SignalStrength::Strong,
        };
        assert_eq!(strong.confidence(), 0.9);

        let weak = HeartbeatSignature {
            rate_bpm: 72.0,
            variability: 0.1,
            strength: SignalStrength::Weak,
        };
        assert_eq!(weak.confidence(), 0.4);
    }
}
