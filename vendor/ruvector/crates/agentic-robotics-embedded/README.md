# agentic-robotics-embedded

[![Crates.io](https://img.shields.io/crates/v/agentic-robotics-embedded.svg)](https://crates.io/crates/agentic-robotics-embedded)
[![Documentation](https://docs.rs/agentic-robotics-embedded/badge.svg)](https://docs.rs/agentic-robotics-embedded)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../../LICENSE)

**Embedded systems support for Agentic Robotics**

Part of the [Agentic Robotics](https://github.com/ruvnet/vibecast) framework - high-performance robotics middleware with ROS2 compatibility.

## Features

- 🔌 **No-std compatible**: Run on bare-metal embedded systems
- ⚡ **RTIC integration**: Real-Time Interrupt-driven Concurrency
- 🚀 **Embassy support**: Modern async/await for embedded
- 💾 **Minimal footprint**: < 50KB code size
- 🎯 **Zero-allocation**: Static memory allocation
- 🔋 **Low power**: Optimized for battery-powered robots

## Installation

```toml
[dependencies]
agentic-robotics-core = { version = "0.1.0", default-features = false }
agentic-robotics-embedded = "0.1.0"
```

## Supported Platforms

| Platform | Status | Framework | Example |
|----------|--------|-----------|---------|
| **STM32** | ✅ Supported | RTIC, Embassy | STM32F4, STM32H7 |
| **ESP32** | ✅ Supported | Embassy | ESP32-C3, ESP32-S3 |
| **nRF** | ✅ Supported | Embassy | nRF52, nRF53 |
| **RP2040** | ✅ Supported | Embassy | Raspberry Pi Pico |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Documentation**: [docs.rs/agentic-robotics-embedded](https://docs.rs/agentic-robotics-embedded)
- **Repository**: [github.com/ruvnet/vibecast](https://github.com/ruvnet/vibecast)

---

**Part of the Agentic Robotics framework** • Built with ❤️ by the Agentic Robotics Team
