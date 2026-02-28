//! Error types for the rvf-kernel crate.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Errors that can occur during kernel building, verification, or embedding.
#[derive(Debug)]
pub enum KernelError {
    /// I/O error reading or writing kernel artifacts.
    Io(io::Error),
    /// The file at the given path is not a valid kernel image.
    InvalidImage { path: PathBuf, reason: String },
    /// The kernel image is too small to contain required headers.
    ImageTooSmall { size: u64, min_size: u64 },
    /// SHA3-256 hash of extracted kernel does not match the stored hash.
    HashMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    /// Docker is not available or the build failed.
    DockerBuildFailed(String),
    /// The initramfs archive could not be built.
    InitramfsBuildFailed(String),
    /// Compression or decompression failed.
    CompressionFailed(String),
    /// A required configuration option is missing.
    MissingConfig(String),
    /// The kernel config string is invalid or missing required options.
    InvalidConfig(String),
    /// No KERNEL_SEG found in the RVF store.
    NoKernelSegment,
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "kernel I/O error: {e}"),
            Self::InvalidImage { path, reason } => {
                write!(f, "invalid kernel image at {}: {reason}", path.display())
            }
            Self::ImageTooSmall { size, min_size } => {
                write!(
                    f,
                    "kernel image too small: {size} bytes (minimum {min_size})"
                )
            }
            Self::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "kernel hash mismatch: expected {}..., got {}...",
                    hex_prefix(expected),
                    hex_prefix(actual)
                )
            }
            Self::DockerBuildFailed(msg) => write!(f, "Docker kernel build failed: {msg}"),
            Self::InitramfsBuildFailed(msg) => write!(f, "initramfs build failed: {msg}"),
            Self::CompressionFailed(msg) => write!(f, "compression failed: {msg}"),
            Self::MissingConfig(msg) => write!(f, "missing kernel config: {msg}"),
            Self::InvalidConfig(msg) => write!(f, "invalid kernel config: {msg}"),
            Self::NoKernelSegment => write!(f, "no KERNEL_SEG found in RVF store"),
        }
    }
}

impl std::error::Error for KernelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for KernelError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

fn hex_prefix(data: &[u8; 32]) -> String {
    data.iter().take(4).map(|b| format!("{b:02x}")).collect()
}
