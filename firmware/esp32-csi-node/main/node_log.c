#include "node_log.h"

#include <limits.h>
#include <string.h>
#include <unistd.h>

#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_vfs_fat.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "nvs.h"
#include "nvs_flash.h"

#include "c6_sync_espnow.h"
#include "csi_collector.h"
#include "nvs_config.h"
#include "thermal.h"

/* firmware/esp32-csi-node/partitions_4mb.csv has two 1.81 MB app slots and
 * only 256 KB free -- no room for the FAT volume this module needs. The
 * whole implementation below is compiled out on that profile; every entry
 * point still links via the stubs in the #else branch so main.c's call
 * sites need no #ifdef of their own. See partitions_display.csv (8MB) and
 * partitions_16mb.csv for the profiles where this module is active. */
#ifndef CONFIG_ESPTOOLPY_FLASHSIZE_4MB

static const char *TAG = "node_log";

/* The FAT volume reserved in partitions_16mb.csv (storage, 8000K) and never
 * mounted until now. */
#define NODE_LOG_PARTITION  "storage"
#define NODE_LOG_MOUNT      "/nlog"
#define NODE_LOG_PATH       NODE_LOG_MOUNT "/health.bin"

/* Ring capacity. 60000 * 128 B = 7.68 MB, just inside the 8000K volume with
 * room for FAT's own metadata. At the 300 s cadence that is ~208 days. */
#define NODE_LOG_MAX_RECORDS 60000

/* Hard floor between periodic samples. The design settled on 300 s; this is
 * the enforcement, not the schedule. A caller asking more often is DROPPED
 * rather than served, because flash wear is the one way this module can do
 * harm and a bug in a caller must not be able to cause it. */
#define NODE_LOG_MIN_PERIOD_S 60

/* Events bypass the periodic floor -- they are rare and they are the whole
 * point -- but not without limit, or a disconnect storm becomes a write storm. */
#define NODE_LOG_MIN_EVENT_MS 1000

static bool              s_active;
static FILE             *s_fp;
static wl_handle_t       s_wl = WL_INVALID_HANDLE;
static SemaphoreHandle_t s_lock;
static uint16_t          s_boot_id;
static uint32_t          s_seq;
static size_t            s_count;      /* records currently in the ring */
static size_t            s_head;       /* next slot to write */
static int64_t           s_last_periodic_us;
static int64_t           s_last_event_us;

/* CRC16/CCITT-FALSE. A record that fails this is skipped by the reader rather
 * than trusted, which matters because a power cut mid-write leaves a partial
 * record and the ring has no other way to know. */
static uint16_t crc16(const uint8_t *p, size_t n)
{
    uint16_t crc = 0xFFFF;
    for (size_t i = 0; i < n; i++) {
        crc ^= (uint16_t)p[i] << 8;
        for (int b = 0; b < 8; b++) {
            crc = (crc & 0x8000) ? (uint16_t)((crc << 1) ^ 0x1021) : (uint16_t)(crc << 1);
        }
    }
    return crc;
}

/* 16 B header, then a type-specific payload. The header is identical across
 * types so a reader can order and validate without dispatching first. */
typedef struct __attribute__((packed)) {
    uint8_t  type;
    uint8_t  flags;
    uint16_t boot_id;
    uint32_t seq;
    uint32_t uptime_s;
    uint16_t payload_len;
    uint16_t crc16;        /* over the record with this field zeroed */
} node_log_hdr_t;

_Static_assert(sizeof(node_log_hdr_t) == 16, "header must be 16 B");

/* ~72 B used of the 112 B payload budget, leaving ~40 B of deliberate slack so
 * a field can be added later WITHOUT changing the record size and invalidating
 * every reader already in the field. */
typedef struct __attribute__((packed)) {
    int8_t   die_c;
    uint8_t  thermal_state;
    int8_t   tx_dbm;
    int8_t   ap_rssi_dbm;      /* the one field we did not have and needed */
    uint8_t  channel;
    uint8_t  wifi_state;
    uint8_t  gate_mode;
    uint8_t  gate_seq_period;
    uint16_t free_heap_kib;
    uint16_t min_heap_kib;
    uint16_t csi_fps_x100;
    uint32_t frames_processed;
    uint32_t frames_rejected;
    uint32_t seq_drop;
    uint32_t tx_early_drop;
    uint16_t tx_rate_skip;
    uint32_t tx_send_fail;
    uint16_t disconnect_count;
    uint8_t  last_disc_reason;
    int8_t   last_disc_rssi;
    uint8_t  mesh_flags;       /* bit0 valid, bit1 leader */
    uint8_t  leader_id;
    int32_t  mesh_offset_us;
    uint16_t mesh_staleness_ms;
    uint32_t mesh_seq;
} node_log_periodic_t;

typedef struct __attribute__((packed)) {
    uint32_t reset_reason;
    uint32_t prev_uptime_s;
    node_log_periodic_t health;   /* a boot record carries a snapshot too */
} node_log_boot_t;

typedef struct __attribute__((packed)) {
    uint8_t  subtype;
    uint8_t  _pad[3];
    int32_t  a;
    int32_t  b;
} node_log_event_t;

/* Counters main.c owns; set through the setters below rather than reached for,
 * so this module has no include cycle with main.c. */
static volatile uint16_t s_disconnect_count;
static volatile uint8_t  s_last_disc_reason;
static volatile int8_t   s_last_disc_rssi;

void node_log_note_disconnect(uint8_t reason, int8_t rssi)
{
    s_disconnect_count++;
    s_last_disc_reason = reason;
    s_last_disc_rssi = rssi;
}

static void fill_health(node_log_periodic_t *h)
{
    memset(h, 0, sizeof(*h));

    /* thermal.c initialises s_last_c to -273.0f and only replaces it on the
     * first thermal_tick(), which runs from the adaptive controller -- LATER
     * than the boot record and later than the first periodic samples. Casting
     * that sentinel to int8 yields -17, a PLAUSIBLE temperature, which is
     * exactly the kind of invented-but-believable number that must never reach
     * a log someone will later reason from. Store INT8_MIN as an explicit
     * "no reading" instead; node_log_read.py renders it as n/a. */
    float die = thermal_celsius();
    h->die_c = (die < -100.0f || die > 125.0f) ? INT8_MIN : (int8_t)die;
    h->thermal_state   = (uint8_t)thermal_state();
    h->tx_dbm          = thermal_tx_dbm();
    h->gate_mode       = csi_collector_get_gate_mode();
    h->gate_seq_period = csi_collector_get_gate_seq_period();

    /* The association link, NOT the CSI-derived rssi_median_dbm in
     * adaptive_controller.c. Nodes 5 and 6 show every symptom of a bad uplink
     * and this is the one call that can confirm or refute it. */
    wifi_ap_record_t ap;
    if (esp_wifi_sta_get_ap_info(&ap) == ESP_OK) {
        h->ap_rssi_dbm = (int8_t)ap.rssi;
        h->channel     = ap.primary;
        h->wifi_state  = 1;
    }

    h->free_heap_kib = (uint16_t)(esp_get_free_heap_size() / 1024);
    h->min_heap_kib  = (uint16_t)(esp_get_minimum_free_heap_size() / 1024);
    h->csi_fps_x100  = csi_collector_get_pkt_yield_per_sec() * 100;
    h->tx_send_fail  = csi_collector_get_send_fail_count();

    h->disconnect_count  = s_disconnect_count;
    h->last_disc_reason  = s_last_disc_reason;
    h->last_disc_rssi    = s_last_disc_rssi;

    h->mesh_flags = (uint8_t)((c6_sync_espnow_is_valid() ? 1 : 0)
                            | (c6_sync_espnow_is_leader() ? 2 : 0));
    int64_t off = c6_sync_espnow_get_offset_us();
    /* Saturate rather than wrap: a wrapped offset reads as a plausible small
     * number and would be believed. */
    if (off > INT32_MAX)      h->mesh_offset_us = INT32_MAX;
    else if (off < INT32_MIN) h->mesh_offset_us = INT32_MIN;
    else                      h->mesh_offset_us = (int32_t)off;
}

/* Append one record. Callers hold no lock; this takes it. */
static void append(uint8_t type, const void *payload, size_t len)
{
    if (!s_active || len > (NODE_LOG_RECORD_SIZE - sizeof(node_log_hdr_t))) {
        return;
    }
    if (xSemaphoreTake(s_lock, pdMS_TO_TICKS(200)) != pdTRUE) {
        return;   /* never block a caller on the log */
    }

    uint8_t rec[NODE_LOG_RECORD_SIZE];
    memset(rec, 0, sizeof(rec));
    node_log_hdr_t *h = (node_log_hdr_t *)rec;
    h->type        = type;
    h->boot_id     = s_boot_id;
    h->seq         = ++s_seq;
    h->uptime_s    = (uint32_t)(esp_timer_get_time() / 1000000);
    h->payload_len = (uint16_t)len;
    h->crc16       = 0;
    memcpy(rec + sizeof(*h), payload, len);
    h->crc16 = crc16(rec, sizeof(rec));

    if (fseek(s_fp, (long)(s_head * NODE_LOG_RECORD_SIZE), SEEK_SET) == 0
        && fwrite(rec, 1, sizeof(rec), s_fp) == sizeof(rec)) {
        /* fflush only pushes the C stdio buffer into the VFS; FATFS still holds
         * the directory entry and FAT chain in RAM. Without fsync a reboot
         * loses the file's size, and on the next open "r+b" sees a zero-length
         * file -- which is precisely what the node-3 pilot showed: boot 1's
         * records vanished while the NVS-backed prev_uptime survived. A log
         * that does not survive a power cycle is the exact thing this module
         * exists to prevent, so the cost of an fsync per record is accepted:
         * at one record per 60 s it is nothing, and the bounded write rate is
         * what keeps it that way. */
        fflush(s_fp);
        fsync(fileno(s_fp));
        s_head = (s_head + 1) % NODE_LOG_MAX_RECORDS;
        if (s_count < NODE_LOG_MAX_RECORDS) s_count++;
    } else {
        ESP_LOGW(TAG, "record write failed; disabling to protect the volume");
        s_active = false;
    }
    xSemaphoreGive(s_lock);
}

esp_err_t node_log_init(void)
{
    if (s_active) return ESP_OK;

    s_lock = xSemaphoreCreateMutex();
    if (!s_lock) return ESP_ERR_NO_MEM;

    const esp_vfs_fat_mount_config_t cfg = {
        .format_if_mount_failed = true,
        .max_files              = 2,
        .allocation_unit_size   = CONFIG_WL_SECTOR_SIZE,
    };
    esp_err_t err = esp_vfs_fat_spiflash_mount_rw_wl(
        NODE_LOG_MOUNT, NODE_LOG_PARTITION, &cfg, &s_wl);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "FAT mount failed (%s); logging DISABLED, node unaffected",
                 esp_err_to_name(err));
        return err;
    }

    /* boot_id orders records across reboots. Without an RTC it is the only
     * ordering that survives a power cycle. */
    nvs_handle_t nh;
    if (nvs_open("nodelog", NVS_READWRITE, &nh) == ESP_OK) {
        uint16_t b = 0;
        nvs_get_u16(nh, "boot_id", &b);
        s_boot_id = (uint16_t)(b + 1);
        nvs_set_u16(nh, "boot_id", s_boot_id);
        nvs_commit(nh);
        nvs_close(nh);
    }

    s_fp = fopen(NODE_LOG_PATH, "r+b");
    if (!s_fp) {
        /* First boot only. Create it, then fsync so the directory entry is on
         * flash before anything is written into it. */
        s_fp = fopen(NODE_LOG_PATH, "w+b");
        if (s_fp) {
            fflush(s_fp);
            fsync(fileno(s_fp));
        }
    }
    if (!s_fp) {
        ESP_LOGW(TAG, "cannot open %s; logging DISABLED", NODE_LOG_PATH);
        esp_vfs_fat_spiflash_unmount_rw_wl(NODE_LOG_MOUNT, s_wl);
        s_wl = WL_INVALID_HANDLE;
        return ESP_FAIL;
    }

    /* Recover head/seq by scanning. The ring has no superblock deliberately:
     * a superblock is one more thing a power cut can leave inconsistent, and
     * the scan is a one-off cost at boot. */
    uint8_t rec[NODE_LOG_RECORD_SIZE];
    uint32_t best_seq = 0;
    for (size_t i = 0; i < NODE_LOG_MAX_RECORDS; i++) {
        if (fseek(s_fp, (long)(i * NODE_LOG_RECORD_SIZE), SEEK_SET) != 0) break;
        if (fread(rec, 1, sizeof(rec), s_fp) != sizeof(rec)) break;
        node_log_hdr_t *h = (node_log_hdr_t *)rec;
        if (h->type == 0) continue;
        uint16_t got = h->crc16;
        h->crc16 = 0;
        if (crc16(rec, sizeof(rec)) != got) continue;
        s_count++;
        if (h->seq > best_seq) {
            best_seq = h->seq;
            s_head = (i + 1) % NODE_LOG_MAX_RECORDS;
        }
    }
    s_seq = best_seq;
    s_active = true;
    ESP_LOGI(TAG, "logging active: boot_id=%u recovered=%u records, head=%u",
             (unsigned)s_boot_id, (unsigned)s_count, (unsigned)s_head);
    return ESP_OK;
}

bool node_log_is_active(void) { return s_active; }
uint16_t node_log_boot_id(void) { return s_boot_id; }
size_t node_log_count(void) { return s_count; }

void node_log_boot(uint32_t reset_reason, uint32_t prev_uptime_s)
{
    node_log_boot_t b;
    memset(&b, 0, sizeof(b));
    b.reset_reason  = reset_reason;
    b.prev_uptime_s = prev_uptime_s;
    fill_health(&b.health);
    append(NODE_LOG_TYPE_BOOT, &b, sizeof(b));
}

void node_log_periodic(void)
{
    int64_t now = esp_timer_get_time();
    if (s_last_periodic_us != 0
        && (now - s_last_periodic_us) < (int64_t)NODE_LOG_MIN_PERIOD_S * 1000000) {
        return;
    }
    s_last_periodic_us = now;
    node_log_periodic_t h;
    fill_health(&h);
    append(NODE_LOG_TYPE_PERIODIC, &h, sizeof(h));
}

void node_log_event(uint8_t subtype, int32_t a, int32_t b)
{
    int64_t now = esp_timer_get_time();
    if (s_last_event_us != 0
        && (now - s_last_event_us) < (int64_t)NODE_LOG_MIN_EVENT_MS * 1000) {
        return;
    }
    s_last_event_us = now;
    node_log_event_t e;
    memset(&e, 0, sizeof(e));
    e.subtype = subtype;
    e.a = a;
    e.b = b;
    append(NODE_LOG_TYPE_EVENT, &e, sizeof(e));
}

size_t node_log_read(uint8_t *out, size_t max_records, size_t skip)
{
    if (!s_active || !out || max_records == 0) return 0;
    if (xSemaphoreTake(s_lock, pdMS_TO_TICKS(500)) != pdTRUE) return 0;

    size_t written = 0;
    for (size_t n = 0; n < s_count && written < max_records; n++) {
        if (n < skip) continue;
        /* newest-first: step back from head */
        size_t idx = (s_head + NODE_LOG_MAX_RECORDS - 1 - n) % NODE_LOG_MAX_RECORDS;
        if (fseek(s_fp, (long)(idx * NODE_LOG_RECORD_SIZE), SEEK_SET) != 0) break;
        if (fread(out + written * NODE_LOG_RECORD_SIZE, 1, NODE_LOG_RECORD_SIZE, s_fp)
            != NODE_LOG_RECORD_SIZE) break;
        written++;
    }
    xSemaphoreGive(s_lock);
    return written;
}

esp_err_t node_log_clear(void)
{
    if (!s_active) return ESP_ERR_INVALID_STATE;
    if (xSemaphoreTake(s_lock, pdMS_TO_TICKS(1000)) != pdTRUE) return ESP_ERR_TIMEOUT;
    esp_err_t err = ESP_OK;
    if (s_fp) { fclose(s_fp); s_fp = NULL; }
    s_fp = fopen(NODE_LOG_PATH, "w+b");
    if (!s_fp) { s_active = false; err = ESP_FAIL; }
    s_count = 0;
    s_head = 0;
    s_seq = 0;
    xSemaphoreGive(s_lock);
    return err;
}

#else /* CONFIG_ESPTOOLPY_FLASHSIZE_4MB */

esp_err_t node_log_init(void) { return ESP_ERR_NOT_SUPPORTED; }
bool node_log_is_active(void) { return false; }
void node_log_boot(uint32_t reset_reason, uint32_t prev_uptime_s)
{
    (void)reset_reason;
    (void)prev_uptime_s;
}
void node_log_periodic(void) { }
void node_log_event(uint8_t subtype, int32_t a, int32_t b)
{
    (void)subtype;
    (void)a;
    (void)b;
}
void node_log_note_disconnect(uint8_t reason, int8_t rssi)
{
    (void)reason;
    (void)rssi;
}
size_t node_log_read(uint8_t *out, size_t max_records, size_t skip)
{
    (void)out;
    (void)max_records;
    (void)skip;
    return 0;
}
size_t node_log_count(void) { return 0; }
uint16_t node_log_boot_id(void) { return 0; }
esp_err_t node_log_clear(void) { return ESP_ERR_NOT_SUPPORTED; }

#endif /* CONFIG_ESPTOOLPY_FLASHSIZE_4MB */
