# rvf-launch

QEMU microVM launcher for RVF cognitive containers.

## What It Does

`rvf-launch` boots an `.rvf` file as a standalone Linux microservice inside a QEMU microVM. It extracts the embedded kernel from `KERNEL_SEG`, launches QEMU with virtio I/O, and provides a programmatic API for lifecycle management.

## Features

| Feature | Description |
|---------|-------------|
| **Launcher** | Full QEMU process management with automatic cleanup |
| **KVM/TCG detection** | Auto-selects KVM acceleration when available, falls back to TCG |
| **QMP protocol** | Real QEMU Machine Protocol client for graceful shutdown |
| **Kernel extraction** | Reads KERNEL_SEG from `.rvf` files for boot |
| **Port forwarding** | Automatic virtio-net with API and optional SSH ports |

## Usage

```rust
use rvf_launch::{Launcher, LaunchConfig};
use std::time::Duration;

let config = LaunchConfig {
    rvf_path: "appliance.rvf".into(),
    memory_mb: 128,
    vcpus: 2,
    api_port: 8080,
    ssh_port: Some(2222),
    enable_kvm: true,
    ..Default::default()
};

let mut vm = Launcher::launch(&config)?;
vm.wait_ready(Duration::from_secs(30))?;

// Query vectors through the VM
let results = vm.query(&query_vector, 10)?;

// Graceful shutdown via QMP
vm.shutdown()?;
```

## Requirements

- QEMU installed (`qemu-system-x86_64` or `qemu-system-aarch64`)
- KVM optional (falls back to TCG emulation)

## Tests

```bash
cargo test -p rvf-launch  # 8 tests
```

## License

MIT OR Apache-2.0
