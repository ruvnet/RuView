//! `rvf launch` -- Boot RVF in QEMU microVM.

use clap::Args;

#[derive(Args)]
pub struct LaunchArgs {
    /// Path to the RVF store
    pub file: String,
    /// API port to forward from the microVM
    #[arg(short, long, default_value = "8080")]
    pub port: u16,
    /// Memory allocation in MB
    #[arg(short, long, default_value = "128")]
    pub memory_mb: u32,
    /// Number of virtual CPUs
    #[arg(long, default_value = "1")]
    pub vcpus: u32,
    /// SSH port to forward (optional)
    #[arg(long)]
    pub ssh_port: Option<u16>,
    /// Disable KVM acceleration (use TCG instead)
    #[arg(long)]
    pub no_kvm: bool,
    /// Override QEMU binary path
    #[arg(long)]
    pub qemu_binary: Option<String>,
    /// Override kernel image path (skip extraction from RVF)
    #[arg(long)]
    pub kernel: Option<String>,
    /// Override initramfs path
    #[arg(long)]
    pub initramfs: Option<String>,
    /// Extra arguments to pass to QEMU
    #[arg(long, num_args = 1..)]
    pub qemu_args: Vec<String>,
}

#[cfg(feature = "launch")]
pub fn run(args: LaunchArgs) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;
    use std::time::Duration;

    let config = rvf_launch::LaunchConfig {
        rvf_path: PathBuf::from(&args.file),
        memory_mb: args.memory_mb,
        vcpus: args.vcpus,
        api_port: args.port,
        ssh_port: args.ssh_port,
        enable_kvm: !args.no_kvm,
        qemu_binary: args.qemu_binary.map(PathBuf::from),
        extra_args: args.qemu_args,
        kernel_path: args.kernel.map(PathBuf::from),
        initramfs_path: args.initramfs.map(PathBuf::from),
    };

    eprintln!("Launching microVM from {}...", args.file);
    eprintln!("  Memory:   {} MiB", config.memory_mb);
    eprintln!("  vCPUs:    {}", config.vcpus);
    eprintln!("  API port: {}", config.api_port);
    if let Some(ssh) = config.ssh_port {
        eprintln!("  SSH port: {}", ssh);
    }
    eprintln!(
        "  KVM:      {}",
        if config.enable_kvm {
            "enabled (if available)"
        } else {
            "disabled"
        }
    );

    let mut vm = rvf_launch::Launcher::launch(&config)?;
    eprintln!("MicroVM started (PID {})", vm.pid());

    eprintln!("Waiting for VM to become ready (timeout: 30s)...");
    match vm.wait_ready(Duration::from_secs(30)) {
        Ok(()) => {
            eprintln!("VM ready.");
            eprintln!("  API: http://127.0.0.1:{}", args.port);
        }
        Err(e) => {
            eprintln!("Warning: VM did not become ready: {e}");
            eprintln!("The VM may still be booting. Check the console output.");
        }
    }

    eprintln!("Press Ctrl+C to stop the VM.");

    // Wait for Ctrl+C
    let (tx, rx) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .map_err(|e| format!("failed to set Ctrl+C handler: {e}"))?;

    rx.recv()
        .map_err(|e| format!("signal channel error: {e}"))?;

    eprintln!("\nShutting down VM...");
    vm.shutdown()?;
    eprintln!("VM stopped.");

    Ok(())
}

#[cfg(not(feature = "launch"))]
pub fn run(_args: LaunchArgs) -> Result<(), Box<dyn std::error::Error>> {
    Err("QEMU launcher requires the 'launch' feature. \
         Rebuild with: cargo build -p rvf-cli --features launch"
        .into())
}
