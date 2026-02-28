//! QEMU command-line builder.
//!
//! Constructs the `qemu-system-*` command line for launching a microVM
//! from an extracted RVF kernel.

use std::path::{Path, PathBuf};
use std::process::Command;

use rvf_types::kernel::{KernelArch, KERNEL_FLAG_REQUIRES_KVM};

use crate::error::LaunchError;
use crate::extract::ExtractedKernel;
use crate::LaunchConfig;

/// Resolved QEMU invocation ready to be spawned.
pub struct QemuCommand {
    pub command: Command,
    pub qmp_socket: PathBuf,
}

/// Check if KVM is available on this host.
pub fn kvm_available() -> bool {
    Path::new("/dev/kvm").exists()
        && std::fs::metadata("/dev/kvm")
            .map(|m| {
                use std::os::unix::fs::PermissionsExt;
                let mode = m.permissions().mode();
                // Check if the file is readable+writable by someone
                mode & 0o666 != 0
            })
            .unwrap_or(false)
}

/// Locate the QEMU binary for the given architecture.
pub fn find_qemu(arch: KernelArch) -> Result<PathBuf, LaunchError> {
    let candidates = match arch {
        KernelArch::X86_64 | KernelArch::Universal | KernelArch::Unknown => {
            vec![
                "qemu-system-x86_64",
                "/usr/bin/qemu-system-x86_64",
                "/usr/local/bin/qemu-system-x86_64",
            ]
        }
        KernelArch::Aarch64 => {
            vec![
                "qemu-system-aarch64",
                "/usr/bin/qemu-system-aarch64",
                "/usr/local/bin/qemu-system-aarch64",
            ]
        }
        KernelArch::Riscv64 => {
            vec![
                "qemu-system-riscv64",
                "/usr/bin/qemu-system-riscv64",
                "/usr/local/bin/qemu-system-riscv64",
            ]
        }
    };

    for candidate in &candidates {
        if let Ok(output) = std::process::Command::new("which").arg(candidate).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(PathBuf::from(path));
            }
        }
        // Also check if the path exists directly (absolute paths)
        let p = Path::new(candidate);
        if p.is_absolute() && p.exists() {
            return Ok(p.to_path_buf());
        }
    }

    Err(LaunchError::QemuNotFound {
        searched: candidates.iter().map(|s| s.to_string()).collect(),
    })
}

/// Build a QEMU command for the given config and extracted kernel.
pub fn build_command(
    config: &LaunchConfig,
    extracted: &ExtractedKernel,
    work_dir: &Path,
) -> Result<QemuCommand, LaunchError> {
    let arch = KernelArch::try_from(extracted.header.arch).unwrap_or(KernelArch::X86_64);

    // Resolve QEMU binary
    let qemu_bin = match &config.qemu_binary {
        Some(p) => {
            if !p.exists() {
                return Err(LaunchError::QemuNotFound {
                    searched: vec![p.display().to_string()],
                });
            }
            p.clone()
        }
        None => find_qemu(arch)?,
    };

    // KVM
    let use_kvm = if config.enable_kvm {
        if kvm_available() {
            true
        } else if extracted.header.kernel_flags & KERNEL_FLAG_REQUIRES_KVM != 0 {
            return Err(LaunchError::KvmRequired);
        } else {
            false
        }
    } else {
        false
    };

    let qmp_socket = work_dir.join("qmp.sock");

    let mut cmd = Command::new(&qemu_bin);

    // Machine type
    match arch {
        KernelArch::X86_64 | KernelArch::Universal | KernelArch::Unknown => {
            if use_kvm {
                cmd.args(["-machine", "microvm,accel=kvm"]);
                cmd.args(["-cpu", "host"]);
            } else {
                cmd.args(["-machine", "microvm,accel=tcg"]);
                cmd.args(["-cpu", "qemu64"]);
            }
        }
        KernelArch::Aarch64 => {
            if use_kvm {
                cmd.args(["-machine", "virt,accel=kvm"]);
                cmd.args(["-cpu", "host"]);
            } else {
                cmd.args(["-machine", "virt,accel=tcg"]);
                cmd.args(["-cpu", "cortex-a72"]);
            }
        }
        KernelArch::Riscv64 => {
            if use_kvm {
                cmd.args(["-machine", "virt,accel=kvm"]);
                cmd.args(["-cpu", "host"]);
            } else {
                cmd.args(["-machine", "virt,accel=tcg"]);
                cmd.args(["-cpu", "rv64"]);
            }
        }
    }

    // Memory and CPUs
    cmd.arg("-m").arg(format!("{}M", config.memory_mb));
    cmd.arg("-smp").arg(config.vcpus.to_string());

    // Kernel image
    let kernel_path = config
        .kernel_path
        .as_deref()
        .unwrap_or(&extracted.kernel_path);
    cmd.arg("-kernel").arg(kernel_path);

    // Initramfs
    let initramfs = config
        .initramfs_path
        .as_deref()
        .or(extracted.initramfs_path.as_deref());
    if let Some(initrd) = initramfs {
        cmd.arg("-initrd").arg(initrd);
    }

    // Kernel command line
    let default_cmdline = format!(
        "console=ttyS0 reboot=t panic=-1 rvf.port={}",
        config.api_port
    );
    let cmdline = if extracted.cmdline.is_empty() {
        default_cmdline
    } else {
        format!("{} {}", extracted.cmdline, default_cmdline)
    };
    cmd.arg("-append").arg(&cmdline);

    // RVF file as a virtio-blk device (read-only)
    cmd.arg("-drive").arg(format!(
        "id=rvf,file={},format=raw,if=none,readonly=on",
        config.rvf_path.display()
    ));
    cmd.args(["-device", "virtio-blk-device,drive=rvf"]);

    // Network: forward API port and optional SSH port
    let mut hostfwd = format!("user,id=net0,hostfwd=tcp::{}:-:8080", config.api_port);
    if let Some(ssh_port) = config.ssh_port {
        hostfwd.push_str(&format!(",hostfwd=tcp::{}:-:2222", ssh_port));
    }
    cmd.arg("-netdev").arg(&hostfwd);
    cmd.args(["-device", "virtio-net-device,netdev=net0"]);

    // Serial console on stdio
    cmd.args(["-chardev", "stdio,id=char0"]);
    cmd.args(["-serial", "chardev:char0"]);

    // QMP socket for management
    cmd.arg("-qmp")
        .arg(format!("unix:{},server,nowait", qmp_socket.display()));

    // No graphics, no reboot on panic
    cmd.arg("-nographic");
    cmd.arg("-no-reboot");

    // Extra user-specified arguments
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    Ok(QemuCommand {
        command: cmd,
        qmp_socket,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kvm_detection_does_not_panic() {
        // Just ensure the function runs without panicking.
        let _ = kvm_available();
    }

    #[test]
    fn find_qemu_returns_result() {
        // In CI, QEMU may not be installed, so we just check it returns
        // either Ok or a proper error.
        let result = find_qemu(KernelArch::X86_64);
        match result {
            Ok(path) => assert!(path.to_str().unwrap().contains("qemu")),
            Err(LaunchError::QemuNotFound { searched }) => {
                assert!(!searched.is_empty());
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
}
