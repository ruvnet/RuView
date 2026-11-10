//! Source resolution state machine (issue #1004, #2097).

/// Source execution plan resolved at server startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePlan {
    /// The `AppStateInner.source` value to start with.
    pub initial_source: String,
    /// Bind the UDP :5005 receiver (and thus allow simulate→esp32 promotion).
    pub bind_udp: bool,
    /// Run the simulated-data generator (serves poses until a real frame arrives).
    pub run_simulator: bool,
    /// Run the platform WiFi capture task.
    pub run_wifi: bool,
}

/// Every `--source` value [`plan_source`] accepts.
pub const SOURCE_VALUES: [&str; 9] = [
    "auto",
    "esp32",
    "wifi",
    "simulated",
    "simulate",
    "mediatek",
    "qualcomm",
    "realtek",
    "realtek_csi",
];

/// Pure decision function — fully unit-testable without binding sockets.
pub fn plan_source(
    requested: &str,
    esp32_detected: bool,
    wifi_detected: bool,
) -> Result<SourcePlan, String> {
    let plan = match requested {
        "auto" => {
            if esp32_detected {
                SourcePlan {
                    initial_source: "esp32".to_string(),
                    bind_udp: true,
                    run_simulator: false,
                    run_wifi: false,
                }
            } else if wifi_detected {
                SourcePlan {
                    initial_source: "wifi".to_string(),
                    bind_udp: true,
                    run_simulator: false,
                    run_wifi: true,
                }
            } else {
                SourcePlan {
                    initial_source: "simulated".to_string(),
                    bind_udp: true,
                    run_simulator: true,
                    run_wifi: false,
                }
            }
        }
        "simulate" | "simulated" => SourcePlan {
            initial_source: "simulated".to_string(),
            bind_udp: false,
            run_simulator: true,
            run_wifi: false,
        },
        "esp32" => SourcePlan {
            initial_source: "esp32".to_string(),
            bind_udp: true,
            run_simulator: false,
            run_wifi: false,
        },
        "mediatek" => SourcePlan {
            initial_source: "mediatek".to_string(),
            bind_udp: true,
            run_simulator: false,
            run_wifi: false,
        },
        "wifi" => SourcePlan {
            initial_source: "wifi".to_string(),
            bind_udp: false,
            run_simulator: false,
            run_wifi: true,
        },
        "qualcomm" | "realtek" | "realtek_csi" => SourcePlan {
            initial_source: requested.to_string(),
            bind_udp: true,
            run_simulator: false,
            run_wifi: false,
        },
        other => {
            return Err(format!(
                "unknown --source '{other}' (valid values: {})",
                SOURCE_VALUES.join(", ")
            ))
        }
    };
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_with_no_boot_source_still_binds_udp_and_simulates() {
        let plan = plan_source("auto", false, false).expect("valid source");
        assert!(plan.bind_udp);
        assert!(plan.run_simulator);
        assert!(!plan.run_wifi);
        assert_eq!(plan.initial_source, "simulated");
    }

    #[test]
    fn auto_with_esp32_detected_binds_udp_no_simulator() {
        let plan = plan_source("auto", true, false).expect("valid source");
        assert!(plan.bind_udp);
        assert!(!plan.run_simulator);
        assert_eq!(plan.initial_source, "esp32");
    }

    #[test]
    fn auto_with_wifi_detected_runs_wifi_and_still_binds_udp() {
        let plan = plan_source("auto", false, true).expect("valid source");
        assert!(plan.run_wifi);
        assert!(plan.bind_udp);
        assert!(!plan.run_simulator);
        assert_eq!(plan.initial_source, "wifi");
    }

    #[test]
    fn explicit_simulated_is_offline_override_no_udp() {
        for s in ["simulated", "simulate"] {
            let plan = plan_source(s, false, false).expect("valid source");
            assert!(!plan.bind_udp);
            assert!(plan.run_simulator);
            assert_eq!(plan.initial_source, "simulated");
        }
    }

    #[test]
    fn explicit_esp32_binds_udp() {
        let plan = plan_source("esp32", false, false).expect("valid source");
        assert!(plan.bind_udp);
        assert!(!plan.run_simulator);
        assert_eq!(plan.initial_source, "esp32");
    }

    #[test]
    fn explicit_mediatek_binds_udp_no_simulator() {
        let plan = plan_source("mediatek", false, false).expect("valid source");
        assert!(plan.bind_udp);
        assert!(!plan.run_simulator);
        assert!(!plan.run_wifi);
        assert_eq!(plan.initial_source, "mediatek");
    }

    #[test]
    fn plan_source_rejects_unknown_values() {
        for bogus in ["macos", "linux", "bogus", ""] {
            assert!(plan_source(bogus, false, false).is_err());
        }
        for valid in SOURCE_VALUES {
            assert!(plan_source(valid, false, false).is_ok());
        }
    }

    #[test]
    fn effective_source_promotes_from_simulated_to_esp32_on_real_frame() {
        let mut src = "simulated".to_string();
        assert_eq!(promote_view(&src, None), "simulated");
        src = "esp32".to_string();
        let fresh = Some(std::time::Duration::from_millis(10));
        assert_eq!(promote_view(&src, fresh), "esp32", "fresh esp32 frame => live");
        let timeout = std::time::Duration::from_secs(5);
        let stale = Some(timeout + std::time::Duration::from_secs(1));
        assert_eq!(promote_view(&src, stale), "esp32:offline");
    }

    fn promote_view(source: &str, last_frame_age: Option<std::time::Duration>) -> String {
        if source == "esp32" {
            if let Some(age) = last_frame_age {
                if age > std::time::Duration::from_secs(5) {
                    return "esp32:offline".to_string();
                }
            }
        }
        source.to_string()
    }
}
