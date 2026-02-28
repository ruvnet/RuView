//! # Tier 3: Synthetic Nervous Systems for Environments
//!
//! Buildings, factories, cities.
//!
//! ## What Changes
//! - Infrastructure becomes a sensing fabric
//! - Reflexes manage local events
//! - Policy emerges from patterns, not rules
//!
//! ## Why This Matters
//! - Environments respond like organisms
//! - Energy, safety, and flow self-regulate
//! - Central planning gives way to distributed intelligence
//!
//! This is exotic but inevitable.

use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;

/// A location in the environment
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LocationId(pub String);

/// A zone grouping multiple locations
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ZoneId(pub String);

/// Environmental sensor reading
#[derive(Clone, Debug)]
pub struct EnvironmentReading {
    pub timestamp: u64,
    pub location: LocationId,
    pub sensor_type: EnvironmentSensor,
    pub value: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EnvironmentSensor {
    Temperature,
    Humidity,
    Light,
    Occupancy,
    AirQuality,
    Noise,
    Motion,
    Energy,
    Water,
}

/// Environmental actuator command
#[derive(Clone, Debug)]
pub struct EnvironmentAction {
    pub location: LocationId,
    pub actuator: EnvironmentActuator,
    pub value: f32,
    pub priority: u8,
}

#[derive(Clone, Debug)]
pub enum EnvironmentActuator {
    HVAC { mode: HVACMode },
    Lighting { brightness: f32 },
    Ventilation { flow_rate: f32 },
    Shading { position: f32 },
    DoorLock { locked: bool },
    Alarm { active: bool },
}

#[derive(Clone, Debug)]
pub enum HVACMode {
    Off,
    Heating(f32),
    Cooling(f32),
    Ventilation,
}

/// Local reflex for immediate environmental response
pub struct LocalEnvironmentReflex {
    pub location: LocationId,
    pub sensor_type: EnvironmentSensor,
    pub threshold_low: f32,
    pub threshold_high: f32,
    pub action_low: EnvironmentActuator,
    pub action_high: EnvironmentActuator,
    pub hysteresis: f32,
    pub last_state: i8, // -1 = below, 0 = normal, 1 = above
}

impl LocalEnvironmentReflex {
    pub fn new(
        location: LocationId,
        sensor_type: EnvironmentSensor,
        threshold_low: f32,
        threshold_high: f32,
        action_low: EnvironmentActuator,
        action_high: EnvironmentActuator,
    ) -> Self {
        Self {
            location,
            sensor_type,
            threshold_low,
            threshold_high,
            action_low,
            action_high,
            hysteresis: 0.5,
            last_state: 0,
        }
    }

    /// Check if reflex should fire
    pub fn check(&mut self, reading: &EnvironmentReading) -> Option<EnvironmentAction> {
        if reading.location != self.location || reading.sensor_type != self.sensor_type {
            return None;
        }

        // Apply hysteresis
        let effective_low = if self.last_state == -1 {
            self.threshold_low + self.hysteresis
        } else {
            self.threshold_low
        };

        let effective_high = if self.last_state == 1 {
            self.threshold_high - self.hysteresis
        } else {
            self.threshold_high
        };

        if reading.value < effective_low && self.last_state != -1 {
            self.last_state = -1;
            Some(EnvironmentAction {
                location: self.location.clone(),
                actuator: self.action_low.clone(),
                value: reading.value,
                priority: 1,
            })
        } else if reading.value > effective_high && self.last_state != 1 {
            self.last_state = 1;
            Some(EnvironmentAction {
                location: self.location.clone(),
                actuator: self.action_high.clone(),
                value: reading.value,
                priority: 1,
            })
        } else if reading.value >= effective_low && reading.value <= effective_high {
            self.last_state = 0;
            None
        } else {
            None
        }
    }
}

/// Zone-level homeostasis controller
pub struct ZoneHomeostasis {
    pub zone: ZoneId,
    pub locations: Vec<LocationId>,
    pub target_temperature: f32,
    pub target_humidity: f32,
    pub target_light: f32,
    pub adaptation_rate: f32,
    /// Learned occupancy pattern (24 hours)
    pub occupancy_pattern: [f32; 24],
    pub learning_enabled: bool,
}

impl ZoneHomeostasis {
    pub fn new(zone: ZoneId, locations: Vec<LocationId>) -> Self {
        Self {
            zone,
            locations,
            target_temperature: 22.0,
            target_humidity: 50.0,
            target_light: 500.0,
            adaptation_rate: 0.1,
            occupancy_pattern: [0.0; 24],
            learning_enabled: true,
        }
    }

    /// Learn from occupancy patterns
    pub fn learn_occupancy(&mut self, hour: usize, occupancy: f32) {
        if self.learning_enabled && hour < 24 {
            self.occupancy_pattern[hour] = self.occupancy_pattern[hour]
                * (1.0 - self.adaptation_rate)
                + occupancy * self.adaptation_rate;
        }
    }

    /// Predict occupancy for pre-conditioning
    pub fn predict_occupancy(&self, hour: usize) -> f32 {
        if hour < 24 {
            self.occupancy_pattern[hour]
        } else {
            0.0
        }
    }

    /// Compute zone-level action based on aggregate readings
    pub fn compute_action(
        &self,
        readings: &[EnvironmentReading],
        hour: usize,
    ) -> Vec<EnvironmentAction> {
        let mut actions = Vec::new();

        // Filter readings for this zone
        let zone_readings: Vec<_> = readings
            .iter()
            .filter(|r| self.locations.contains(&r.location))
            .collect();

        if zone_readings.is_empty() {
            return actions;
        }

        // Average temperature
        let temp_readings: Vec<_> = zone_readings
            .iter()
            .filter(|r| r.sensor_type == EnvironmentSensor::Temperature)
            .collect();

        if !temp_readings.is_empty() {
            let avg_temp: f32 =
                temp_readings.iter().map(|r| r.value).sum::<f32>() / temp_readings.len() as f32;

            // Adjust target based on predicted occupancy
            let predicted_occ = self.predict_occupancy(hour);
            let effective_target = if predicted_occ > 0.5 {
                self.target_temperature
            } else {
                // Setback when unoccupied
                self.target_temperature - 2.0
            };

            let temp_error = avg_temp - effective_target;

            if temp_error.abs() > 1.0 {
                let mode = if temp_error > 0.0 {
                    HVACMode::Cooling(temp_error.abs().min(5.0))
                } else {
                    HVACMode::Heating(temp_error.abs().min(5.0))
                };

                for loc in &self.locations {
                    actions.push(EnvironmentAction {
                        location: loc.clone(),
                        actuator: EnvironmentActuator::HVAC { mode: mode.clone() },
                        value: temp_error,
                        priority: 2,
                    });
                }
            }
        }

        // Light based on occupancy
        let occupancy_readings: Vec<_> = zone_readings
            .iter()
            .filter(|r| r.sensor_type == EnvironmentSensor::Occupancy)
            .collect();

        if !occupancy_readings.is_empty() {
            let occupied = occupancy_readings.iter().any(|r| r.value > 0.5);

            for loc in &self.locations {
                let brightness = if occupied { 1.0 } else { 0.1 };
                actions.push(EnvironmentAction {
                    location: loc.clone(),
                    actuator: EnvironmentActuator::Lighting { brightness },
                    value: brightness,
                    priority: 3,
                });
            }
        }

        actions
    }
}

/// Global workspace for environment-wide coordination
pub struct EnvironmentWorkspace {
    pub capacity: usize,
    pub items: VecDeque<WorkspaceItem>,
    pub policies: Vec<EmergentPolicy>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceItem {
    pub zone: ZoneId,
    pub observation: String,
    pub salience: f32,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub struct EmergentPolicy {
    pub name: String,
    pub trigger_pattern: String,
    pub action_pattern: String,
    pub confidence: f32,
    pub occurrences: u64,
}

impl EnvironmentWorkspace {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: VecDeque::new(),
            policies: Vec::new(),
        }
    }

    /// Broadcast observation to workspace
    pub fn broadcast(&mut self, item: WorkspaceItem) {
        if self.items.len() >= self.capacity {
            // Remove lowest salience
            if let Some(min_idx) = self
                .items
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.salience.partial_cmp(&b.salience).unwrap())
                .map(|(i, _)| i)
            {
                self.items.remove(min_idx);
            }
        }
        self.items.push_back(item);
    }

    /// Detect emergent patterns
    pub fn detect_patterns(&mut self) -> Option<EmergentPolicy> {
        // Look for repeated sequences in workspace
        let observations: Vec<_> = self.items.iter().map(|i| i.observation.clone()).collect();

        if observations.len() < 3 {
            return None;
        }

        // Simple pattern: if same observation repeats
        let last = observations.last()?;
        let count = observations.iter().filter(|o| *o == last).count();

        if count >= 3 {
            let policy = EmergentPolicy {
                name: format!("Pattern_{}", self.policies.len()),
                trigger_pattern: last.clone(),
                action_pattern: "coordinate_response".to_string(),
                confidence: count as f32 / observations.len() as f32,
                occurrences: 1,
            };

            // Check if already known
            if !self
                .policies
                .iter()
                .any(|p| p.trigger_pattern == last.clone())
            {
                self.policies.push(policy.clone());
                return Some(policy);
            }
        }

        None
    }
}

/// Complete synthetic nervous system for an environment
pub struct SyntheticNervousSystem {
    pub name: String,
    /// Local reflexes (fast, location-specific)
    pub reflexes: Vec<LocalEnvironmentReflex>,
    /// Zone homeostasis (medium, zone-level)
    pub zones: HashMap<ZoneId, ZoneHomeostasis>,
    /// Global workspace (slow, environment-wide)
    pub workspace: EnvironmentWorkspace,
    /// Current time
    pub timestamp: u64,
    /// Action history
    pub action_log: Vec<(u64, EnvironmentAction)>,
}

impl SyntheticNervousSystem {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            reflexes: Vec::new(),
            zones: HashMap::new(),
            workspace: EnvironmentWorkspace::new(7),
            timestamp: 0,
            action_log: Vec::new(),
        }
    }

    /// Add a zone
    pub fn add_zone(&mut self, zone_id: &str, locations: Vec<&str>) {
        let zone = ZoneId(zone_id.to_string());
        let locs: Vec<_> = locations
            .iter()
            .map(|l| LocationId(l.to_string()))
            .collect();

        self.zones
            .insert(zone.clone(), ZoneHomeostasis::new(zone, locs));
    }

    /// Add a local reflex
    pub fn add_reflex(&mut self, reflex: LocalEnvironmentReflex) {
        self.reflexes.push(reflex);
    }

    /// Process sensor readings through the nervous system
    pub fn process(&mut self, readings: Vec<EnvironmentReading>) -> Vec<EnvironmentAction> {
        self.timestamp += 1;
        let hour = ((self.timestamp / 60) % 24) as usize;

        let mut actions = Vec::new();

        // 1. Local reflexes (fastest)
        for reflex in &mut self.reflexes {
            for reading in &readings {
                if let Some(action) = reflex.check(reading) {
                    actions.push(action);
                }
            }
        }

        // If reflexes fired, skip higher levels
        if !actions.is_empty() {
            for action in &actions {
                self.action_log.push((self.timestamp, action.clone()));
            }
            return actions;
        }

        // 2. Zone homeostasis (medium)
        for (_, zone) in &mut self.zones {
            // Learn occupancy
            for reading in &readings {
                if reading.sensor_type == EnvironmentSensor::Occupancy
                    && zone.locations.contains(&reading.location)
                {
                    zone.learn_occupancy(hour, reading.value);
                }
            }

            // Compute zone actions
            let zone_actions = zone.compute_action(&readings, hour);
            actions.extend(zone_actions);
        }

        // 3. Global workspace (slowest, pattern detection)
        for reading in &readings {
            if reading.value > 0.8 || reading.value < 0.2 {
                // Significant observation
                self.workspace.broadcast(WorkspaceItem {
                    zone: ZoneId("global".to_string()),
                    observation: format!(
                        "{:?}_{}",
                        reading.sensor_type,
                        if reading.value > 0.5 { "high" } else { "low" }
                    ),
                    salience: reading.value.abs(),
                    timestamp: self.timestamp,
                });
            }
        }

        // Detect emergent patterns
        if let Some(policy) = self.workspace.detect_patterns() {
            println!(
                "  [EMERGENT] New policy: {} (confidence: {:.2})",
                policy.name, policy.confidence
            );
        }

        for action in &actions {
            self.action_log.push((self.timestamp, action.clone()));
        }

        actions
    }

    /// Get system status
    pub fn status(&self) -> EnvironmentStatus {
        let learned_patterns = self.workspace.policies.len();

        let zone_states: HashMap<_, _> = self
            .zones
            .iter()
            .map(|(id, zone)| {
                (
                    id.clone(),
                    ZoneState {
                        target_temp: zone.target_temperature,
                        occupancy_learned: zone.occupancy_pattern.iter().sum::<f32>() > 0.0,
                    },
                )
            })
            .collect();

        EnvironmentStatus {
            timestamp: self.timestamp,
            active_reflexes: self.reflexes.len(),
            zones: self.zones.len(),
            learned_patterns,
            zone_states,
            recent_actions: self.action_log.len(),
        }
    }
}

#[derive(Debug)]
pub struct EnvironmentStatus {
    pub timestamp: u64,
    pub active_reflexes: usize,
    pub zones: usize,
    pub learned_patterns: usize,
    pub zone_states: HashMap<ZoneId, ZoneState>,
    pub recent_actions: usize,
}

#[derive(Debug)]
pub struct ZoneState {
    pub target_temp: f32,
    pub occupancy_learned: bool,
}

fn main() {
    println!("=== Tier 3: Synthetic Nervous Systems for Environments ===\n");

    let mut building = SyntheticNervousSystem::new("Smart Building");

    // Add zones
    building.add_zone("office_north", vec!["room_101", "room_102", "room_103"]);
    building.add_zone("office_south", vec!["room_201", "room_202"]);
    building.add_zone("lobby", vec!["entrance", "reception"]);

    // Add local reflexes
    building.add_reflex(LocalEnvironmentReflex::new(
        LocationId("room_101".to_string()),
        EnvironmentSensor::Temperature,
        18.0,
        28.0,
        EnvironmentActuator::HVAC {
            mode: HVACMode::Heating(3.0),
        },
        EnvironmentActuator::HVAC {
            mode: HVACMode::Cooling(3.0),
        },
    ));

    building.add_reflex(LocalEnvironmentReflex::new(
        LocationId("entrance".to_string()),
        EnvironmentSensor::Motion,
        0.0,
        0.5,
        EnvironmentActuator::Lighting { brightness: 0.2 },
        EnvironmentActuator::Lighting { brightness: 1.0 },
    ));

    println!("Building initialized:");
    let status = building.status();
    println!("  Zones: {}", status.zones);
    println!("  Active reflexes: {}", status.active_reflexes);

    // Simulate a day
    println!("\nSimulating 24 hours...");
    for hour in 0..24 {
        for minute in 0..60 {
            let timestamp = hour * 60 + minute;

            // Generate readings based on time of day
            let occupied = (hour >= 8 && hour <= 18) && (minute % 5 == 0);
            let temp = 20.0 + 4.0 * ((hour as f32 / 24.0) * PI).sin();

            let readings = vec![
                EnvironmentReading {
                    timestamp,
                    location: LocationId("room_101".to_string()),
                    sensor_type: EnvironmentSensor::Temperature,
                    value: temp,
                },
                EnvironmentReading {
                    timestamp,
                    location: LocationId("room_101".to_string()),
                    sensor_type: EnvironmentSensor::Occupancy,
                    value: if occupied { 1.0 } else { 0.0 },
                },
                EnvironmentReading {
                    timestamp,
                    location: LocationId("entrance".to_string()),
                    sensor_type: EnvironmentSensor::Motion,
                    value: if occupied && minute % 15 == 0 {
                        1.0
                    } else {
                        0.0
                    },
                },
            ];

            let actions = building.process(readings);

            if hour % 4 == 0 && minute == 0 {
                println!(
                    "  Hour {}: {} actions, temp={:.1}°C, occupied={}",
                    hour,
                    actions.len(),
                    temp,
                    occupied
                );
            }
        }
    }

    // Summary
    let status = building.status();
    println!("\n=== End of Day Status ===");
    println!("  Total actions taken: {}", status.recent_actions);
    println!("  Emergent policies learned: {}", status.learned_patterns);
    println!("  Zone states:");
    for (zone, state) in &status.zone_states {
        println!(
            "    {:?}: target={:.1}°C, occupancy_learned={}",
            zone.0, state.target_temp, state.occupancy_learned
        );
    }

    println!("\n=== Key Benefits ===");
    println!("- Infrastructure becomes a sensing fabric");
    println!("- Local reflexes handle immediate events");
    println!("- Zone homeostasis manages comfort autonomously");
    println!("- Policies emerge from patterns, not rules");
    println!("- Energy, safety, and flow self-regulate");
    println!("\nThis is exotic but inevitable.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_reflex() {
        let mut reflex = LocalEnvironmentReflex::new(
            LocationId("test".to_string()),
            EnvironmentSensor::Temperature,
            18.0,
            28.0,
            EnvironmentActuator::HVAC {
                mode: HVACMode::Heating(1.0),
            },
            EnvironmentActuator::HVAC {
                mode: HVACMode::Cooling(1.0),
            },
        );

        // Cold triggers heating
        let reading = EnvironmentReading {
            timestamp: 0,
            location: LocationId("test".to_string()),
            sensor_type: EnvironmentSensor::Temperature,
            value: 15.0,
        };

        let action = reflex.check(&reading);
        assert!(action.is_some());
    }

    #[test]
    fn test_zone_homeostasis() {
        let mut zone = ZoneHomeostasis::new(
            ZoneId("test".to_string()),
            vec![LocationId("room1".to_string())],
        );

        // Learn occupancy pattern
        for _ in 0..10 {
            zone.learn_occupancy(10, 1.0); // 10am occupied
            zone.learn_occupancy(22, 0.0); // 10pm empty
        }

        assert!(zone.predict_occupancy(10) > 0.5);
        assert!(zone.predict_occupancy(22) < 0.5);
    }

    #[test]
    fn test_workspace_patterns() {
        let mut workspace = EnvironmentWorkspace::new(7);

        // Add repeated observation
        for _ in 0..5 {
            workspace.broadcast(WorkspaceItem {
                zone: ZoneId("test".to_string()),
                observation: "Temperature_high".to_string(),
                salience: 1.0,
                timestamp: 0,
            });
        }

        let pattern = workspace.detect_patterns();
        assert!(pattern.is_some());
    }
}
