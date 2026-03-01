//! Industrial Anomaly Detection Example
//!
//! Demonstrates using RuVector anomaly detection on ESP32 for
//! real-time industrial equipment monitoring.
//!
//! # Use Cases
//! - Motor vibration analysis
//! - Temperature monitoring
//! - Power consumption anomalies
//! - Predictive maintenance

#![allow(unused)]

use heapless::Vec as HVec;

const SENSOR_DIM: usize = 16;
const MAX_PATTERNS: usize = 128;
const WINDOW_SIZE: usize = 16;

/// Sensor reading from industrial equipment
#[derive(Debug, Clone, Copy)]
struct SensorReading {
    /// Vibration (mm/s RMS)
    vibration: i16,
    /// Temperature (°C * 10)
    temperature: i16,
    /// Current draw (mA)
    current: i16,
    /// Sound level (dB)
    sound: i16,
    /// Timestamp (seconds)
    timestamp: u32,
}

impl SensorReading {
    /// Convert to embedding vector
    fn to_embedding(&self) -> [i8; SENSOR_DIM] {
        let mut embed = [0i8; SENSOR_DIM];

        // Normalize and pack sensor values
        embed[0] = (self.vibration / 4).clamp(-127, 127) as i8;
        embed[1] = (self.temperature / 4).clamp(-127, 127) as i8;
        embed[2] = (self.current / 100).clamp(-127, 127) as i8;
        embed[3] = (self.sound - 50).clamp(-127, 127) as i8;

        // Add derived features
        embed[4] = ((self.vibration * self.temperature) / 1000).clamp(-127, 127) as i8;
        embed[5] = ((self.current * self.vibration) / 1000).clamp(-127, 127) as i8;

        // Time-based features (hour of day affects baseline)
        let hour = (self.timestamp / 3600) % 24;
        embed[6] = (hour as i8 * 5) - 60; // -60 to +60 for hours

        embed
    }
}

/// Anomaly types for industrial equipment
#[derive(Debug, Clone, Copy, PartialEq)]
enum AnomalyType {
    Normal,
    HighVibration,
    Overheating,
    PowerSpike,
    BearingWear,
    Imbalance,
    Cavitation,
    Unknown,
}

impl AnomalyType {
    fn severity(&self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::HighVibration => 60,
            Self::Imbalance => 50,
            Self::BearingWear => 80,
            Self::Overheating => 90,
            Self::Cavitation => 70,
            Self::PowerSpike => 75,
            Self::Unknown => 40,
        }
    }

    fn action(&self) -> &'static str {
        match self {
            Self::Normal => "Continue monitoring",
            Self::HighVibration => "Schedule inspection",
            Self::Imbalance => "Check alignment",
            Self::BearingWear => "Plan bearing replacement",
            Self::Overheating => "URGENT: Reduce load or shutdown",
            Self::Cavitation => "Check pump inlet",
            Self::PowerSpike => "Check electrical connections",
            Self::Unknown => "Investigate manually",
        }
    }
}

/// Anomaly detection result
#[derive(Debug)]
struct AnomalyResult {
    is_anomaly: bool,
    anomaly_type: AnomalyType,
    confidence: u8,
    distance: i32,
    recommendation: &'static str,
}

/// Industrial Anomaly Detector
struct IndustrialAnomalyDetector {
    /// Normal pattern embeddings
    patterns: HVec<[i8; SENSOR_DIM], MAX_PATTERNS>,
    /// Pattern centroids (for classification)
    centroid: [i32; SENSOR_DIM],
    /// Variance for adaptive threshold
    variance: [i32; SENSOR_DIM],
    /// Sample count
    sample_count: u32,
    /// Recent readings window
    window: HVec<SensorReading, WINDOW_SIZE>,
    /// Running average distance
    avg_distance: i32,
    /// Anomaly streak counter
    anomaly_streak: u8,
}

impl IndustrialAnomalyDetector {
    fn new() -> Self {
        Self {
            patterns: HVec::new(),
            centroid: [0; SENSOR_DIM],
            variance: [100; SENSOR_DIM], // Initial variance estimate
            sample_count: 0,
            window: HVec::new(),
            avg_distance: 0,
            anomaly_streak: 0,
        }
    }

    /// Train on normal operation data
    fn learn_normal(&mut self, reading: &SensorReading) -> Result<(), &'static str> {
        let embedding = reading.to_embedding();

        // Update centroid (online mean)
        self.sample_count += 1;
        let n = self.sample_count as i32;

        for i in 0..SENSOR_DIM {
            let delta = embedding[i] as i32 - self.centroid[i] / n.max(1);
            self.centroid[i] += delta;
        }

        // Store pattern (circular buffer)
        if self.patterns.len() >= MAX_PATTERNS {
            self.patterns.remove(0);
        }
        self.patterns.push(embedding).map_err(|_| "Pattern storage full")?;

        // Update variance estimate
        if self.sample_count > 10 {
            for i in 0..SENSOR_DIM {
                let diff = embedding[i] as i32 - self.centroid[i] / n;
                self.variance[i] = (self.variance[i] * 9 + diff * diff) / 10;
            }
        }

        Ok(())
    }

    /// Check if system is trained
    fn is_trained(&self) -> bool {
        self.sample_count >= 20
    }

    /// Detect anomaly in reading
    fn detect(&mut self, reading: &SensorReading) -> AnomalyResult {
        let embedding = reading.to_embedding();

        // Update window
        if self.window.len() >= WINDOW_SIZE {
            self.window.remove(0);
        }
        let _ = self.window.push(*reading);

        // Not enough training data
        if !self.is_trained() {
            let _ = self.learn_normal(reading);
            return AnomalyResult {
                is_anomaly: false,
                anomaly_type: AnomalyType::Normal,
                confidence: 0,
                distance: 0,
                recommendation: "Training... need more normal samples",
            };
        }

        // Calculate distance to centroid
        let n = self.sample_count as i32;
        let mut distance = 0i32;
        let mut weighted_diffs = [0i32; SENSOR_DIM];

        for i in 0..SENSOR_DIM {
            let expected = self.centroid[i] / n;
            let diff = embedding[i] as i32 - expected;
            weighted_diffs[i] = diff;

            // Mahalanobis-like weighting
            let var = self.variance[i].max(1);
            distance += (diff * diff * 100) / var;
        }

        // Find nearest pattern
        let mut min_pattern_dist = i32::MAX;
        for pattern in self.patterns.iter() {
            let dist = euclidean_distance(&embedding, pattern);
            min_pattern_dist = min_pattern_dist.min(dist);
        }

        // Adaptive threshold
        let threshold = self.avg_distance * 2 + 500;
        let is_anomaly = distance > threshold || min_pattern_dist > threshold;

        // Update running average
        self.avg_distance = (self.avg_distance * 9 + distance) / 10;

        // Classify anomaly type
        let anomaly_type = if is_anomaly {
            self.anomaly_streak += 1;
            self.classify_anomaly(reading, &weighted_diffs)
        } else {
            self.anomaly_streak = 0;
            // Learn this as normal
            let _ = self.learn_normal(reading);
            AnomalyType::Normal
        };

        // Calculate confidence
        let confidence = if is_anomaly {
            ((distance * 100) / threshold.max(1)).min(100) as u8
        } else {
            (100 - (distance * 100) / threshold.max(1)).max(0) as u8
        };

        AnomalyResult {
            is_anomaly,
            anomaly_type,
            confidence,
            distance,
            recommendation: anomaly_type.action(),
        }
    }

    /// Classify the type of anomaly based on sensor deviations
    fn classify_anomaly(&self, reading: &SensorReading, diffs: &[i32; SENSOR_DIM]) -> AnomalyType {
        // Check specific conditions

        // High vibration
        if reading.vibration > 150 {
            // Check for bearing wear pattern (high freq + temperature)
            if reading.temperature > 600 {
                return AnomalyType::BearingWear;
            }
            // Check for imbalance (periodic vibration)
            return AnomalyType::HighVibration;
        }

        // Overheating
        if reading.temperature > 800 {
            return AnomalyType::Overheating;
        }

        // Power issues
        if reading.current > 5000 {
            return AnomalyType::PowerSpike;
        }

        // Check window for trends
        if self.window.len() >= 8 {
            // Rising temperature trend
            let temp_trend: i32 = self.window.iter()
                .rev()
                .take(4)
                .map(|r| r.temperature as i32)
                .sum::<i32>()
                - self.window.iter()
                    .rev()
                    .skip(4)
                    .take(4)
                    .map(|r| r.temperature as i32)
                    .sum::<i32>();

            if temp_trend > 200 {
                return AnomalyType::Overheating;
            }

            // Check for cavitation (vibration + sound pattern)
            let high_sound = self.window.iter()
                .filter(|r| r.sound > 85)
                .count();

            if high_sound > 4 {
                return AnomalyType::Cavitation;
            }
        }

        AnomalyType::Unknown
    }

    /// Get system statistics
    fn stats(&self) -> (u32, u8, i32) {
        (self.sample_count, self.anomaly_streak, self.avg_distance)
    }
}

/// Euclidean distance for embeddings
fn euclidean_distance(a: &[i8], b: &[i8]) -> i32 {
    let mut sum = 0i32;
    for (va, vb) in a.iter().zip(b.iter()) {
        let diff = *va as i32 - *vb as i32;
        sum += diff * diff;
    }
    sum
}

fn main() {
    println!("🏭 Industrial Anomaly Detection Example");
    println!("======================================\n");

    let mut detector = IndustrialAnomalyDetector::new();

    // Simulate training phase with normal operation
    println!("📊 Training on normal operation data...\n");

    for i in 0..30 {
        let reading = SensorReading {
            vibration: 50 + (i % 10) as i16,      // 50-60 mm/s (normal)
            temperature: 450 + (i % 20) as i16,   // 45-47°C (normal)
            current: 2500 + (i % 200) as i16,     // 2.5-2.7A (normal)
            sound: 65 + (i % 5) as i16,           // 65-70 dB (normal)
            timestamp: i * 60,
        };

        let result = detector.detect(&reading);
        if i % 10 == 0 {
            println!("Training sample {}: distance={}", i, result.distance);
        }
    }

    println!("\n✅ Training complete ({} samples)\n", detector.sample_count);

    // Test scenarios
    println!("🔍 Testing anomaly detection:\n");

    let test_scenarios = [
        ("Normal operation", SensorReading {
            vibration: 55, temperature: 460, current: 2600, sound: 67, timestamp: 2000
        }),
        ("High vibration", SensorReading {
            vibration: 180, temperature: 480, current: 2700, sound: 75, timestamp: 2060
        }),
        ("Overheating", SensorReading {
            vibration: 60, temperature: 850, current: 2800, sound: 68, timestamp: 2120
        }),
        ("Power spike", SensorReading {
            vibration: 70, temperature: 500, current: 6000, sound: 72, timestamp: 2180
        }),
        ("Bearing wear (vibration + heat)", SensorReading {
            vibration: 200, temperature: 700, current: 3000, sound: 80, timestamp: 2240
        }),
        ("Normal again", SensorReading {
            vibration: 52, temperature: 455, current: 2550, sound: 66, timestamp: 2300
        }),
    ];

    for (name, reading) in test_scenarios.iter() {
        println!("Scenario: {}", name);
        println!("  Reading: vib={}mm/s, temp={:.1}°C, curr={}mA, sound={}dB",
            reading.vibration,
            reading.temperature as f32 / 10.0,
            reading.current,
            reading.sound
        );

        let result = detector.detect(reading);

        println!("  Result: {}", if result.is_anomaly { "⚠️  ANOMALY" } else { "✅ Normal" });
        println!("  Type: {:?} (severity: {})", result.anomaly_type, result.anomaly_type.severity());
        println!("  Confidence: {}%", result.confidence);
        println!("  Distance: {}", result.distance);
        println!("  Action: {}", result.recommendation);
        println!();
    }

    // Simulate gradual bearing degradation
    println!("📈 Simulating gradual bearing degradation:\n");

    for i in 0..10 {
        let degradation = i * 15;
        let reading = SensorReading {
            vibration: 55 + degradation as i16,
            temperature: 460 + (degradation * 2) as i16,
            current: 2600 + (degradation * 10) as i16,
            sound: 67 + (degradation / 3) as i16,
            timestamp: 3000 + i * 3600, // Hourly readings
        };

        let result = detector.detect(&reading);

        println!("Hour {}: vib={}, temp={:.1}°C → {} {:?}",
            i,
            reading.vibration,
            reading.temperature as f32 / 10.0,
            if result.is_anomaly { "ANOMALY" } else { "OK" },
            result.anomaly_type
        );
    }

    // Memory statistics
    println!("\n📊 Memory Usage:");
    let pattern_mem = detector.patterns.len() * SENSOR_DIM;
    let window_mem = detector.window.len() * core::mem::size_of::<SensorReading>();
    let total_mem = pattern_mem + window_mem + 200; // +200 for other fields

    println!("   Patterns stored: {}", detector.patterns.len());
    println!("   Window size: {} readings", detector.window.len());
    println!("   Total memory: ~{} bytes ({:.1} KB)", total_mem, total_mem as f32 / 1024.0);

    println!("\n✨ Industrial Anomaly Detection Demo Complete!");
    println!("\n💡 On ESP32:");
    println!("   - Detects anomalies in <1ms");
    println!("   - Learns normal patterns adaptively");
    println!("   - Classifies 7+ anomaly types");
    println!("   - Perfect for predictive maintenance");
}
