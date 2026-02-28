//! Error Diagnostics with Fix Suggestions
//!
//! Provides helpful error messages and automated fix suggestions
//! for common issues encountered during build, flash, and runtime.

use core::fmt;
use heapless::String;

/// Diagnostic severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Informational message
    Info,
    /// Warning - may cause issues
    Warning,
    /// Error - operation failed
    Error,
    /// Fatal - cannot continue
    Fatal,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "INFO"),
            Severity::Warning => write!(f, "WARN"),
            Severity::Error => write!(f, "ERROR"),
            Severity::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Error category
#[derive(Debug, Clone, Copy)]
pub enum ErrorCategory {
    /// Build/compilation errors
    Build,
    /// Toolchain issues
    Toolchain,
    /// Flash/upload errors
    Flash,
    /// Runtime errors
    Runtime,
    /// Memory issues
    Memory,
    /// Network/WiFi errors
    Network,
    /// Hardware issues
    Hardware,
}

/// Diagnostic result with fix suggestions
#[derive(Clone)]
pub struct Diagnostic {
    /// Error code (e.g., "E0001")
    pub code: String<8>,
    /// Severity level
    pub severity: Severity,
    /// Error category
    pub category: ErrorCategory,
    /// Short description
    pub message: String<128>,
    /// Detailed explanation
    pub explanation: String<256>,
    /// Suggested fixes
    pub fixes: heapless::Vec<String<128>, 4>,
    /// Related documentation link
    pub docs_url: Option<String<128>>,
}

impl Diagnostic {
    /// Create new diagnostic
    pub fn new(code: &str, severity: Severity, category: ErrorCategory, message: &str) -> Self {
        Self {
            code: String::try_from(code).unwrap_or_default(),
            severity,
            category,
            message: String::try_from(message).unwrap_or_default(),
            explanation: String::new(),
            fixes: heapless::Vec::new(),
            docs_url: None,
        }
    }

    /// Add explanation
    pub fn with_explanation(mut self, explanation: &str) -> Self {
        self.explanation = String::try_from(explanation).unwrap_or_default();
        self
    }

    /// Add fix suggestion
    pub fn with_fix(mut self, fix: &str) -> Self {
        let _ = self.fixes.push(String::try_from(fix).unwrap_or_default());
        self
    }

    /// Add documentation URL
    pub fn with_docs(mut self, url: &str) -> Self {
        self.docs_url = Some(String::try_from(url).unwrap_or_default());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n[{}] {}: {}", self.code, self.severity, self.message)?;

        if !self.explanation.is_empty() {
            writeln!(f, "\n  {}", self.explanation)?;
        }

        if !self.fixes.is_empty() {
            writeln!(f, "\n  Suggested fixes:")?;
            for (i, fix) in self.fixes.iter().enumerate() {
                writeln!(f, "    {}. {}", i + 1, fix)?;
            }
        }

        if let Some(url) = &self.docs_url {
            writeln!(f, "\n  Documentation: {}", url)?;
        }

        Ok(())
    }
}

/// Known error patterns and their diagnostics
pub fn diagnose_error(error_text: &str) -> Option<Diagnostic> {
    // Toolchain errors
    if error_text.contains("espup") && error_text.contains("not found") {
        return Some(
            Diagnostic::new("T0001", Severity::Error, ErrorCategory::Toolchain, "ESP toolchain not installed")
                .with_explanation("The ESP32 Rust toolchain (espup) is not installed or not in PATH.")
                .with_fix("Run: npx ruvllm-esp32 install")
                .with_fix("Or manually: cargo install espup && espup install")
                .with_fix("Then restart your terminal or run: source ~/export-esp.sh")
                .with_docs("https://esp-rs.github.io/book/installation/")
        );
    }

    if error_text.contains("LIBCLANG_PATH") {
        return Some(
            Diagnostic::new("T0002", Severity::Error, ErrorCategory::Toolchain, "LIBCLANG_PATH not set")
                .with_explanation("The LIBCLANG_PATH environment variable is not set or points to an invalid location.")
                .with_fix("Windows: Run .\\scripts\\windows\\env.ps1")
                .with_fix("Linux/Mac: source ~/export-esp.sh")
                .with_fix("Or set manually: export LIBCLANG_PATH=/path/to/libclang")
        );
    }

    if error_text.contains("ldproxy") && error_text.contains("not found") {
        return Some(
            Diagnostic::new("T0003", Severity::Error, ErrorCategory::Toolchain, "ldproxy not installed")
                .with_explanation("The ldproxy linker wrapper is required for ESP32 builds.")
                .with_fix("Run: cargo install ldproxy")
        );
    }

    // Flash errors
    if error_text.contains("Permission denied") && error_text.contains("/dev/tty") {
        return Some(
            Diagnostic::new("F0001", Severity::Error, ErrorCategory::Flash, "Serial port permission denied")
                .with_explanation("Your user does not have permission to access the serial port.")
                .with_fix("Add user to dialout group: sudo usermod -a -G dialout $USER")
                .with_fix("Then log out and log back in")
                .with_fix("Or use sudo (not recommended): sudo espflash flash ...")
        );
    }

    if error_text.contains("No such file or directory") && error_text.contains("/dev/tty") {
        return Some(
            Diagnostic::new("F0002", Severity::Error, ErrorCategory::Flash, "Serial port not found")
                .with_explanation("The specified serial port does not exist. The ESP32 may not be connected.")
                .with_fix("Check USB connection")
                .with_fix("Try a different USB cable (data cable, not charge-only)")
                .with_fix("Install USB-to-serial drivers if needed")
                .with_fix("Run 'ls /dev/tty*' to find available ports")
        );
    }

    if error_text.contains("A]fatal error occurred: Failed to connect") {
        return Some(
            Diagnostic::new("F0003", Severity::Error, ErrorCategory::Flash, "Failed to connect to ESP32")
                .with_explanation("Could not establish connection with the ESP32 bootloader.")
                .with_fix("Hold BOOT button while connecting")
                .with_fix("Try pressing RESET while holding BOOT")
                .with_fix("Check that the correct port is selected")
                .with_fix("Try a lower baud rate: --baud 115200")
        );
    }

    // Memory errors
    if error_text.contains("out of memory") || error_text.contains("alloc") {
        return Some(
            Diagnostic::new("M0001", Severity::Error, ErrorCategory::Memory, "Out of memory")
                .with_explanation("The device ran out of RAM during operation.")
                .with_fix("Use a smaller model (e.g., nanoembed-500k)")
                .with_fix("Reduce max_seq_len in config")
                .with_fix("Enable binary quantization for 32x compression")
                .with_fix("Use ESP32-S3 for more SRAM (512KB)")
        );
    }

    if error_text.contains("stack overflow") {
        return Some(
            Diagnostic::new("M0002", Severity::Fatal, ErrorCategory::Memory, "Stack overflow")
                .with_explanation("The call stack exceeded its allocated size.")
                .with_fix("Increase stack size in sdkconfig")
                .with_fix("Reduce recursion depth in your code")
                .with_fix("Move large arrays to heap allocation")
        );
    }

    // Build errors
    if error_text.contains("error[E0433]") && error_text.contains("esp_idf") {
        return Some(
            Diagnostic::new("B0001", Severity::Error, ErrorCategory::Build, "ESP-IDF crate not found")
                .with_explanation("The esp-idf-* crates are not available for your target.")
                .with_fix("Ensure you're using the ESP toolchain: rustup default esp")
                .with_fix("Check that esp feature is enabled in Cargo.toml")
                .with_fix("Run: source ~/export-esp.sh")
        );
    }

    if error_text.contains("target may not be installed") {
        return Some(
            Diagnostic::new("B0002", Severity::Error, ErrorCategory::Build, "Target not installed")
                .with_explanation("The Rust target for your ESP32 variant is not installed.")
                .with_fix("Run: espup install")
                .with_fix("Or: rustup target add <target>")
        );
    }

    // Network errors
    if error_text.contains("WiFi") && error_text.contains("connect") {
        return Some(
            Diagnostic::new("N0001", Severity::Error, ErrorCategory::Network, "WiFi connection failed")
                .with_explanation("Could not connect to the WiFi network.")
                .with_fix("Check SSID and password")
                .with_fix("Ensure the network is 2.4GHz (ESP32 doesn't support 5GHz)")
                .with_fix("Move closer to the access point")
                .with_fix("Check that the network is not hidden")
        );
    }

    None
}

/// Check system for common issues
pub fn run_diagnostics() -> heapless::Vec<Diagnostic, 8> {
    let mut issues = heapless::Vec::new();

    // These would be actual checks in a real implementation
    // Here we just show the structure

    // Check available memory
    // In real impl: check heap_caps_get_free_size()

    // Check flash size
    // In real impl: check partition table

    // Check WiFi status
    // In real impl: check esp_wifi_get_mode()

    issues
}

/// Print diagnostic in colored format (for terminals)
pub fn format_diagnostic_colored(diag: &Diagnostic) -> String<512> {
    let mut output = String::new();

    let color = match diag.severity {
        Severity::Info => "\x1b[36m",    // Cyan
        Severity::Warning => "\x1b[33m", // Yellow
        Severity::Error => "\x1b[31m",   // Red
        Severity::Fatal => "\x1b[35m",   // Magenta
    };
    let reset = "\x1b[0m";

    let _ = core::fmt::write(
        &mut output,
        format_args!("\n{}[{}]{} {}: {}\n", color, diag.code, reset, diag.severity, diag.message)
    );

    if !diag.explanation.is_empty() {
        let _ = core::fmt::write(&mut output, format_args!("\n  {}\n", diag.explanation));
    }

    if !diag.fixes.is_empty() {
        let _ = output.push_str("\n  \x1b[32mSuggested fixes:\x1b[0m\n");
        for (i, fix) in diag.fixes.iter().enumerate() {
            let _ = core::fmt::write(&mut output, format_args!("    {}. {}\n", i + 1, fix));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnose_toolchain_error() {
        let error = "error: espup: command not found";
        let diag = diagnose_error(error);
        assert!(diag.is_some());
        assert_eq!(diag.unwrap().code.as_str(), "T0001");
    }

    #[test]
    fn test_diagnose_flash_error() {
        let error = "Permission denied: /dev/ttyUSB0";
        let diag = diagnose_error(error);
        assert!(diag.is_some());
        assert_eq!(diag.unwrap().code.as_str(), "F0001");
    }

    #[test]
    fn test_diagnose_memory_error() {
        let error = "panicked at 'alloc error'";
        let diag = diagnose_error(error);
        assert!(diag.is_some());
        assert_eq!(diag.unwrap().code.as_str(), "M0001");
    }
}
