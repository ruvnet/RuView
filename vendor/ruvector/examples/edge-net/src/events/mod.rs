//! Lifecycle events, Easter eggs, and network celebrations
//!
//! Special events that bring joy to the network - subtle surprises
//! embedded in the system's lifecycle, commemorating milestones
//! and spreading positivity across the distributed compute mesh.

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Network lifecycle events and Easter eggs manager
#[wasm_bindgen]
pub struct NetworkEvents {
    /// Current time (for testing)
    current_time: Option<u64>,
    /// Active events
    active_events: Vec<NetworkEvent>,
    /// Network milestones achieved
    milestones: HashMap<String, u64>,
    /// Hidden discoveries
    discoveries: Vec<Discovery>,
    /// Celebration multiplier boost
    celebration_boost: f32,
}

#[derive(Clone, Serialize, Deserialize)]
struct NetworkEvent {
    id: String,
    name: String,
    description: String,
    bonus_multiplier: f32,
    start_timestamp: u64,
    duration_hours: u32,
    is_secret: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Discovery {
    id: String,
    hint: String,
    discovered: bool,
    discovered_by: Option<String>,
    reward: u64,
}

/// Special dates and their celebrations
const SPECIAL_DATES: &[(u8, u8, &str, &str, f32)] = &[
    // (month, day, name, description, bonus_multiplier)
    (1, 1, "genesis_day", "New beginnings for the network", 2.0),
    (2, 14, "love_compute", "Share the love, share compute", 1.5),
    (3, 14, "pi_day", "Celebrate the mathematical constant", 3.14159),
    (4, 1, "surprise_day", "Expect the unexpected", 1.0),
    (5, 4, "stellar_force", "May the fourth compute with you", 1.4),
    (6, 21, "summer_solstice", "Longest day, maximum contribution", 1.8),
    (7, 20, "moonlanding_day", "One small step for compute", 1.969),
    (10, 31, "spooky_cycles", "Hauntingly good performance", 1.31),
    (11, 11, "binary_day", "11/11 - pure binary celebration", 1.1111),
    (12, 25, "gift_of_compute", "The gift that keeps computing", 2.5),
    (12, 31, "year_end_boost", "Celebrating another year", 1.99),
];

/// Hidden milestone triggers (subtle references)
const MILESTONES: &[(&str, u64, &str, f32)] = &[
    // (milestone_id, threshold, description, reward_multiplier)
    ("first_ruv", 1, "Your first resource utility voucher", 1.5),
    ("century", 100, "A century of contributions", 1.1),
    ("kilo_ruv", 1000, "A thousand vouchers earned", 1.2),
    ("answer", 42, "You found the answer", 4.2),
    ("power_up", 256, "Power of two mastery", 1.256),
    ("golden_ratio", 1618, "Approaching phi", 1.618),
    ("euler", 2718, "Euler would be proud", 2.718),
    ("velocity", 299792, "Speed of light contributor", 3.0),
    ("avogadro", 602214, "Molecular scale achieved", 6.022),
];

#[wasm_bindgen]
impl NetworkEvents {
    #[wasm_bindgen(constructor)]
    pub fn new() -> NetworkEvents {
        NetworkEvents {
            current_time: None,
            active_events: Vec::new(),
            milestones: HashMap::new(),
            discoveries: vec![
                Discovery {
                    id: "resource_origin".to_string(),
                    hint: "The meaning behind rUv runs deep".to_string(),
                    discovered: false,
                    discovered_by: None,
                    reward: 100,
                },
                Discovery {
                    id: "hidden_vector".to_string(),
                    hint: "Vectors point the way".to_string(),
                    discovered: false,
                    discovered_by: None,
                    reward: 50,
                },
                Discovery {
                    id: "quantum_whisper".to_string(),
                    hint: "Some things exist in superposition".to_string(),
                    discovered: false,
                    discovered_by: None,
                    reward: 200,
                },
            ],
            celebration_boost: 1.0,
        }
    }

    /// Set current time (for testing)
    #[wasm_bindgen(js_name = setCurrentTime)]
    pub fn set_current_time(&mut self, timestamp: u64) {
        self.current_time = Some(timestamp);
    }

    /// Get current timestamp
    fn now(&self) -> u64 {
        self.current_time.unwrap_or_else(|| js_sys::Date::now() as u64)
    }

    /// Check for active special events
    #[wasm_bindgen(js_name = checkActiveEvents)]
    pub fn check_active_events(&mut self) -> String {
        let now = self.now();
        let date = js_sys::Date::new(&JsValue::from_f64(now as f64));
        let month = date.get_month() as u8 + 1; // 0-indexed
        let day = date.get_date() as u8;

        self.active_events.clear();
        self.celebration_boost = 1.0;

        for &(m, d, id, desc, bonus) in SPECIAL_DATES {
            if m == month && d == day {
                self.active_events.push(NetworkEvent {
                    id: id.to_string(),
                    name: self.format_event_name(id),
                    description: desc.to_string(),
                    bonus_multiplier: bonus,
                    start_timestamp: now,
                    duration_hours: 24,
                    is_secret: id == "surprise_day",
                });
                self.celebration_boost = self.celebration_boost.max(bonus);
            }
        }

        // Special: Friday the 13th
        if day == 13 && date.get_day() == 5 {
            self.active_events.push(NetworkEvent {
                id: "lucky_friday".to_string(),
                name: "Lucky Friday".to_string(),
                description: "Turn bad luck into good compute".to_string(),
                bonus_multiplier: 1.13,
                start_timestamp: now,
                duration_hours: 24,
                is_secret: true,
            });
        }

        // Build result
        let events_json: Vec<String> = self.active_events.iter()
            .filter(|e| !e.is_secret)
            .map(|e| format!(
                r#"{{"id":"{}","name":"{}","bonus":{:.4}}}"#,
                e.id, e.name, e.bonus_multiplier
            ))
            .collect();

        format!("[{}]", events_json.join(","))
    }

    /// Get celebration multiplier boost
    #[wasm_bindgen(js_name = getCelebrationBoost)]
    pub fn get_celebration_boost(&self) -> f32 {
        self.celebration_boost
    }

    /// Check milestone achievements
    #[wasm_bindgen(js_name = checkMilestones)]
    pub fn check_milestones(&mut self, balance: u64, node_id: &str) -> String {
        let mut newly_achieved = Vec::new();

        for &(id, threshold, desc, reward) in MILESTONES {
            if balance >= threshold && !self.milestones.contains_key(id) {
                self.milestones.insert(id.to_string(), self.now());
                newly_achieved.push((id, desc, reward));
            }
        }

        if newly_achieved.is_empty() {
            return "[]".to_string();
        }

        let json: Vec<String> = newly_achieved.iter()
            .map(|(id, desc, reward)| format!(
                r#"{{"id":"{}","description":"{}","reward":{:.2},"achieved_by":"{}"}}"#,
                id, desc, reward, node_id
            ))
            .collect();

        format!("[{}]", json.join(","))
    }

    /// Get a subtle motivational message
    #[wasm_bindgen(js_name = getMotivation)]
    pub fn get_motivation(&self, balance: u64) -> String {
        let messages = [
            "Every cycle counts in the resource mesh.",
            "Utility flows through the network.",
            "Vectors of contribution align.",
            "Your resources amplify the collective.",
            "The mesh grows stronger with each voucher.",
            "Innovation emerges from distributed effort.",
            "Compute shared is compute multiplied.",
            "The network remembers those who contribute.",
        ];

        // Deterministic selection based on balance
        let idx = (balance % messages.len() as u64) as usize;
        messages[idx].to_string()
    }

    /// Check for discovery triggers (Easter eggs)
    #[wasm_bindgen(js_name = checkDiscovery)]
    pub fn check_discovery(&mut self, action: &str, node_id: &str) -> Option<String> {
        // Subtle discovery triggers
        let discovery = match action {
            // Hidden trigger: reading the source
            "inspect_ruv" | "view_resource_utility" => Some("resource_origin"),
            // Hidden trigger: specific vector operations
            "vector_1618" | "golden_search" => Some("hidden_vector"),
            // Hidden trigger: quantum-related operations
            "superposition" | "entangle" => Some("quantum_whisper"),
            _ => None,
        };

        if let Some(disc_id) = discovery {
            if let Some(disc) = self.discoveries.iter_mut().find(|d| d.id == disc_id && !d.discovered) {
                disc.discovered = true;
                disc.discovered_by = Some(node_id.to_string());
                return Some(format!(
                    r#"{{"discovery":"{}","hint":"{}","reward":{}}}"#,
                    disc.id, disc.hint, disc.reward
                ));
            }
        }

        None
    }

    /// Get network status with thematic flair
    #[wasm_bindgen(js_name = getThemedStatus)]
    pub fn get_themed_status(&self, node_count: u32, total_ruv: u64) -> String {
        let theme = if node_count < 100 {
            ("Genesis Era", "The pioneers forge the network", "seedling")
        } else if node_count < 1000 {
            ("Growth Phase", "Utility spreads across nodes", "sprout")
        } else if node_count < 10000 {
            ("Expansion", "A thriving resource ecosystem", "tree")
        } else if node_count < 100000 {
            ("Maturity", "Self-sustaining compute mesh", "forest")
        } else {
            ("Transcendence", "Beyond individual nodes, unified intelligence", "galaxy")
        };

        format!(
            r#"{{"era":"{}","description":"{}","symbol":"{}","nodes":{},"total_ruv":{}}}"#,
            theme.0, theme.1, theme.2, node_count, total_ruv
        )
    }

    /// Get ASCII art for special occasions
    #[wasm_bindgen(js_name = getSpecialArt)]
    pub fn get_special_art(&self) -> Option<String> {
        if self.active_events.is_empty() {
            return None;
        }

        let event = &self.active_events[0];
        let art = match event.id.as_str() {
            "genesis_day" => Some(r#"
    ╔════════════════════════════════╗
    ║   ★ GENESIS DAY ★              ║
    ║   New beginnings await         ║
    ║   rUv flows through all        ║
    ╚════════════════════════════════╝
"#),
            "pi_day" => Some(r#"
    π═══════════════════════════════π
    ║   3.14159265358979323846...   ║
    ║   Infinite compute ahead      ║
    π═══════════════════════════════π
"#),
            "stellar_force" => Some(r#"
           ★
          ╱ ╲
    ════════════════
    May the compute
    be with you
    ════════════════
"#),
            "binary_day" => Some(r#"
    01100010 01101001 01101110
    ║ 1 + 1 = 10 ║ Pure binary ║
    01100001 01110010 01111001
"#),
            _ => None,
        };

        art.map(String::from)
    }

    fn format_event_name(&self, id: &str) -> String {
        id.chars()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 || id.chars().nth(i - 1) == Some('_') {
                    c.to_uppercase().next().unwrap_or(c)
                } else if c == '_' {
                    ' '
                } else {
                    c
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // Tests requiring WASM environment (uses js_sys::Date)
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_milestone_achievements() {
        let mut events = NetworkEvents::new();

        // First rUv
        let result = events.check_milestones(1, "test-node");
        assert!(result.contains("first_ruv"));

        // Should not trigger again
        let result2 = events.check_milestones(1, "test-node");
        assert_eq!(result2, "[]");

        // Answer to everything
        let result3 = events.check_milestones(42, "test-node");
        assert!(result3.contains("answer"));
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_themed_status() {
        let events = NetworkEvents::new();

        let genesis = events.get_themed_status(50, 1000);
        assert!(genesis.contains("Genesis"));

        let mature = events.get_themed_status(50000, 10000000);
        assert!(mature.contains("Maturity"));
    }
}
