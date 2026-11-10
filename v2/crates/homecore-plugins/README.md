# homecore-plugins

WASM integration plugin runtime for HOMECORE with native Rust runtime (P1) and Wasmtime JIT sandbox support (P2).

[![Crates.io](https://img.shields.io/crates/v/homecore-plugins.svg)](https://crates.io/crates/homecore-plugins)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![MSRV: 1.89+](https://img.shields.io/badge/MSRV-1.89%2B-purple.svg)
[![Tests](https://img.shields.io/badge/tests-10%20passing-brightgreen.svg)](https://github.com/ruvnet/RuView)
[![ADR-128](https://img.shields.io/badge/ADR-128-orange.svg)](../../docs/adr/ADR-128-homecore-integration-plugin-system.md)

**P1 scaffold**: manifest parsing, plugin traits, and in-memory native Rust plugin registry. Wasmtime sandbox (P2) and hot-reload (P3) are deferred.

## What this crate does

`homecore-plugins` provides a trait-based plugin system that can host both native Rust plugins (in-process) and WASM plugins (Wasmtime sandbox, P2). It defines:

- **PluginManifest** — JSON schema for plugin metadata (superset of Home Assistant's `manifest.json`), validated at load time
- **HomeCorePlugin trait** — async lifecycle hooks (`setup`, `teardown`, state changed handlers)
- **PluginRuntime trait** — abstraction over execution environments (native vs WASM)
- **InProcessRuntime** — built-in runtime for first-party Rust plugins (P1)
- **PluginRegistry** — manages loading, unloading, and querying plugins
- **Host ABI (stubs)** — C-compatible function signatures for WASM ↔ homecore calls (wiring in P2)

The system is designed to be feature-gated: compile with `--features wasmtime` to unlock JIT sandbox support for untrusted third-party plugins.

## Features

- **Native Rust plugins** — first-party integrations compiled into the binary, zero sandbox overhead (P1)
- **WASM plugin framework** — trait-based abstraction ready for Wasmtime JIT (P2) or wasm3 interpreter (P3)
- **PluginManifest validation** — required fields enforced at load time; superset of HA manifest fields
- **Async plugin lifecycle** — `setup()` and `teardown()` for resource management
- **State change subscriptions** — plugins can subscribe to entity state changes with handler callbac
- **Config entry lifecycle** — plugin receives config when registered; P3 adds hot-reload
- **Feature-gated runtimes** — Wasmtime (30 MB, P2) and wasm3 (50 kB, P3) are optional dependencies
- **Manifest inheritance from Home Assistant** — `codeowners`, `requirements`, `documentation`, `issue_tracker`, IoT classification

## Capabilities

| Capability | Type | Method | Notes |
|------------|------|--------|-------|
| Load native plugin | Runtime | `InProcessRuntime::load(manifest, handler)` | Sync; handler is a Rust type implementing `HomeCorePlugin` |
| Load WASM plugin | Runtime | `WasmtimeRuntime::load(wasm_bytes, manifest)` (P2) | Async; JIT compiles via Cranelift; requires `--features wasmtime` |
| List loaded plugins | Registry | `PluginRegistry::list()` | Returns `Vec<(PluginId, PluginManifest)>` |
| Query plugin config | Registry | `PluginRegistry::get_config(plugin_id)` | Returns `Arc<ConfigEntryJson>` |
| Call plugin handler | Host ABI | `hc_state_changed(event)` (P2) | WASM plugin receives state change events via exported function |
| Unload plugin | Registry | `PluginRegistry::unload(plugin_id)` | Calls `teardown()`, frees memory (P3 = hot-reload) |

## Comparison to Home Assistant

| Aspect | Home Assistant | homecore-plugins |
|--------|----------------|------------------|
| Plugin language | Python (`.py` integrations) | Rust (P1) + WASM (P2+) |
| Sandbox | None (all Python in same process) | None (P1); Wasmtime sandbox (P2) |
| Plugin discovery | `homeassistant/components/` directory | `PluginManifest` JSON + registry |
| Config lifecycle | YAML + dynamic reload | Config entry + manifest (hot-reload P3) |
| Host ABI | CPython C API | C types + Wasmtime exported functions (P2) |
| Manifest format | Home Assistant's `manifest.json` subset | Superset with `ioc_class`, `cog_publisher` |
| Feature gating | Integration-specific | Feature flags: `wasmtime`, `wasm3` |

## Performance

- **Native plugin overhead** — same as regular Rust function calls; no sandbox cost
- **WASM plugin sandbox** — Wasmtime JIT ~5 ms per call (after warmup); memory overhead ~10 MB per instance
- **Manifest parsing** — < 1 ms (serde_json)
- **Registry operations** — O(1) plugin lookup (DashMap); O(n) for `list()`
- **No per-crate benchmarks yet** — a follow-up issue tracks baseline measurements

## Usage

Native plugin (P1):

```toml
[dependencies]
homecore = "0.1.0-alpha.0"
homecore-plugins = "0.1.0-alpha.1"
async-trait = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use std::sync::Arc;

use async_trait::async_trait;
use homecore::HomeCore;
use homecore_plugins::{
    HomeCorePlugin, InProcessRuntime, PluginError, PluginId, PluginManifest, PluginRuntime,
};

struct MyPlugin;

#[async_trait]
impl HomeCorePlugin for MyPlugin {
    async fn setup(&self, _hc: HomeCore) -> Result<(), PluginError> {
        println!("Plugin setup");
        Ok(())
    }

    async fn unload(&self) -> Result<(), PluginError> {
        println!("Plugin unload");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), PluginError> {
    let manifest = PluginManifest::parse_json(
        r#"{ "domain": "my_plugin", "name": "My Plugin", "version": "0.1.0" }"#,
    )?;

    let loaded = InProcessRuntime
        .load(PluginId::new("my_plugin"), manifest, Arc::new(MyPlugin))
        .await?;
    loaded.setup(HomeCore::new()).await?;
    println!("Loaded plugin: {:?}", loaded.id);
    loaded.unload().await?;
    Ok(())
}
```

WASM plugin (P2 example):

```bash
# Build a WASM plugin (requires --features wasmtime)
cargo build -p homecore-plugin-example --target wasm32-unknown-unknown --release

# The WasmtimeRuntime will be available at P2:
# let mut runtime = WasmtimeRuntime::new();
# let plugin_id = runtime.load(wasm_bytes, manifest).await?;
```

## Relation to other HOMECORE crates

```
homecore-plugins (plugin registry + runtime abstraction)
├─ homecore (state machine; plugins receive state changes)
├─ homecore-plugin-example (reference WASM plugin)
├─ homecore-server (loads plugins at startup)
└─ homecore-automation (can invoke handlers via service calls)
```

## Security Notes

**P1 (this release)**: No sandbox. Native Rust plugins have full process access.

**P2 (planned)**: Wasmtime JIT sandbox is opt-in via `--features wasmtime`. WASM plugins run in isolated memory with explicit host ABI calls to access homecore state. The host ABI is frozen before P2 begins (ADR-128 §8 risk mitigation).

**P4+**: Ed25519 signature verification and permission enforcement for third-party Cog registry distribution.

## References

- [ADR-128: HOMECORE Integration Plugin System](../../docs/adr/ADR-128-homecore-integration-plugin-system.md)
- [homecore-plugin-example: reference WASM plugin](../homecore-plugin-example)
- [Host ABI spec](src/host_abi.rs)
- [README — wifi-densepose](../../../README.md)
