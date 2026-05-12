# `ruv_temporal` — ESP32-S3 on-device temporal head

ESP-IDF component implementing ADR-095 (#513). The Rust staticlib at
`src/lib.rs` wraps `ruvllm_sparse_attention` (vendored at
`vendor/ruvector/crates/ruvllm_sparse_attention`) and exposes a narrow
C ABI declared in `include/ruv_temporal.h`.

## Status

| Phase | Scope | State |
|-------|-------|-------|
| 4 — Scaffold | Cargo.toml, src/{lib.rs,window.rs,weights.rs}, include/ruv_temporal.h, CMakeLists.txt, .cargo/config.toml | **Done.** |
| 5 — Cross-compile | `cargo +esp build --release --target xtensa-esp32s3-none-elf` produces `libruv_temporal.a`. | **Blocked** — see below. |
| 6 — Wire from edge_processing.c | FreeRTOS task on Core 1, queue from adaptive_controller fast loop, push() in fast tick, classify() at 1 Hz, emit `0xC5110007` packet. | **Done** in `main/temporal_task.c` (no-op shim path verified by 8MB firmware build with feature off). |
| 7 — COM8 validation | Flash 8MB build with `CONFIG_CSI_TEMPORAL_HEAD_ENABLED=y`, soak ≥5 min, check no Tmr Svc / task_wdt overflow. | Pending board reattach. |

## Module map

| File | Purpose |
|------|---------|
| `src/lib.rs` | C ABI: `ruv_temporal_init / push / classify / destroy / kernel_self_test` |
| `src/window.rs` | `FrameRing` rolling buffer used by `ruv_temporal_push` |
| `src/weights.rs` | Loader-side mirror of host `wifi_densepose_temporal::weights`. Parses the `.rvne` blob format (magic `RVNE`, version 1, FP32/FP16, CRC32-IEEE). Bit-exact with the host crate; a blob produced by the host's `WeightBlob::serialize()` parses here byte-for-byte. |
| `include/ruv_temporal.h` | Public C header consumed by `main/temporal_task.c` |
| `shim.c` | Empty C shim for `idf_component_register` |

## Phase 5 blocker — esp toolchain rust-src bug

The system esp toolchain at `C:\Users\ruv\.rustup\toolchains\esp` has
no precompiled `core` for `xtensa-esp32s3-none-elf`. It requires
`-Z build-std=core,alloc`, but the bundled rust-src snapshot
(`esp` channel, nightly 2025-09-16) hits two known bugs when build-std
compiles `core`:

1. `library/portable-simd/crates/core_simd/src/simd/ptr/mut_ptr.rs` —
   `Copy` trait and `size_of` not in scope, ~16,000 errors.
2. `library/core` itself — "cannot resolve a prelude import",
   "attributes starting with `rustc` are reserved", `concat!` macro
   not found.

These are upstream Rust nightly snapshot regressions, not anything
this component is doing wrong. The fix is to refresh the esp toolchain
to a newer nightly:

```powershell
C:/Users/ruv/.cargo/bin/espup.exe install
# (re-source export-esp.ps1 / export-esp.sh after install)
```

`espup install` pulls the latest pinned esp Rust + LLVM. It is a
~1.5 GB download and ~5-10 min install. That step lands in the next
loop iteration of #513 implementation work.

## Build (once Phase 5 unblocks)

From this directory:

```bash
cargo +esp build --release --target xtensa-esp32s3-none-elf
```

Output:
`target/xtensa-esp32s3-none-elf/release/libruv_temporal.a`.

ESP-IDF's `idf.py build` will pick this up via `CMakeLists.txt` —
`add_custom_command` runs the cargo build before
`idf_component_register` consumes the static library.

## C ABI summary

```c
esp_err_t ruv_temporal_init(const uint8_t *weights, size_t wlen,
                            uint32_t input_dim, uint32_t window_len,
                            uint32_t n_classes,
                            ruv_temporal_ctx_t **out_ctx);
esp_err_t ruv_temporal_push(ruv_temporal_ctx_t *ctx, const float *frame);
esp_err_t ruv_temporal_classify(ruv_temporal_ctx_t *ctx,
                                float *logits, uint32_t n_classes);
void      ruv_temporal_destroy(ruv_temporal_ctx_t *ctx);
esp_err_t ruv_temporal_kernel_self_test(void);
```

Threading: caller is responsible. Per ADR-095 §3.3, the firmware will
spawn a single dedicated FreeRTOS task that owns the context and
serialises all calls — push() and classify() are not internally
synchronised.
