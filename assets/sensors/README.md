# Sensor & antenna hardware inventory

Photos of the deployment's 6-node sensor mesh + external-antenna stock,
captured 2026-05-18. The fleet splits cleanly into two roles: two
**camera-bearing** nodes (1, 2) that can collect ground-truth keypoints
on-device, and four **antenna-upgradeable** nodes (3-6) that supply
spatial coverage.

## Active sensor mesh (6 ESP32-S3 nodes)

| node | IP | Board | Camera | u.FL | Photos |
|---|---|---|---|---|---|
| 1 | 192.168.0.101 | ESP32-S3 + OV camera + microSD + USB-OTG (FFC ribbon) | ✅ | — | [`sensor_06`](sensor_06.jpeg), [`sensor_07`](sensor_07.jpeg) |
| 2 | 192.168.0.100 | same as node 1 | ✅ | — | [`sensor_06`](sensor_06.jpeg), [`sensor_07`](sensor_07.jpeg) |
| 3 | 192.168.0.102 | YD-ESP32-23 V1.3, ESP32-S3-N16R8 + FTDI USB-serial + dual USB-C | — | ✅ | [`sensor_08`](sensor_08.jpeg), [`sensor_09`](sensor_09.jpeg) |
| 4 | 192.168.0.104 | same as node 3 | — | ✅ | same |
| 5 | 192.168.0.105 | same as node 3 | — | ✅ | same |
| 6 | 192.168.0.106 | same as node 3 | — | ✅ | same |

## External antenna stock (for nodes 3-6)

| File | What | Use |
|---|---|---|
| [`sensor_01.jpeg`](sensor_01.jpeg) | 5× u.FL (IPEX-1) pigtail antennas, bare cable | Direct feed via the u.FL connector on YD-ESP32-23 boards (nodes 3-6). Adds gain over the chip antenna; supports polarisation diversity if mounted perpendicular pairs. |
| [`sensor_02.jpeg`](sensor_02.jpeg) | 4× flat PCB-strip 2.4 GHz antennas with 3M double-sided tape backing + u.FL pigtails | Stick-on external antennas — better directivity than the bare pigtail. One per node 3-6 = full set. |

## Auxiliary modality — mmWave radar (vitals ground truth)

| File | What | Use |
|---|---|---|
| [`sensor_03.jpeg`](sensor_03.jpeg) | HLK-LD2402 24 GHz mmWave radar (V1.0, chip `S1KM0008` 2438 batch), TX/RX patch antennas | New sensing modality. mmWave gives sub-mm range to a moving target — ideal for vitals (breathing / pulse) ground truth alongside WiFi CSI. UART output @ 256000 8N1. |
| [`sensor_04.jpeg`](sensor_04.jpeg) | CP2102 USB-to-UART bridge (AMS1117-3.3 LDO, USB-A) | Powers + reads the HLK-LD2402 from the Mac. Pin map: GND / RXT / TXD / 3.3V / RTS / CTS. |
| [`sensor_05.jpeg`](sensor_05.jpeg) | HLK-LD2402 + USB-UART wired together | Plug-and-play setup; module ships with factory firmware, no flashing required. |

## Suggested next moves

* **External antennas for nodes 3-6** — the 4 PCB-strip antennas in
  `sensor_02` map 1:1 to the 4 YD-ESP32-23 boards. Power-cycle each
  to attach. Per the ADR-118 audit, nodes near the AP currently sit
  at sep_ratio ~0.05 — external antennas perpendicular to the body
  axis should pull more body modulation into the signal.
* **HLK-LD2402 as vitals ground truth** — connect via USB-UART, log
  breathing rate alongside the WiFi vitals detector, compare bias.
  Later fuse via `MultistaticFuser` if accuracy delta is material.
  Would warrant a fresh ADR.
* **On-device camera capture for WiFlow Pack E.2 retrain** — nodes
  1 and 2 already have OV-cameras. Path is:
  1. Extend `firmware/esp32-csi-node/` with a parallel
     `camera_capture.c` that grabs frames @ ~10 Hz and streams them
     to the server as MJPEG over a new UDP port (or HTTP `multipart/x-mixed-replace`).
  2. Run MediaPipe Pose on the server (we already have it in
     `~/.venv/ruview-train` from this session).
  3. Time-align CSI + keypoints via the existing
     `scripts/align-ground-truth.js` infrastructure.
  4. Train via `scripts/train-wiflow-supervised.js --scale lite`.
  This is the cleanest replacement for the awkward "laptop is the
  camera AND the server AND in the sensing zone" workaround.

---

These are reference photos. Linked from `docs/use-cases.md` and
`CHECKLIST.md` so future sessions see the available hardware at
a glance.
