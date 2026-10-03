# Running RuView with Docker (and CasaOS)

This guide covers running the RuView **sensing server** (the Rust + Axum service
that hosts the REST API, WebSocket stream, and web dashboard) as a Docker
container, and importing it into [CasaOS](https://casaos.io/).

It uses the prebuilt image `ruvnet/wifi-densepose:latest` from Docker Hub, so the
host does **not** need to compile the Rust workspace.

> **Architecture note:** the published `:latest` tag currently ships an **amd64**
> layer only. On an amd64 host (e.g. an x86 mini-PC / NUC / Ryzen box) it runs as
> is. On arm64 (Raspberry Pi, etc.) build locally first:
> `docker compose -f docker/docker-compose.yml build`.

---

## 1. Quick start (no hardware)

```bash
cd ruview
CSI_SOURCE=simulated docker compose -f docker-compose.yml up -d
```

Then open the dashboard:

```
http://<host-ip>:3000/ui/index.html
```

> The bare root `http://<host-ip>:3000/` is only an **API index** (a plain list
> of endpoints) — the visual dashboards live under `/ui/`:
> `index.html` (main), `observatory.html` (live feed), `pose-fusion.html`
> (webcam + CSI), `viz.html` (3D).

`CSI_SOURCE=simulated` feeds the pipeline with synthetic CSI so you can explore
the dashboard, API, and vital-sign/pose visualizations without any hardware.

Stop / remove:

```bash
docker compose -f docker-compose.yml down
```

---

## 2. Ports

The compose file (`docker-compose.yml`) publishes these host ports:

| Service                | Container | Host | Notes |
|------------------------|-----------|------|-------|
| REST API + web UI      | 3000/tcp  | **3000** | Dashboard lives here (`/`, `/ui/...`) |
| WebSocket sensing feed | 3001/tcp  | **3001** | `ws://host:3001/ws/sensing` |
| ESP32 CSI ingest       | 5005/udp  | **5005** | ESP32-S3 nodes stream CSI frames here |

If a host port clashes on your machine, edit the `published:` values in
`docker-compose.yml` (and update `port_map` / `webui_port` in the
`x-casaos` block to match the new UI port). The dashboard's WebSocket URL is
derived from the page host, so keep the API and WS ports reachable from the same
hostname.

Verified endpoints (all return `200` once running):

```
/                      /ui/index.html          /ui/observatory.html
/api/v1/status         /api/v1/sensing/latest   /api/v1/models
```

---

## 3. Data source modes (`CSI_SOURCE`)

| Value       | Behaviour |
|-------------|-----------|
| `auto`      | (default) Probe UDP 5005 for an ESP32 node; fall back to `simulated`. |
| `esp32`     | Require real CSI frames from an ESP32-S3 node on UDP 5005. |
| `simulated` | Synthetic CSI — no hardware. Best for first evaluation. |
| `wifi`      | Host Wi-Fi RSSI/scan (Windows `netsh`) — **not** available inside a Linux container. |

Set it inline (`CSI_SOURCE=esp32 docker compose ... up -d`) or via a `.env` file
next to the compose, or in the CasaOS environment-variable UI.

### Models

Drop `.rvf` model files into `./data/models/` (mounted to `/app/models`); the API
exposes them under `/api/v1/models`. Pretrained weights:

```bash
pip install huggingface_hub
huggingface-cli download ruvnet/wifi-densepose-pretrained --local-dir data/models/wifi-densepose-pretrained
```

### Optional API auth

Leave `RUVIEW_API_TOKEN` empty for LAN-only use. Set it to require
`Authorization: Bearer <token>` on `/api/v1/*`:

```bash
RUVIEW_API_TOKEN=$(openssl rand -hex 32) docker compose -f docker-compose.yml up -d
```

---

## 4. Importing into CasaOS

The compose file includes an `x-casaos:` metadata block (icon, title, category,
description, port map), so CasaOS shows it as a proper app tile.

**Option A — CasaOS UI (Custom Install)**

1. CasaOS dashboard → **App Store** → **Custom Install** (the `+` / "Install a
   customized app").
2. Switch to the **Import** / YAML view and paste the contents of
   `docker-compose.yml`.
3. Install. The tile opens `http://<host-ip>:3000/ui/index.html`.

**Option B — CLI (CasaOS still detects the container)**

```bash
cd ruview
docker compose -f docker-compose.yml up -d
```

> **Icon:** the manifest points at
> `https://cdn.jsdelivr.net/gh/ruvnet/RuView@main/assets/ruview-icon.png`
> (jsDelivr serves `assets/ruview-icon.png` straight from the repo). To use a
> different or fully local icon, replace both `icon:` URLs (in `labels:` and
> `x-casaos:`) with another URL or a file path served by your own host.

---

## 5. Connecting live ESP32-S3 CSI nodes

RuView's full capability set (presence through walls, breathing/heart rate, fall
detection, pose) needs **Channel State Information** from a CSI-capable node.

1. Flash the firmware from `firmware/esp32-csi-node/` to an **ESP32-S3**
   (8 MB or 4 MB). See the repo `CLAUDE.md` "ESP32 Firmware Build" section.
2. Provision Wi-Fi + the sink (this server's) IP:
   ```bash
   python firmware/esp32-csi-node/provision.py --port /dev/ttyUSB0 \
     --ssid "YourWiFi" --password "secret" --target-ip <host-ip>
   ```
   Point `--target-ip` at the host running this container; frames arrive on
   **UDP 5005**.
3. Start with `CSI_SOURCE=esp32` (or `auto`).

> **Supported chips:** ESP32-**S3** (dual-core) and ESP32-**C6**. The original
> ESP32 and ESP32-**C3** are **single-core** and cannot run the CSI DSP pipeline.

---

## 6. Operations

```bash
# Logs
docker logs ruview -f --tail 100

# Restart / update to the latest image
docker compose -f docker-compose.yml pull
docker compose -f docker-compose.yml up -d

# Status of the running server
curl -s http://<host-ip>:3000/api/v1/status
```

**Troubleshooting**

- *Dashboard loads but no data:* check the data source — with `auto` and no ESP32
  present it falls back to `simulated`; logs print `Data source: ...`.
- *Multiple ESP32 nodes on Docker Desktop for Windows:* multi-source UDP collapses
  to one source IP at the WSL boundary. Use the host relay (see
  `docs/TROUBLESHOOTING.md §9`). Native Linux/CasaOS hosts are unaffected.
- *Port clash on 3000/3001:* edit `published:` in `docker-compose.yml`.
