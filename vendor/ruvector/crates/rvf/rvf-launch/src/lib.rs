//! QEMU microVM launcher for RVF computational containers.
//!
//! This crate extracts a kernel image from an RVF file's KERNEL_SEG,
//! builds a QEMU command line, launches the VM, and provides a handle
//! for management (query, shutdown, kill) via QMP.

pub mod error;
pub mod extract;
pub mod qemu;
pub mod qmp;

use std::io::Read;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use rvf_types::kernel::KernelArch;

pub use error::LaunchError;

/// Configuration for launching an RVF microVM.
#[derive(Clone, Debug)]
pub struct LaunchConfig {
    /// Path to the RVF store file.
    pub rvf_path: PathBuf,
    /// Memory allocation in MiB.
    pub memory_mb: u32,
    /// Number of virtual CPUs.
    pub vcpus: u32,
    /// Host port to forward to the VM's API port (guest :8080).
    pub api_port: u16,
    /// Optional host port to forward to the VM's SSH port (guest :2222).
    pub ssh_port: Option<u16>,
    /// Whether to enable KVM acceleration (falls back to TCG if unavailable
    /// unless the kernel requires KVM).
    pub enable_kvm: bool,
    /// Override the QEMU binary path.
    pub qemu_binary: Option<PathBuf>,
    /// Extra arguments to pass to QEMU.
    pub extra_args: Vec<String>,
    /// Override the kernel image path (skip extraction from RVF).
    pub kernel_path: Option<PathBuf>,
    /// Override the initramfs path.
    pub initramfs_path: Option<PathBuf>,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            rvf_path: PathBuf::new(),
            memory_mb: 128,
            vcpus: 1,
            api_port: 8080,
            ssh_port: None,
            enable_kvm: true,
            qemu_binary: None,
            extra_args: Vec::new(),
            kernel_path: None,
            initramfs_path: None,
        }
    }
}

/// Current status of the microVM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmStatus {
    /// QEMU process is running.
    Running,
    /// QEMU process has exited.
    Exited(Option<i32>),
}

/// A running QEMU microVM.
pub struct MicroVm {
    process: Child,
    api_port: u16,
    ssh_port: Option<u16>,
    qmp_socket: PathBuf,
    pid: u32,
    /// Holds the extracted kernel temp files alive.
    _extracted: Option<extract::ExtractedKernel>,
    /// Holds the work directory alive.
    _workdir: tempfile::TempDir,
}

/// Result of a requirements check.
#[derive(Clone, Debug)]
pub struct RequirementsReport {
    /// Whether qemu-system-x86_64 (or arch equivalent) was found.
    pub qemu_found: bool,
    /// Path to the QEMU binary, if found.
    pub qemu_path: Option<PathBuf>,
    /// Whether KVM acceleration is available.
    pub kvm_available: bool,
    /// Platform-specific install instructions if QEMU is missing.
    pub install_hint: String,
}

impl std::fmt::Display for RequirementsReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.qemu_found {
            writeln!(
                f,
                "QEMU: found at {}",
                self.qemu_path.as_ref().unwrap().display()
            )?;
        } else {
            writeln!(f, "QEMU: NOT FOUND")?;
            writeln!(f, "  Install instructions:")?;
            writeln!(f, "  {}", self.install_hint)?;
        }
        writeln!(
            f,
            "KVM:  {}",
            if self.kvm_available {
                "available"
            } else {
                "not available (will use TCG)"
            }
        )
    }
}

/// Description of what a launch would execute, without spawning QEMU.
#[derive(Clone, Debug)]
pub struct DryRunResult {
    /// The full QEMU command line that would be executed.
    pub command_line: Vec<String>,
    /// Path to the kernel image that would be used.
    pub kernel_path: PathBuf,
    /// Path to the initramfs, if any.
    pub initramfs_path: Option<PathBuf>,
    /// The kernel command line that would be passed.
    pub cmdline: String,
    /// Whether KVM would be used.
    pub use_kvm: bool,
    /// Memory allocation in MiB.
    pub memory_mb: u32,
    /// Number of virtual CPUs.
    pub vcpus: u32,
    /// The API port mapping.
    pub api_port: u16,
}

impl std::fmt::Display for DryRunResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Dry run - QEMU command that would be executed:")?;
        writeln!(f, "  {}", self.command_line.join(" "))?;
        writeln!(f, "")?;
        writeln!(f, "  Kernel:    {}", self.kernel_path.display())?;
        if let Some(ref initrd) = self.initramfs_path {
            writeln!(f, "  Initramfs: {}", initrd.display())?;
        }
        writeln!(f, "  Cmdline:   {}", self.cmdline)?;
        writeln!(
            f,
            "  KVM:       {}",
            if self.use_kvm { "yes" } else { "no (TCG)" }
        )?;
        writeln!(f, "  Memory:    {} MiB", self.memory_mb)?;
        writeln!(f, "  vCPUs:     {}", self.vcpus)?;
        writeln!(f, "  API port:  {}", self.api_port)
    }
}

/// Top-level launcher API.
pub struct Launcher;

impl Launcher {
    /// Check whether all requirements for launching a microVM are met.
    ///
    /// Returns a `RequirementsReport` with details about what was found
    /// and platform-specific install instructions if QEMU is missing.
    pub fn check_requirements(arch: KernelArch) -> RequirementsReport {
        let qemu_result = qemu::find_qemu(arch);
        let kvm = qemu::kvm_available();

        let install_hint = match std::env::consts::OS {
            "linux" => {
                // Detect package manager
                if std::path::Path::new("/usr/bin/apt").exists()
                    || std::path::Path::new("/usr/bin/apt-get").exists()
                {
                    "sudo apt install qemu-system-x86".to_string()
                } else if std::path::Path::new("/usr/bin/dnf").exists() {
                    "sudo dnf install qemu-system-x86".to_string()
                } else if std::path::Path::new("/usr/bin/pacman").exists() {
                    "sudo pacman -S qemu-system-x86".to_string()
                } else if std::path::Path::new("/sbin/apk").exists() {
                    "sudo apk add qemu-system-x86_64".to_string()
                } else {
                    "Install QEMU via your distribution's package manager \
                     (e.g. apt, dnf, pacman)"
                        .to_string()
                }
            }
            "macos" => "brew install qemu".to_string(),
            _ => "Download QEMU from https://www.qemu.org/download/".to_string(),
        };

        match qemu_result {
            Ok(path) => RequirementsReport {
                qemu_found: true,
                qemu_path: Some(path),
                kvm_available: kvm,
                install_hint,
            },
            Err(_) => RequirementsReport {
                qemu_found: false,
                qemu_path: None,
                kvm_available: kvm,
                install_hint,
            },
        }
    }

    /// Extract kernel from an RVF file and launch it in a QEMU microVM.
    ///
    /// Calls `check_requirements()` first and returns a helpful error if
    /// QEMU is not found.
    pub fn launch(config: &LaunchConfig) -> Result<MicroVm, LaunchError> {
        if !config.rvf_path.exists() {
            return Err(LaunchError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("RVF file not found: {}", config.rvf_path.display()),
            )));
        }

        // Check requirements first (unless user provided a custom binary)
        if config.qemu_binary.is_none() {
            let report = Self::check_requirements(KernelArch::X86_64);
            if !report.qemu_found {
                return Err(LaunchError::QemuNotFound {
                    searched: vec![format!(
                        "QEMU not found. Install it with: {}",
                        report.install_hint,
                    )],
                });
            }
        }

        // Extract kernel from RVF
        let extracted = extract::extract_kernel(&config.rvf_path)?;

        // Create a working directory for QMP socket, logs, etc.
        let workdir = tempfile::tempdir().map_err(LaunchError::TempFile)?;

        // Build the QEMU command
        let qemu_cmd = qemu::build_command(config, &extracted, workdir.path())?;

        let qmp_socket = qemu_cmd.qmp_socket.clone();

        // Spawn QEMU
        let mut command = qemu_cmd.command;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command.spawn().map_err(LaunchError::QemuSpawn)?;

        let pid = child.id();

        Ok(MicroVm {
            process: child,
            api_port: config.api_port,
            ssh_port: config.ssh_port,
            qmp_socket,
            pid,
            _extracted: Some(extracted),
            _workdir: workdir,
        })
    }

    /// Show what WOULD be executed without actually spawning QEMU.
    ///
    /// Useful for CI/testing and debugging launch configuration. Extracts
    /// the kernel from the RVF file and builds the full command line, but
    /// does not spawn any process.
    pub fn dry_run(config: &LaunchConfig) -> Result<DryRunResult, LaunchError> {
        if !config.rvf_path.exists() {
            return Err(LaunchError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("RVF file not found: {}", config.rvf_path.display()),
            )));
        }

        let extracted = extract::extract_kernel(&config.rvf_path)?;
        let workdir = tempfile::tempdir().map_err(LaunchError::TempFile)?;
        let qemu_cmd = qemu::build_command(config, &extracted, workdir.path())?;

        // Reconstruct the command line as a Vec<String>
        let cmd = &qemu_cmd.command;
        let program = cmd.get_program().to_string_lossy().to_string();
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        let mut command_line = vec![program];
        command_line.extend(args);

        let kernel_path = config
            .kernel_path
            .clone()
            .unwrap_or_else(|| extracted.kernel_path.clone());

        let initramfs_path = config
            .initramfs_path
            .clone()
            .or_else(|| extracted.initramfs_path.clone());

        let use_kvm = config.enable_kvm && qemu::kvm_available();

        Ok(DryRunResult {
            command_line,
            kernel_path,
            initramfs_path,
            cmdline: extracted.cmdline,
            use_kvm,
            memory_mb: config.memory_mb,
            vcpus: config.vcpus,
            api_port: config.api_port,
        })
    }

    /// Find the QEMU binary for the given architecture.
    pub fn find_qemu(arch: KernelArch) -> Result<PathBuf, LaunchError> {
        qemu::find_qemu(arch)
    }

    /// Check if KVM is available on this host.
    pub fn kvm_available() -> bool {
        qemu::kvm_available()
    }
}

impl MicroVm {
    /// Wait for the VM's API port to accept TCP connections.
    pub fn wait_ready(&mut self, timeout: Duration) -> Result<(), LaunchError> {
        let start = Instant::now();
        let addr = format!("127.0.0.1:{}", self.api_port);

        loop {
            // Check if the process has exited
            if let Some(exit) = self.try_wait_process()? {
                let mut stderr_buf = String::new();
                if let Some(ref mut stderr) = self.process.stderr {
                    let _ = stderr.read_to_string(&mut stderr_buf);
                }
                return Err(LaunchError::QemuExited {
                    code: exit,
                    stderr: stderr_buf,
                });
            }

            // Try connecting to the API port
            if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(200))
                .is_ok()
            {
                return Ok(());
            }

            if start.elapsed() >= timeout {
                return Err(LaunchError::Timeout {
                    seconds: timeout.as_secs(),
                });
            }

            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// Send a vector query to the running VM's HTTP API.
    pub fn query(
        &self,
        vector: &[f32],
        k: usize,
    ) -> Result<Vec<rvf_runtime::SearchResult>, LaunchError> {
        let _url = format!("http://127.0.0.1:{}/query", self.api_port);

        // Build JSON payload
        let payload = serde_json::json!({
            "vector": vector,
            "k": k,
        });
        let body =
            serde_json::to_vec(&payload).map_err(|e| LaunchError::Io(std::io::Error::other(e)))?;

        // Use a raw TCP connection to send an HTTP POST (avoids depending
        // on a full HTTP client library).
        let addr = format!("127.0.0.1:{}", self.api_port);
        let mut stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))
            .map_err(LaunchError::Io)?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(LaunchError::Io)?;

        use std::io::Write;
        let request = format!(
            "POST /query HTTP/1.1\r\n\
             Host: 127.0.0.1:{}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n",
            self.api_port,
            body.len(),
        );
        stream
            .write_all(request.as_bytes())
            .map_err(LaunchError::Io)?;
        stream.write_all(&body).map_err(LaunchError::Io)?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(LaunchError::Io)?;

        // Parse the HTTP response body (skip headers)
        let body_start = response.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
        let resp_body = &response[body_start..];

        #[derive(serde::Deserialize)]
        struct QueryResult {
            id: u64,
            distance: f32,
        }

        let results: Vec<QueryResult> = serde_json::from_str(resp_body).map_err(|e| {
            LaunchError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;

        Ok(results
            .into_iter()
            .map(|r| rvf_runtime::SearchResult {
                id: r.id,
                distance: r.distance,
                retrieval_quality: rvf_types::quality::RetrievalQuality::Full,
            })
            .collect())
    }

    /// Get the current VM status.
    pub fn status(&mut self) -> VmStatus {
        match self.process.try_wait() {
            Ok(Some(status)) => VmStatus::Exited(status.code()),
            Ok(None) => VmStatus::Running,
            Err(_) => VmStatus::Exited(None),
        }
    }

    /// Graceful shutdown: try QMP `system_powerdown`, fall back to SIGTERM.
    pub fn shutdown(&mut self) -> Result<(), LaunchError> {
        // Try QMP first
        if self.qmp_socket.exists() {
            match qmp::QmpClient::connect(&self.qmp_socket, Duration::from_secs(5)) {
                Ok(mut client) => {
                    let _ = client.system_powerdown();

                    // Wait up to 10 seconds for the VM to shut down
                    let start = Instant::now();
                    while start.elapsed() < Duration::from_secs(10) {
                        if let Ok(Some(_)) = self.process.try_wait() {
                            return Ok(());
                        }
                        std::thread::sleep(Duration::from_millis(200));
                    }

                    // Still running, try quit
                    let _ = client.quit();
                    let start = Instant::now();
                    while start.elapsed() < Duration::from_secs(5) {
                        if let Ok(Some(_)) = self.process.try_wait() {
                            return Ok(());
                        }
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
                Err(_) => {
                    // QMP not available, fall through to SIGTERM
                }
            }
        }

        // Fall back to SIGTERM (via kill on Unix)
        #[cfg(unix)]
        {
            unsafe {
                libc_kill(self.pid as i32);
            }
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                if let Ok(Some(_)) = self.process.try_wait() {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }

        // Last resort: kill -9
        let _ = self.process.kill();
        let _ = self.process.wait();
        Ok(())
    }

    /// Force-kill the VM process immediately.
    pub fn kill(&mut self) -> Result<(), LaunchError> {
        self.process.kill().map_err(LaunchError::Io)?;
        let _ = self.process.wait();
        Ok(())
    }

    /// Get the QEMU process PID.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the API port.
    pub fn api_port(&self) -> u16 {
        self.api_port
    }

    /// Get the SSH port, if configured.
    pub fn ssh_port(&self) -> Option<u16> {
        self.ssh_port
    }

    /// Get the QMP socket path.
    pub fn qmp_socket(&self) -> &PathBuf {
        &self.qmp_socket
    }

    fn try_wait_process(&mut self) -> Result<Option<Option<i32>>, LaunchError> {
        match self.process.try_wait() {
            Ok(Some(status)) => Ok(Some(status.code())),
            Ok(None) => Ok(None),
            Err(e) => Err(LaunchError::Io(e)),
        }
    }
}

impl Drop for MicroVm {
    fn drop(&mut self) {
        // Best-effort cleanup: try to kill the process if still running.
        if let Ok(None) = self.process.try_wait() {
            let _ = self.process.kill();
            let _ = self.process.wait();
        }
    }
}

/// Send SIGTERM on Unix. Avoids a libc dependency by using a raw syscall.
#[cfg(unix)]
unsafe fn libc_kill(pid: i32) {
    // SIGTERM = 15 on all Unix platforms
    // We use std::process::Command as a portable way to send signals.
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = LaunchConfig::default();
        assert_eq!(config.memory_mb, 128);
        assert_eq!(config.vcpus, 1);
        assert_eq!(config.api_port, 8080);
        assert!(config.enable_kvm);
    }

    #[test]
    fn vm_status_variants() {
        assert_eq!(VmStatus::Running, VmStatus::Running);
        assert_eq!(VmStatus::Exited(Some(0)), VmStatus::Exited(Some(0)));
        assert_ne!(VmStatus::Running, VmStatus::Exited(None));
    }

    #[test]
    fn check_requirements_returns_report() {
        let report = Launcher::check_requirements(KernelArch::X86_64);
        // Install hint should never be empty
        assert!(!report.install_hint.is_empty());
        // Display formatting should work
        let display = format!("{report}");
        assert!(display.contains("QEMU:"));
        assert!(display.contains("KVM:"));

        if report.qemu_found {
            assert!(report.qemu_path.is_some());
        } else {
            assert!(report.qemu_path.is_none());
        }
    }

    #[test]
    fn check_requirements_has_platform_install_hint() {
        let report = Launcher::check_requirements(KernelArch::X86_64);
        // On Linux CI we expect an apt/dnf/pacman hint
        #[cfg(target_os = "linux")]
        {
            assert!(
                report.install_hint.contains("apt")
                    || report.install_hint.contains("dnf")
                    || report.install_hint.contains("pacman")
                    || report.install_hint.contains("apk")
                    || report.install_hint.contains("package manager"),
                "expected Linux install hint, got: {}",
                report.install_hint,
            );
        }
    }

    #[test]
    fn launch_rejects_missing_rvf() {
        let config = LaunchConfig {
            rvf_path: PathBuf::from("/nonexistent/test.rvf"),
            ..Default::default()
        };
        let result = Launcher::launch(&config);
        assert!(result.is_err());
    }

    #[test]
    fn dry_run_rejects_missing_rvf() {
        let config = LaunchConfig {
            rvf_path: PathBuf::from("/nonexistent/test.rvf"),
            ..Default::default()
        };
        let result = Launcher::dry_run(&config);
        assert!(result.is_err());
    }

    #[test]
    fn dry_run_with_real_rvf() {
        use rvf_runtime::options::RvfOptions;
        use rvf_runtime::RvfStore;

        let dir = tempfile::tempdir().unwrap();
        let rvf_path = dir.path().join("dry_run.rvf");

        let opts = RvfOptions {
            dimension: 4,
            ..Default::default()
        };
        let mut store = RvfStore::create(&rvf_path, opts).unwrap();
        let image = b"MZ\x00fake-kernel-for-dry-run-test";
        store
            .embed_kernel(
                KernelArch::X86_64 as u8,
                0x01,
                0,
                image,
                8080,
                Some("console=ttyS0"),
            )
            .unwrap();
        store.close().unwrap();

        let config = LaunchConfig {
            rvf_path: rvf_path.clone(),
            memory_mb: 256,
            vcpus: 2,
            api_port: 9090,
            ..Default::default()
        };

        let result = Launcher::dry_run(&config);
        // dry_run may fail if QEMU binary not found - that is expected
        match result {
            Ok(dry) => {
                assert!(!dry.command_line.is_empty());
                assert!(dry.command_line[0].contains("qemu"));
                assert_eq!(dry.memory_mb, 256);
                assert_eq!(dry.vcpus, 2);
                assert_eq!(dry.api_port, 9090);
                assert_eq!(dry.cmdline, "console=ttyS0");
                // Display should work
                let display = format!("{dry}");
                assert!(display.contains("Dry run"));
                assert!(display.contains("256 MiB"));
            }
            Err(LaunchError::QemuNotFound { .. }) => {
                // Expected in environments without QEMU
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn requirements_report_display() {
        let report = RequirementsReport {
            qemu_found: true,
            qemu_path: Some(PathBuf::from("/usr/bin/qemu-system-x86_64")),
            kvm_available: false,
            install_hint: "sudo apt install qemu-system-x86".to_string(),
        };
        let s = format!("{report}");
        assert!(s.contains("/usr/bin/qemu-system-x86_64"));
        assert!(s.contains("not available"));

        let report_missing = RequirementsReport {
            qemu_found: false,
            qemu_path: None,
            kvm_available: false,
            install_hint: "brew install qemu".to_string(),
        };
        let s2 = format!("{report_missing}");
        assert!(s2.contains("NOT FOUND"));
        assert!(s2.contains("brew install qemu"));
    }
}
