/**
 * @file c6_sync_espnow.c
 * @brief ESP-NOW cross-node time-sync — ADR-110 D1 workaround.
 *
 * Same protocol as c6_timesync.c (TS_BEACON every 100 ms with leader epoch),
 * but over ESP-NOW instead of 802.15.4 because the IDF v5.4 ieee802154 RX
 * path doesn't deliver frames to user-space (see WITNESS-LOG-110 §D1).
 *
 * Frame layout (16 bytes payload, broadcast MAC FF:FF:FF:FF:FF:FF):
 *   [0..3]   Magic         0x53454E50  ('SENP' — Sync via ESP-NOW)
 *   [4]      Protocol ver  0x01
 *   [5]      Leader flag   1 if sender claims leader
 *   [6..7]   Reserved
 *   [8..15]  Leader epoch µs (LE u64)
 */

#include "sdkconfig.h"
#include "c6_sync_espnow.h"
#include "esp_log.h"
#include "esp_now.h"
#include "esp_wifi.h"
#include "esp_mac.h"
#include "esp_timer.h"
#include "esp_idf_version.h"
#include "freertos/FreeRTOS.h"
#include "freertos/timers.h"
#include "nvs_config.h"   /* g_nvs_config: fleet size and beacon override */
#include <string.h>

static const char *TAG = "c6_espnow";

#define BEACON_MAGIC      0x53454E50u   /* 'SENP' little-endian */
#define BEACON_PROTO_VER  0x01
#define BEACON_PERIOD_MS  100

/* Sizing constants for the derivation below.
 *
 * BEACON_AP_BUDGET_FPS is what the associated AP contributes to the same
 * 50 Hz receive gate — measured 10-15 fps on this fleet, taken at the
 * pessimistic end so the derived period errs slow.
 *
 * BEACON_MARGIN_NUM/DEN is the headroom over the bare inequality. At three
 * nodes the inequality demands >= 57 ms and the value proven on hardware is
 * 80 ms, which is where 7/5 comes from — the derivation returns 81 ms at N=3,
 * so it reproduces the measured setting rather than being fitted after it.
 *
 * Across fleet sizes this holds the offered load at ~40 fps against the 50 fps
 * gate — 3 nodes 81 ms, 6 nodes 200 ms, 9 nodes 320 ms, 13 nodes 480 ms — so
 * every size keeps the same 10 fps of headroom rather than one being tuned
 * and the rest inheriting whatever falls out. */
#define BEACON_AP_BUDGET_FPS   15
#define BEACON_GATE_FPS        50
#define BEACON_MARGIN_NUM       7
#define BEACON_MARGIN_DEN       5
#define BEACON_PERIOD_MIN_MS   20
#define BEACON_PERIOD_MAX_MS 2000

static uint32_t s_beacon_period_ms = BEACON_PERIOD_MS;

/* Effective beacon period for this node.
 *
 * An explicit NVS `beacon_ms` wins, for benching. Otherwise derive from the
 * provisioned fleet size, because that is the quantity the period actually
 * depends on and it is already provisioned for TDM:
 *
 *     period >= (N - 1) * 1000 / (GATE - AP_BUDGET)   then * margin
 *
 * N=3 -> 80 ms (the measured value), N=9 -> 320 ms, N=13 -> 480 ms.
 *
 * A fleet-size change is therefore a re-provision, never a reflash — which
 * is the whole point, since getting this wrong does not degrade gracefully:
 * an over-fast beacon overruns the receive gate, 60-90% of peer frames are
 * discarded at random, and the transmit queue can wedge outright. */
uint32_t c6_sync_espnow_period_ms(void)
{
    if (g_nvs_config.beacon_period_ms != 0) {
        return g_nvs_config.beacon_period_ms;
    }
    uint32_t n = g_nvs_config.tdm_node_count;
    if (n < 2) {
        /* Unprovisioned or a single node: nobody to collide with. */
        return BEACON_PERIOD_MS;
    }
    uint32_t budget = BEACON_GATE_FPS - BEACON_AP_BUDGET_FPS;      /* 35 fps */
    uint32_t base   = ((n - 1) * 1000U + budget - 1) / budget;     /* ceil */
    uint32_t period = (base * BEACON_MARGIN_NUM) / BEACON_MARGIN_DEN;
    if (period < BEACON_PERIOD_MIN_MS) period = BEACON_PERIOD_MIN_MS;
    if (period > BEACON_PERIOD_MAX_MS) period = BEACON_PERIOD_MAX_MS;
    return period;
}
#define VALID_WINDOW_MS   3000

typedef struct __attribute__((packed)) {
    uint32_t magic;
    uint8_t  proto_ver;
    uint8_t  leader_flag;
    uint16_t _reserved;
    uint64_t leader_epoch_us;
} espnow_beacon_t;

static const uint8_t s_broadcast_mac[6] = {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF};

static uint64_t s_local_id     = 0;   /* 6-byte MAC packed into u64 */
static uint64_t s_leader_id    = 0;
static int64_t  s_offset_us    = 0;
static uint64_t s_last_seen_us = 0;
static bool     s_is_leader    = false;
static TimerHandle_t s_beacon_timer = NULL;

static uint32_t s_tx_count = 0;
static uint32_t s_tx_fail  = 0;
static uint32_t s_rx_count = 0;
static uint32_t s_rx_magic_match = 0;

/* ADR-110 P10 — EMA-smoothed offset (host-side trajectory in firmware).
 *
 * The §A0.8 four-minute soak measured 540 µs sample-stdev around a true
 * offset that drifts at ≈1.4 ppm between two C6 crystals. An exponential
 * moving average with α=0.125 (Q3.3 fixed-point shift = 3) yields an
 * effective ~8-sample window, fast enough to track the drift (~7 µs/sec
 * worst-case) while suppressing the per-beacon WiFi-MAC jitter.
 *
 * Two consumers: get_offset_us() (raw, unchanged — for diagnostics) and
 * get_offset_us_smoothed() (filtered — what CSI frames should stamp).
 * Both expose `int64_t` so call sites stay identical. */
#define OFFSET_EMA_SHIFT 3           /* α = 1/8 = 0.125 */
static int64_t s_offset_us_smoothed = 0;
static bool    s_smoothed_seeded    = false;

static uint64_t mac6_to_u64(const uint8_t mac[6])
{
    return ((uint64_t)mac[0] << 40) | ((uint64_t)mac[1] << 32) |
           ((uint64_t)mac[2] << 24) | ((uint64_t)mac[3] << 16) |
           ((uint64_t)mac[4] <<  8) |  (uint64_t)mac[5];
}

static void send_beacon(void)
{
    espnow_beacon_t b = {
        .magic           = BEACON_MAGIC,
        .proto_ver       = BEACON_PROTO_VER,
        .leader_flag     = s_is_leader ? 1 : 0,
        ._reserved       = 0,
        .leader_epoch_us = (uint64_t)esp_timer_get_time(),
    };
    esp_err_t r = esp_now_send(s_broadcast_mac, (uint8_t *)&b, sizeof(b));
    s_tx_count++;
    if (r != ESP_OK) s_tx_fail++;
    /* Diag log every 50 beacons. */
    if ((s_tx_count % 50) == 1) {
        ESP_LOGI(TAG, "tx#%lu (fail=%lu) rx#%lu (match=%lu) leader=%d offset_us=%lld smoothed=%lld",
                 (unsigned long)s_tx_count, (unsigned long)s_tx_fail,
                 (unsigned long)s_rx_count, (unsigned long)s_rx_magic_match,
                 (int)s_is_leader, (long long)s_offset_us,
                 (long long)s_offset_us_smoothed);
    }
}

/* IDF v5.4 ESP-NOW recv callback signature uses esp_now_recv_info_t.
 * Falls back to the older signature on older IDF via ifdef. */
#if ESP_IDF_VERSION >= ESP_IDF_VERSION_VAL(5, 0, 0)
static void on_recv(const esp_now_recv_info_t *info,
                    const uint8_t *data, int len)
{
    const uint8_t *src_mac = info ? info->src_addr : NULL;
#else
static void on_recv(const uint8_t *src_mac, const uint8_t *data, int len)
{
#endif
    s_rx_count++;
    if (data == NULL || len < (int)sizeof(espnow_beacon_t)) return;
    const espnow_beacon_t *b = (const espnow_beacon_t *)data;
    if (b->magic != BEACON_MAGIC || b->proto_ver != BEACON_PROTO_VER) return;
    s_rx_magic_match++;
    uint64_t sender_id = src_mac ? mac6_to_u64(src_mac) : 0;
    uint64_t now_us    = (uint64_t)esp_timer_get_time();

    /* Adopt sender as leader if it's claiming leadership AND its ID is
     * lower than our current leader (or we have no leader). Lowest MAC
     * wins — deterministic. */
    if (b->leader_flag && (s_leader_id == 0 || sender_id < s_leader_id)) {
        if (s_is_leader && sender_id < s_local_id) {
            ESP_LOGI(TAG, "stepping down: heard lower-id leader %012llx (we are %012llx)",
                     (unsigned long long)sender_id, (unsigned long long)s_local_id);
            s_is_leader = false;
        }
        s_leader_id = sender_id;
    }

    /* If accepted leader, compute offset from their epoch (only for non-leader). */
    if (b->leader_flag && !s_is_leader && sender_id == s_leader_id) {
        int64_t raw = (int64_t)b->leader_epoch_us - (int64_t)now_us;
        s_offset_us    = raw;
        s_last_seen_us = now_us;
        /* EMA: y[n] = y[n-1] + (raw - y[n-1]) >> SHIFT */
        if (!s_smoothed_seeded) {
            s_offset_us_smoothed = raw;
            s_smoothed_seeded    = true;
        } else {
            s_offset_us_smoothed += (raw - s_offset_us_smoothed) >> OFFSET_EMA_SHIFT;
        }
    }
}

/* Issue #944: ESP-IDF v6.0 changed `esp_now_send_cb_t` from
 *   void (*)(const uint8_t *mac, esp_now_send_status_t status)
 * to
 *   void (*)(const esp_now_send_info_t *tx_info, esp_now_send_status_t status)
 * Both signatures ignore the address-side argument here — we only inspect
 * `status` to bump the TX-fail counter — so the body is identical; only the
 * function-pointer type differs.
 *
 * Issue #1005: Espressif backported the new signature to v5.5
 * (`esp_now_send_info_t` = typedef of `wifi_tx_info_t` there), so the guard
 * must be the full version triple, not ESP_IDF_VERSION_MAJOR.
 */
#if ESP_IDF_VERSION >= ESP_IDF_VERSION_VAL(5, 5, 0)
static void on_send(const esp_now_send_info_t *tx_info, esp_now_send_status_t status)
{
    (void)tx_info;
    if (status != ESP_NOW_SEND_SUCCESS) s_tx_fail++;
}
#else
static void on_send(const uint8_t *mac, esp_now_send_status_t status)
{
    (void)mac;
    if (status != ESP_NOW_SEND_SUCCESS) s_tx_fail++;
}
#endif

static void beacon_timer_cb(TimerHandle_t t)
{
    (void)t;
    uint64_t now = (uint64_t)esp_timer_get_time();
    /* Promote self if no leader beacon for VALID_WINDOW_MS and we have lowest known id. */
    if (!s_is_leader && (now - s_last_seen_us) > (VALID_WINDOW_MS * 1000ULL)) {
        if (s_leader_id == 0 || s_local_id < s_leader_id) {
            s_is_leader  = true;
            s_leader_id  = s_local_id;
            s_offset_us  = 0;
            ESP_LOGI(TAG, "promoting self to leader (no beacons for %u ms; local_id=%012llx)",
                     (unsigned)VALID_WINDOW_MS, (unsigned long long)s_local_id);
        }
    }
    send_beacon();
}

esp_err_t c6_sync_espnow_init(void)
{
    uint8_t mac[6];
    esp_read_mac(mac, ESP_MAC_WIFI_STA);
    s_local_id = mac6_to_u64(mac);

    esp_err_t r = esp_now_init();
    if (r != ESP_OK) {
        ESP_LOGE(TAG, "esp_now_init failed: %s", esp_err_to_name(r));
        return r;
    }
    esp_now_register_recv_cb(on_recv);
    esp_now_register_send_cb(on_send);

    /* Add broadcast peer so esp_now_send to FF:FF:FF:FF:FF:FF works. */
    esp_now_peer_info_t peer = {0};
    memcpy(peer.peer_addr, s_broadcast_mac, 6);
    peer.channel = 0;        /* current STA channel */
    peer.ifidx   = WIFI_IF_STA;
    peer.encrypt = false;
    r = esp_now_add_peer(&peer);
    if (r != ESP_OK && r != ESP_ERR_ESPNOW_EXIST) {
        ESP_LOGW(TAG, "esp_now_add_peer(broadcast) failed: %s", esp_err_to_name(r));
    }

    /* Start as candidate leader — will step down on receiving lower-id beacon. */
    s_is_leader    = true;
    s_leader_id    = s_local_id;
    s_last_seen_us = (uint64_t)esp_timer_get_time();

    s_beacon_period_ms = c6_sync_espnow_period_ms();

    s_beacon_timer = xTimerCreate("c6_espnow_beacon",
                                  pdMS_TO_TICKS(s_beacon_period_ms),
                                  pdTRUE, NULL, beacon_timer_cb);
    if (s_beacon_timer == NULL) {
        ESP_LOGE(TAG, "xTimerCreate failed");
        return ESP_ERR_NO_MEM;
    }
    xTimerStart(s_beacon_timer, 0);

    ESP_LOGI(TAG, "init done: local_id=%012llx leader=yes(candidate) period=%ums",
             (unsigned long long)s_local_id, (unsigned)s_beacon_period_ms);
    return ESP_OK;
}

uint64_t c6_sync_espnow_get_epoch_us(void)
{
    /* Prefer the smoothed offset once we've heard a leader beacon; falls
     * back to raw=0 on the leader board and during the first second after
     * follower boot. The smoothed value is what CSI frames should stamp
     * for cross-board multistatic alignment (§A0.8 measured 540 µs raw
     * stdev → expected <100 µs smoothed with α=1/8 over ~8 samples). */
    int64_t off = s_smoothed_seeded ? s_offset_us_smoothed : s_offset_us;
    return (uint64_t)((int64_t)esp_timer_get_time() + off);
}

bool c6_sync_espnow_is_leader(void) { return s_is_leader; }
int64_t c6_sync_espnow_get_offset_us(void) { return s_offset_us; }
int64_t c6_sync_espnow_get_offset_us_smoothed(void) { return s_offset_us_smoothed; }

bool c6_sync_espnow_is_valid(void)
{
    if (s_is_leader) return true;
    uint64_t now = (uint64_t)esp_timer_get_time();
    return (now - s_last_seen_us) < (VALID_WINDOW_MS * 1000ULL);
}

uint32_t c6_sync_espnow_tx_count(void)       { return s_tx_count; }
uint32_t c6_sync_espnow_tx_fail(void)        { return s_tx_fail; }
uint32_t c6_sync_espnow_rx_count(void)       { return s_rx_count; }
uint32_t c6_sync_espnow_rx_magic_match(void) { return s_rx_magic_match; }
