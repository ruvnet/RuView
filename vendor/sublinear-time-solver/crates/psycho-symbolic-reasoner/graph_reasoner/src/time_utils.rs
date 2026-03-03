use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WasmTime {
    #[serde(with = "timestamp_serde")]
    pub timestamp: SystemTime,
}

impl WasmTime {
    pub fn now() -> Self {
        Self {
            timestamp: wasm_time_now(),
        }
    }

    pub fn from_system_time(time: SystemTime) -> Self {
        Self { timestamp: time }
    }
}

impl Default for WasmTime {
    fn default() -> Self {
        Self::now()
    }
}

#[cfg(target_arch = "wasm32")]
fn wasm_time_now() -> SystemTime {
    // For WASM, use a fake timestamp based on performance.now() if available
    // or fall back to UNIX_EPOCH
    use js_sys::Date;
    let millis = Date::now();
    UNIX_EPOCH + Duration::from_millis(millis as u64)
}

#[cfg(not(target_arch = "wasm32"))]
fn wasm_time_now() -> SystemTime {
    SystemTime::now()
}

mod timestamp_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}