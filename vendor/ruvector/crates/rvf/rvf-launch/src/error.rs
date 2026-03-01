//! Error types for the RVF launcher.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// All errors that the launcher can produce.
#[derive(Debug)]
pub enum LaunchError {
    /// QEMU binary not found on the system.
    QemuNotFound { searched: Vec<String> },
    /// KVM is required but not available.
    KvmRequired,
    /// The RVF file does not contain a KERNEL_SEG.
    NoKernelSegment { path: PathBuf },
    /// Failed to extract kernel from the RVF file.
    KernelExtraction(String),
    /// Failed to create a temporary file for the extracted kernel.
    TempFile(io::Error),
    /// QEMU process failed to start.
    QemuSpawn(io::Error),
    /// QEMU process exited with a non-zero code.
    QemuExited { code: Option<i32>, stderr: String },
    /// Timeout waiting for the VM to become ready.
    Timeout { seconds: u64 },
    /// QMP protocol error.
    Qmp(String),
    /// I/O error communicating with QMP socket.
    QmpIo(io::Error),
    /// Port is already in use.
    PortInUse { port: u16 },
    /// The VM process has already exited.
    VmNotRunning,
    /// Generic I/O error.
    Io(io::Error),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QemuNotFound { searched } => {
                write!(f, "QEMU not found; searched: {}", searched.join(", "))
            }
            Self::KvmRequired => {
                write!(
                    f,
                    "KVM is required by kernel flags but /dev/kvm is not accessible"
                )
            }
            Self::NoKernelSegment { path } => {
                write!(f, "no KERNEL_SEG found in {}", path.display())
            }
            Self::KernelExtraction(msg) => write!(f, "kernel extraction failed: {msg}"),
            Self::TempFile(e) => write!(f, "failed to create temp file: {e}"),
            Self::QemuSpawn(e) => write!(f, "failed to spawn QEMU: {e}"),
            Self::QemuExited { code, stderr } => {
                write!(f, "QEMU exited with code {code:?}")?;
                if !stderr.is_empty() {
                    write!(f, ": {stderr}")?;
                }
                Ok(())
            }
            Self::Timeout { seconds } => {
                write!(f, "VM did not become ready within {seconds}s")
            }
            Self::Qmp(msg) => write!(f, "QMP error: {msg}"),
            Self::QmpIo(e) => write!(f, "QMP I/O error: {e}"),
            Self::PortInUse { port } => write!(f, "port {port} is already in use"),
            Self::VmNotRunning => write!(f, "VM process is not running"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TempFile(e) | Self::QemuSpawn(e) | Self::QmpIo(e) | Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for LaunchError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
