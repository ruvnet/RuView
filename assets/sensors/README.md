# Sensor & antenna hardware inventory

Photos of the additional sensor + antenna hardware staged for ADR-120+
experimentation in this deployment. Captured 2026-05-18.

| File | What it is | Role |
|---|---|---|
| [`sensor_01.jpeg`](sensor_01.jpeg) | 5× u.FL (IPEX-1) pigtail antennas, bare cable | External antenna feed for ESP32-S3 boards that have a u.FL connector but currently use the on-PCB chip antenna. Adds gain + diversity once attached. |
| [`sensor_02.jpeg`](sensor_02.jpeg) | 4× flat PCB-strip antennas (2.4 GHz, FR4 copper trace, 3M double-sided tape backing) + u.FL pigtails | Stick-on external antennas — better gain than the chip antenna, doubles as polarisation diversity if mounted perpendicular to one another. |
| [`sensor_03.jpeg`](sensor_03.jpeg) | **HLK-LD2402** 24 GHz mmWave radar module (V1.0, chip `S1KM0008` 2438 batch), TX/RX patch antennas, UART output | New modality. mmWave gives sub-mm range to a moving target — useful for vital-sign disambiguation alongside WiFi CSI. UART @ 256000 8N1. |
| [`sensor_04.jpeg`](sensor_04.jpeg) | CP2102 USB-to-UART bridge (AMS1117-3.3 LDO, USB-A) | Powers + reads the HLK-LD2402 from a Mac/Linux host. Pin map: GND / RXT / TXD / 3.3V / RTS / CTS. |
| [`sensor_05.jpeg`](sensor_05.jpeg) | The two boards from `sensor_03` + `sensor_04` wired together | Working HLK-LD2402 setup ready to receive on a USB tty (no firmware needed — module ships with its own factory firmware). |
| [`sensor_06.jpeg`](sensor_06.jpeg) | Back of ESP32-S3 dev board with **microSD slot**, marked "Camera / SD Card / PSRAM", dual USB-UART + USB-OTG | Different board family from the 6 already in service. SD slot + USB-OTG + on-board camera connector make it a candidate for **on-device camera ground-truth capture** (alternative to using the Mac webcam for ADR-079 / Pack E.2 retraining). |
| [`sensor_07.jpeg`](sensor_07.jpeg) | ESP32-S3-WROOM with **OV-camera mounted** + camera ribbon FFC connector | Same board family as `sensor_06`. Camera ribbon already attached. With WiFi CSI + onboard camera in one device, this could collect synchronised paired data (CSI + image) entirely without involving the Mac — the missing piece for the wiflow fine-tune Pack E.2. |
| [`sensor_08.jpeg`](sensor_08.jpeg) | Back of **YD-ESP32-23 2022-V1.3 (V2356)** dev board | Same board family as the 6 currently provisioned (nodes 1-6). Spare. |
| [`sensor_09.jpeg`](sensor_09.jpeg) | Front of **YD-ESP32-23**: ESP32-S3-N16R8 WiFi+BT module, FTDI USB-serial, RGB LED, dual USB-C | Confirms it's the **same hardware** we flashed for nodes 3-6 (FT232R serial chip on board, see `/dev/cu.usbserial-A5069RR4` during provisioning). |

## Suggested next moves

* **u.FL antennas (`sensor_01`/`_02`)** — attach to the two
  "near-AP" nodes (n1, n5) which currently sit at sep_ratio ~0.05 per
  the ADR-118 audit. External antenna oriented perpendicular to the
  body axis should pull more body modulation into the signal.
* **HLK-LD2402 mmWave (`sensor_03`–`05`)** — start as a **vitals
  ground-truth reference** for the WiFi pipeline. Connect via the
  USB-UART, log breathing rate at the same time as the WiFi vitals
  detector, compare. Later fuse via `MultistaticFuser` if accuracy
  improves materially (would warrant a fresh ADR).
* **Camera-bearing ESP32-S3 (`sensor_06`/`_07`)** — the cleanest path
  to Pack E.2 (WiFlow camera-supervised retrain): collect synced
  `(CSI, MediaPipe keypoints)` pairs entirely on-device. Avoids the
  awkward "laptop is the camera AND the server AND in the sensing
  zone" problem from the earlier session.
* **Spare YD-ESP32-23 (`sensor_08`/`_09`)** — flash with the same
  build + `provision.py --node-id 7` if/when a 7-node deployment is
  desired. Bring-up is 5 min per the ADR-117 OTA recipe.

---

These are reference photos only — not used by any code path. Linked
from `docs/use-cases.md` and `CHECKLIST.md` Deferred section so future
sessions can see the available hardware at a glance.
