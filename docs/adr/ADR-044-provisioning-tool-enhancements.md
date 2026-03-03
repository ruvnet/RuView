# ADR-044: Provisioning Tool Enhancements

**Status**: Proposed
**Date**: 2026-03-03
**Deciders**: @ruvnet
**Supersedes**: None
**Related**: ADR-029, ADR-032, ADR-039, ADR-040

---

## Context

The ESP32-S3 CSI node provisioning script (`firmware/esp32-csi-node/provision.py`) is the primary tool for configuring pre-built firmware binaries without recompiling. It writes NVS key-value pairs that the firmware reads at boot.

After #131 added TDM and edge intelligence flags, the script now covers the most-requested NVS keys. However, there remain gaps between what the firmware reads from NVS (`nvs_config.c`, 20 keys) and what the provisioning script can write (13 keys). Additionally, the script lacks usability features that would help field operators deploying multi-node meshes.

### Gap 1: Missing NVS Keys (7 keys)

The firmware reads these NVS keys at boot but the provisioning script has no corresponding CLI flags:

| NVS Key | Type | Firmware Default | Purpose |
|---------|------|-----------------|---------|
| `hop_count` | u8 | 1 (no hop) | Number of channels to hop through |
| `chan_list` | blob (u8[6]) | {1,6,11} | Channel numbers for hopping sequence |
| `dwell_ms` | u32 | 100 | Time to dwell on each channel before hopping (ms) |
| `power_duty` | u8 | 100 | Power duty cycle percentage (10-100%) for battery life |
| `wasm_max` | u8 | 4 | Max concurrent WASM modules (ADR-040) |
| `wasm_verify` | u8 | 0 | Require Ed25519 signature for WASM uploads (0/1) |
| `wasm_pubkey` | blob (32B) | zeros | Ed25519 public key for WASM signature verification |

### Gap 2: No Read-Back

There is no way to read the current NVS configuration from a device. Field operators must remember what was provisioned or reflash everything. This is especially problematic for multi-node meshes where each node has different TDM slots.

### Gap 3: No Verification

After flashing, there is no automated check that the device booted successfully with the new configuration. Operators must manually run a serial monitor and inspect logs.

### Gap 4: No Config File Support

Provisioning a 6-node mesh requires running the script 6 times with largely overlapping flags (same SSID, password, target IP) and only TDM slot varying. There is no way to define a mesh configuration in a file.

### Gap 5: No Presets

Common deployment scenarios (single-node basic, 3-node mesh, 6-node mesh with vitals) require operators to know which flags to combine. Named presets would lower the barrier to entry.

### Gap 6: No Auto-Detect

The `--port` flag is required even though the script could auto-detect connected ESP32-S3 devices via `esptool.py`.

---

## Decision

Enhance `provision.py` with the following capabilities, implemented incrementally.

### Phase 1: Complete NVS Coverage

Add flags for all remaining firmware NVS keys:

```
--hop-count N          Channel hop count (1=no hop, default: 1)
--channels 1,6,11      Comma-separated channel list for hopping
--dwell-ms N           Dwell time per channel in ms (default: 100)
--power-duty N         Power duty cycle 10-100% (default: 100)
--wasm-max N           Max concurrent WASM modules 1-8 (default: 4)
--wasm-verify          Require Ed25519 signature for WASM uploads
--wasm-pubkey FILE     Path to Ed25519 public key file (32 bytes raw or PEM)
```

Validation:
- `--channels` length must match `--hop-count`
- `--power-duty` clamped to 10-100
- `--wasm-pubkey` implies `--wasm-verify`

### Phase 2: Config File and Mesh Provisioning

Add `--config FILE` to load settings from a JSON or TOML file:

```json
{
  "common": {
    "ssid": "SensorNet",
    "password": "secret",
    "target_ip": "192.168.1.20",
    "target_port": 5005,
    "edge_tier": 2
  },
  "nodes": [
    { "port": "COM7", "node_id": 0, "tdm_slot": 0 },
    { "port": "COM8", "node_id": 1, "tdm_slot": 1 },
    { "port": "COM9", "node_id": 2, "tdm_slot": 2 }
  ]
}
```

`--config mesh.json` provisions all listed nodes in sequence, computing `tdm_total` automatically from the `nodes` array length.

### Phase 3: Presets

Add `--preset NAME` for common deployment profiles:

| Preset | What It Sets |
|--------|-------------|
| `basic` | Single node, edge_tier=0, no TDM, no hopping |
| `vitals` | Single node, edge_tier=2, vital_int=1000, subk_count=32 |
| `mesh-3` | 3-node TDM, edge_tier=1, hop_count=3, channels=1,6,11 |
| `mesh-6-vitals` | 6-node TDM, edge_tier=2, hop_count=3, channels=1,6,11, vital_int=500 |

Presets set defaults that can be overridden by explicit flags.

### Phase 4: Read-Back and Verify

Add `--read` to dump the current NVS configuration from a connected device:

```bash
python provision.py --port COM7 --read
# Output:
#   ssid:         SensorNet
#   target_ip:    192.168.1.20
#   tdm_slot:     0
#   tdm_nodes:    3
#   edge_tier:    2
#   ...
```

Implementation: use `esptool.py read_flash` to read the NVS partition, then parse the NVS binary format to extract key-value pairs.

Add `--verify` to provision and then confirm the device booted:

```bash
python provision.py --port COM7 --ssid "Net" --password "pass" --target-ip 192.168.1.20 --verify
# After flash, opens serial monitor for 5 seconds
# Checks for "CSI streaming active" log line
# Reports PASS or FAIL
```

### Phase 5: Auto-Detect Port

When `--port` is omitted, scan for connected ESP32-S3 devices:

```bash
python provision.py --ssid "Net" --password "pass" --target-ip 192.168.1.20
# Auto-detected ESP32-S3 on COM7 (Silicon Labs CP210x)
# Proceed? [Y/n]
```

Implementation: use `esptool.py` or `serial.tools.list_ports` to enumerate ports.

---

## Rationale

### Why incremental phases?

Phase 1 is a small diff that closes the NVS coverage gap immediately. Phases 2-5 add progressively more UX polish. Each phase is independently useful and can be shipped separately.

### Why JSON config over YAML/TOML?

JSON requires no additional Python dependencies (stdlib `json` module). TOML requires `tomllib` (Python 3.11+) or `tomli`. JSON is sufficient for this use case.

### Why not a GUI?

The target users are embedded developers and field operators who are already running `esptool` from the command line. A TUI/GUI would add dependencies and complexity for minimal benefit.

---

## Consequences

### Positive

- **Complete NVS coverage**: Every firmware-readable key can be set from the provisioning tool
- **Mesh provisioning in one command**: `--config mesh.json` replaces 6 separate invocations
- **Lower barrier to entry**: Presets eliminate the need to know which flags to combine
- **Auditability**: `--read` lets operators inspect and verify deployed configurations
- **Fewer mis-provisions**: `--verify` catches flashing failures before the operator walks away

### Negative

- **NVS binary parsing** (Phase 4) requires understanding the ESP-IDF NVS binary format, which is not officially documented as a stable API
- **Auto-detect** (Phase 5) may produce false positives if other ESP32 variants are connected

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| NVS binary format changes in ESP-IDF v6 | Low | Medium | Pin to known ESP-IDF NVS page format; add format version check |
| `--verify` serial parsing is fragile | Medium | Low | Match on stable log tag `[CSI_MAIN]`; timeout after 10s |
| Config file credentials in plaintext | Medium | Medium | Document that config files should not be committed; add `.gitignore` pattern |

---

## Implementation Priority

| Phase | Effort | Impact | Priority |
|-------|--------|--------|----------|
| Phase 1: Complete NVS coverage | Small (1 file, ~50 lines) | High — closes feature gap | P0 |
| Phase 2: Config file + mesh | Medium (~100 lines) | High — biggest UX win | P1 |
| Phase 3: Presets | Small (~40 lines) | Medium — convenience | P2 |
| Phase 4: Read-back + verify | Medium (~150 lines) | Medium — debugging aid | P2 |
| Phase 5: Auto-detect | Small (~30 lines) | Low — minor convenience | P3 |

---

## References

- `firmware/esp32-csi-node/main/nvs_config.h` — NVS config struct (20 fields)
- `firmware/esp32-csi-node/main/nvs_config.c` — NVS read logic (20 keys)
- `firmware/esp32-csi-node/provision.py` — Current provisioning script (13 of 20 keys)
- ADR-029: RuvSense multistatic sensing mode (TDM, channel hopping)
- ADR-032: Multistatic mesh security hardening (mesh keys)
- ADR-039: ESP32-S3 edge intelligence (edge tiers, vitals)
- ADR-040: WASM programmable sensing (WASM modules, signature verification)
- Issue #130: Provisioning script doesn't support TDM
