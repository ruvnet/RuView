//! Shared output formatting helpers.

use serde::Serialize;

/// Print a value as pretty-printed JSON.
pub fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

/// Print a key-value pair with aligned formatting.
pub fn print_kv(key: &str, value: &str) {
    println!("  {:<20} {}", key, value);
}

/// Format a byte array as a hex string.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
