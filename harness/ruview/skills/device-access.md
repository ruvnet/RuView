---
name: device-access
description: Reach RuView sensing hardware from the machine it is attached to — ESP32 CSI nodes (serial + UDP stream), 60 GHz MR60BHA2 / 24 GHz LD2410 mmWave radars, RPLIDAR and iPhone LiDAR — and from other machines over SSH, read-only.
---

# device-access

Hardware tools must run **on the host the device is plugged into**. A cloud
agent cannot see your USB ports. Run `npx @ruvnet/ruview` locally, start a
local Claude Code session there (`claude remote-control` in the repo if you
want to drive it from the Claude app), or add the machine as an SSH host.

## 1. Find devices

```
npx @ruvnet/ruview doctor --group python,serial,devices,remote
npx @ruvnet/ruview devices          # USB VID:PID → likely ESP32 / mmWave / RPLIDAR + confirm command
```

## 2. Read each modality

| Device | Command | Confirms |
|---|---|---|
| ESP32 node (USB) | `ruview doctor --port <p> --probe`, `ruview monitor --port <p>` | chip id; CSI callbacks in the serial log (the monitor never resets the node; C6/S3 USB-Serial/JTAG builds log on the native USB port) |
| ESP32 / Realtek node (network) | `ruview esp32 --seconds 10 [--udp-port 5005]` | per-node CSI rate, sequence loss, RSSI, CSI shapes, device vitals; Realtek RAC1 CSI and heartbeat-only senders |
| Realtek RTL8721Dx (USB, PL2303) | `ruview monitor --port <p> --baud 1500000` | `RUVIEW_CSI: frame #` lines in the Ameba log |
| Live CSI → kernel | `ruview esp32 --seconds 45 --analyze [--node-id N] [--backend napi]` | kernel vitals summary at the measured frame rate (no reference measurement) |
| 60 GHz MR60BHA2 | `ruview mmwave --port <p> --model mr60bha2` | valid frames, checksum errors, presence/distance/device HR+BR |
| 24 GHz LD2410 | `ruview mmwave --port <p> --model ld2410` | target state and distances |
| RPLIDAR | `ruview lidar --source rplidar --port <p> [--baud 115200]` | points, revolutions, range, angular coverage |
| iPhone LiDAR | `export RUVIEW_LIDAR_TOKEN=…; ruview lidar --source iphone --url ws://HOST:8787/ws/lidar` | depth frame rate, confident fraction, median depth |

The UDP capture cannot share port 5005 with a running sensing server; stop
the server or have the nodes target another port.

## 3. Other machines (SSH, read-only)

```
npx @ruvnet/ruview hosts add --name lab-pi --ssh ruv@lab-pi.local
ssh ruv@lab-pi.local true                       # pin the host key once
npx @ruvnet/ruview hosts run --host lab-pi --tool ruview_devices_scan
npx @ruvnet/ruview hosts run --host lab-pi --tool ruview_mmwave_read --args-json '{"port":"/dev/ttyUSB0"}'
```

Remote hosts run a pinned `@ruvnet/ruview` and accept only read-only tools.
Flash, train, and calibrate on the host itself.

## Evidence

Device readouts are `MEASURED` *readings*, not accuracy results. mmWave
heart/breathing values are the radar's own estimates. Hardware validation
still means a captured log or stream from real silicon, recorded with the
command that produced it.
