# OTA Pipeline — Full Reproduction Recipe

Verbatim agent contribution (2026-05-17), saved as authoritative
reference for the WiFi-OTA flow on this RuView fork. Kept whole
deliberately — splitting it would lose the diagnostic flowchart.

## TL;DR

OTA works because **three FW-side fixes** are in place. Without them
the chip receives the firmware, reboots, **panics during early boot
of the new partition**, the bootloader rolls back, and from outside
it looks like "OTA didn't work" even though the upload succeeded.
Most agents focus on the network side (curl, gh-action) and miss it,
because the bug lives inside the firmware.

---

## 0 · Prerequisites (without them OTA = panic loop)

These three things **must already be in the firmware running on the
chip** (i.e. in ota_0/factory before the first OTA). If they're not
there, fix once via USB-flash; after that, OTA works.

### A. `OTA_SIZE_UNKNOWN` instead of `OTA_WITH_SEQUENTIAL_WRITES`

**File:** `firmware/esp32-csi-node/main/ota_update.c:137`

```c
esp_err_t err = esp_ota_begin(update_partition, OTA_SIZE_UNKNOWN, &ota_handle);
```

**Why:** `OTA_WITH_SEQUENTIAL_WRITES` erases 4 KB pages on the fly
as it writes. If the new binary (~870 KB) is smaller than the previous
one in the same partition (~1.1 MB), **tail of the old code stays in
the partition**. The SHA-image-verify in `esp_ota_end()` only checks
the declared image-header length — residual code isn't covered. After
reboot the new app may jump into IRAM / a .literal pool address
overlapped by stale code → **Guru Meditation Error** → bootloader
rolls back.

`OTA_SIZE_UNKNOWN` forces a **full partition erase before write**
(~1.5 s overhead, unnoticeable).

### B. `config.stack_size = 8192` for httpd

**File:** `firmware/esp32-csi-node/main/ota_update.c:225`

```c
httpd_config_t config = HTTPD_DEFAULT_CONFIG();   // default stack_size = 4096
config.server_port = OTA_PORT;
config.max_uri_handlers = 12;
config.recv_wait_timeout = 30;
config.stack_size = 8192;                          // ← critical
```

**Why:** `esp_ota_end()` streams a SHA-256 verify over the entire
image and walks the mmap segments = >5 KB of local variables. On the
standard 4 KB httpd-task stack → **stack overflow** at validation
time. The chip panics **inside the handler**, before
`esp_ota_set_boot_partition()`. From outside you see
`{"status":"ok"}` (it's sent before `esp_ota_end`), but the partition
doesn't switch.

### C. Reset reason logged in `app_main`

**File:** `firmware/esp32-csi-node/main/main.c:130-153`

```c
static const char *reset_reason_str(esp_reset_reason_t r) {
    switch (r) {
        case ESP_RST_PANIC:    return "PANIC";
        case ESP_RST_TASK_WDT: return "TASK_WDT";
        case ESP_RST_SW:       return "SW";
        ...
    }
}
void app_main(void) {
    esp_reset_reason_t rr = esp_reset_reason();
    const esp_partition_t *running = esp_ota_get_running_partition();
    ESP_LOGI(TAG, "boot: reset_reason=%s running_partition=%s",
             reset_reason_str(rr),
             running ? running->label : "?");
    ...
}
```

**Why:** Without this line you **cannot tell** "new image booted
cleanly after OTA" from "new image panicked → rolled back". `/ota/status`
looks the same (or suspicious) in both cases. With this line the
first UART line after boot tells the truth:

- `reset_reason=SW  running_partition=ota_1` → OTA OK, new image in ota_1.
- `reset_reason=PANIC  running_partition=ota_0` → new image panicked,
  rollback worked. **This is the case other agents get stuck in —
  without the log it's impossible to diagnose.**

---

## 1 · Wire format of POST /ota

**Endpoint:** `POST http://<node-ip>:8032/ota`

**Headers:**
- `Content-Type: application/octet-stream` (required)
- `Content-Length: <bytes>` (curl/urllib sets it)
- `Authorization: Bearer <psk>` (only if `security/ota_psk` is in NVS)

**Body:** raw bytes of `build/esp32-csi-node.bin` — no multipart, no base64.

**Response on success:**

```json
{"status":"ok","message":"OTA update successful. Rebooting..."}
```

**Important about the response:** the chip sends it **before
`esp_restart()`**, but `vTaskDelay(1000ms)` between response and
restart **does not guarantee delivery**. On macOS / Linux curl will see:

- `{"status":"ok"...}`, or
- `Connection reset by peer` (TCP RST from the dying side), or
- `Recv failure`.

**All three are upload success.** The real check is NOT curl's
status — it's a **second GET `/ota/status` after reboot**.

---

## 2 · Chip's path through the handler

```
HTTP POST /ota
    │
    ▼
ota_check_auth(req)              ← if PSK in NVS, verifies Authorization header
    │
    ▼
esp_ota_get_next_update_partition(NULL)
    │                            ← running in ota_0 → returns ota_1, and vice-versa
    ▼
esp_ota_begin(part, OTA_SIZE_UNKNOWN, &handle)
    │                            ← full erase of target partition (~1.5 s)
    ▼
loop {
    received = httpd_req_recv(req, buf, 1024)
    esp_ota_write(handle, buf, received)
}                                ← writes in 1 KB chunks
    │
    ▼
esp_ota_end(handle)              ← SHA-256 verify over the entire image (>5 KB stack)
    │
    ▼
esp_ota_set_boot_partition(part) ← writes "boot from target" into otadata
    │
    ▼
httpd_resp_send(JSON)            ← replies {"status":"ok"...}
    │
    ▼
vTaskDelay(1000ms)               ← window so TCP flush goes out (best-effort)
    │
    ▼
esp_restart()                    ← soft reset via RTC_SW_CPU_RST
    │
    ▼
[bootloader picks ota_1 from otadata → loads new image → app_main]
    │
    ▼
"I (335) main: boot: reset_reason=SW running_partition=ota_1"
```

---

## 3 · Flashing via `scripts/ota-deploy.sh`

```bash
# Scenario A — deploy to all nodes on local /24 (auto-discover):
scripts/ota-deploy.sh

# Scenario B — specific IPs:
scripts/ota-deploy.sh 192.168.0.100 192.168.0.101

# Scenario C — build before deploy:
scripts/ota-deploy.sh --build

# Scenario D — with auth:
OTA_PSK=your_token scripts/ota-deploy.sh
```

**What the script does under the hood (4 phases):**

### Phase 1 — discovery

```python
arp -a -n  →  ['192.168.0.100', '192.168.0.101', ...]
# parallel GET /ota/status:8032 (timeout 1.5s)
# only IPs that return valid JSON survive
```

If ARP is empty (fresh Mac boot) → fallback ping-sweep `.100`–`.110`.

### Phase 2 — snapshot before

```
GET /ota/status:8032 on each node
→  remember running_partition (ota_0 or ota_1)
```

### Phase 3 — parallel upload

```python
ThreadPoolExecutor(max_workers=len(targets))
for each node:
    urllib POST with body = read_bytes(esp32-csi-node.bin)
    ConnectionResetError caught as expected (that's the reboot)
```

### Phase 4 — verify

```
sleep 10  ← wait for boot to finish
for each node (up to 6 retries, 3-s delay):
    GET /ota/status:8032
    new_part != old_part   →  ✓
    new_part == old_part   →  ✗ FAIL (panicked)
exit 0 if all OK, 1 if any node didn't confirm
```

---

## 4 · Diagnosis when "OTA doesn't work"

Flowchart that catches **every observable failure mode** on ESP32-S3
in this FW:

```
GET /ota/status works?
├── 404/timeout    → node offline / wrong network / IP changed (check `arp -a`)
├── 200, time=OLD  → OTA didn't take (see below)
└── 200, time=NEW  → OTA OK ✓

OTA didn't take — diagnose via UART (USB!):

See "boot: reset_reason=..." in UART?
├── reset_reason=POWERON  → chip didn't reboot — POST didn't arrive, check curl
├── reset_reason=SW  AND  running_partition=ota_X  → OTA OK, may be server-side cache
├── reset_reason=PANIC AND running_partition=ota_0
│       → NEW image panics at boot
│       → causes (most likely first):
│           1. OTA_WITH_SEQUENTIAL_WRITES → tail of old code (fix A above)
│           2. esp_ota_end stack overflow (fix B above)
│           3. ABI mismatch bootloader vs new app (USB-flash bootloader.bin)
│           4. real bug in new code (read the backtrace before PANIC)
├── reset_reason=TASK_WDT → handler hung mid-upload
└── reset_reason=BROWNOUT → power supply browned out under stress
                            (USB on bus power?)
```

If UART is unavailable (no USB) but HTTP works: POST then GET
`/ota/status` three times at 5 s intervals. If `next_partition`
flip-flops, the chip is in a panic loop. That's a definitive diagnosis.

---

## 5 · Why other agents fail (common pitfalls)

| Pitfall | Symptom | Fix |
|---|---|---|
| Treat OTA as a pure network problem, never look at FW | "POST returned 200 but time doesn't change" → endless curl-header experiments | **Verify the three FW prerequisites first**, before any curl |
| Use `OTA_WITH_SEQUENTIAL_WRITES` (it's in IDF examples) | OTA works once, stops working after binary size changes | Switch to `OTA_SIZE_UNKNOWN` |
| Leave httpd stack at 4 KB | Sometimes works (fast SHA), sometimes doesn't — looks flaky | `config.stack_size = 8192` |
| Enable `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y` "for safety" | Every OTA rolled back because nobody calls `esp_ota_mark_app_valid_cancel_rollback()` | Either disable, or call the API after 10 s |
| `curl` without `--data-binary` (only `-d`) | Binary corrupted by HTML-encoding | Use `--data-binary @file.bin` or urllib bytes |
| Measure success by HTTP response code | Connection reset = normal (esp_restart kills socket), not failure | Re-check via **GET /ota/status after reboot** |
| Don't wait 10 s after reboot before verify | Verify times out, agent thinks OTA failed | `sleep 10` (or backoff retries) |
| Ignore that mDNS names drift | Flash the wrong node, or stale ARP cache | Auto-discover by IP **at deploy time**, not by hostname |
| Share a single file descriptor across upload threads | Race conditions, partial reads | Each upload-thread opens its own file |
| Rely on bootloader rollback instead of explicit app_valid | Image sometimes flagged BAD, OTA becomes non-idempotent | If rollback enabled, MUST call `esp_ota_mark_app_valid_cancel_rollback()` |

---

## 6 · Things other agents do **wrong**

From recurring patterns in others' logs:

1. **Rely on `idf.py flash --port .../ota`** — that mode does NOT
   exist in idf.py. OTA is only via the HTTP handler.
2. **Send via `ssh esp32 'esp_ota_write ...'`** — ESP32 has no shell;
   OTA is only via the HTTP endpoint.
3. **Run MQTT-based OTA** — this FW has no MQTT client; only HTTP
   POST on 8032.
4. **Use ESP RainMaker / esp_https_ota** — those require HTTPS +
   cert; we serve plain HTTP. Don't confuse the APIs.
5. **Re-use an old build of
   `firmware/esp32-csi-node/build/esp32-csi-node.bin`** — forget to
   run `idf.py build`. The script's `--build` solves that.

---

## 7 · Quick reference (for the next agent)

```bash
# Once over USB if the nodes still run pre-fix firmware:
cd /Users/arsen/Desktop/RuView/firmware/esp32-csi-node
source ~/esp/esp-idf-v5.2/export.sh
idf.py build

# Hold BOOT+RESET on the device
cd build
esptool.py --chip esp32s3 --port /dev/cu.usbmodem... -b 460800 \
  --before default-reset --after hard-reset write-flash \
  --flash-mode dio --flash-size 8MB --flash-freq 80m \
  0x0 bootloader/bootloader.bin \
  0x8000 partition_table/partition-table.bin \
  0xf000 ota_data_initial.bin \
  0x20000 esp32-csi-node.bin

# Forever after, over WiFi:
scripts/ota-deploy.sh --build
# (auto-discover, parallel POST, verify, exit code)
```

---

**Bottom line:** OTA is not "send a file via curl", it's an
**end-to-end protocol** between the on-chip handler and the host
tooling. 80 % of the work lives on the FW side (correct erase,
correct stack, correct log). The network part is trivial
(`urllib.request.urlopen(POST)`). Agents who "can't" usually stopped
at the network layer and didn't realise the chip is panicking.
