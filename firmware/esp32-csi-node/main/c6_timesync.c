/**
 * @file c6_timesync.c
 * @brief 802.15.4 mesh time-sync skeleton — ADR-110 Phase 4.
 *
 * P4 ships the API surface, role election, and the leader-broadcast +
 * follower-receive paths using esp_ieee802154 raw frames. Full
 * OpenThread MTD attachment with a real network key is deferred to a
 * follow-up turn — the skeleton already exercises the radio init and
 * the offset-tracking math.
 *
 * Beacon frame layout (12 bytes payload + 802.15.4 MAC header):
 *   [0..3]   Magic        0x54534D45  ('TSME' — Time Sync MEsh)
 *   [4]      Protocol ver 0x01
 *   [5]      Leader flag  1 if sender is current leader
 *   [6..7]   Reserved
 *   [8..15]  Leader epoch µs (LE u64)
 */

#include "sdkconfig.h"

#if (defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)) && defined(CONFIG_IEEE802154_ENABLED)

#include "c6_timesync.h"
#include "esp_log.h"
#include "esp_mac.h"
#include "esp_timer.h"
#include "esp_ieee802154.h"
#include "esp_idf_version.h"
#if CONFIG_ESP_COEX_SW_COEXIST_ENABLE
#include "esp_coexist.h"
#endif
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/timers.h"
#include <string.h>

static const char *TAG = "c6_ts";

#define TS_MAGIC        0x54534D45u
#define TS_PROTO_VER    0x01
#define TS_BEACON_MS    100
#define TS_VALID_WINDOW_MS  3000   /* drop to invalid if no beacon in 3 s */

typedef struct __attribute__((packed)) {
    uint32_t magic;
    uint8_t  proto_ver;
    uint8_t  leader_flag;
    uint16_t _reserved;
    uint64_t leader_epoch_us;
} ts_beacon_t;

static uint64_t s_local_eui    = 0;
static uint64_t s_leader_eui   = 0;       /* 0 = unknown */
static volatile int64_t  s_offset_us    = 0;       /* leader_us - local_us */
static volatile uint64_t s_last_seen_us = 0;
static volatile bool     s_is_leader    = false;
/* Set in the RX ISR, logged from the beacon timer task (no logging in ISR). */
static volatile bool     s_step_down_pending = false;
/* First frame that fails the beacon check, copied in the ISR and dumped once
 * from the timer task, to tell foreign 15.4 traffic from a layout mismatch. */
static uint8_t           s_odd_frame[24];
static volatile uint8_t  s_odd_len = 0;
static volatile bool     s_odd_pending = false, s_odd_logged = false;
static uint8_t  s_channel      = 15;
static TimerHandle_t s_beacon_timer = NULL;

/* IEEE EUI-64 from a 6-byte MAC-48: insert 0xFFFE between bytes 2 and 3.
 * Used only as a fallback when esp_read_mac(..., ESP_MAC_IEEE802154) is
 * unavailable. The C6's native call returns 8 bytes already in EUI-64
 * format, so prefer that path (see c6_timesync_init). */
static uint64_t mac48_to_eui64(const uint8_t mac[6])
{
    return ((uint64_t)mac[0] << 56) | ((uint64_t)mac[1] << 48) |
           ((uint64_t)mac[2] << 40) | ((uint64_t)0xFF   << 32) |
           ((uint64_t)0xFE   << 24) | ((uint64_t)mac[3] << 16) |
           ((uint64_t)mac[4] << 8 ) |  (uint64_t)mac[5];
}

/* Pack 8 already-EUI-64 bytes into a uint64. */
static uint64_t eui64_bytes_to_u64(const uint8_t eui[8])
{
    return ((uint64_t)eui[0] << 56) | ((uint64_t)eui[1] << 48) |
           ((uint64_t)eui[2] << 40) | ((uint64_t)eui[3] << 32) |
           ((uint64_t)eui[4] << 24) | ((uint64_t)eui[5] << 16) |
           ((uint64_t)eui[6] << 8 ) |  (uint64_t)eui[7];
}

static volatile uint32_t s_tx_count = 0;
static volatile uint32_t s_tx_fail  = 0;
static volatile uint32_t s_tx_done_fail = 0;
static volatile uint32_t s_rx_count = 0;
static volatile uint32_t s_rx_magic_match = 0;
static volatile uint32_t s_tx_skipped_busy = 0;
/* esp_ieee802154_transmit() is asynchronous: the radio reads the buffer after
 * the call returns. A stack buffer here was reused before the radio read it,
 * so peers received garbage (RAM/register addresses) after a valid header.
 * Keep the frame in a static buffer, untouched until transmit_done/failed. */
static uint8_t           s_tx_buf[64];
static volatile bool     s_tx_in_flight = false;

static void send_beacon(void)
{
    uint8_t frame[32];
    /* Minimal 802.15.4 MAC header: FCF + seq + dst PAN + dst short addr. */
    frame[0] = 0x41;            /* FCF lo: data frame, no security, no ack */
    frame[1] = 0x88;            /* FCF hi: short addrs, intra-PAN */
    frame[2] = 0x00;            /* seq number — placeholder */
    /* Empirically (rx#0 over 60s on all 3 boards), the IDF v5.4 receiver
     * was rejecting the dst-PAN-broadcast (0xFFFF) frames even in
     * promiscuous mode. Match our configured PAN ID 0xCAFE here — short
     * dst stays 0xFFFF for intra-PAN broadcast. PAN bytes are LE. */
    frame[3] = 0xFE; frame[4] = 0xCA;  /* dst PAN = 0xCAFE (matches local) */
    frame[5] = 0xFF; frame[6] = 0xFF;  /* dst short broadcast */
    frame[7] = 0x00; frame[8] = 0x00;  /* src short = 0x0000 */
    ts_beacon_t *b = (ts_beacon_t *)&frame[9];
    b->magic           = TS_MAGIC;
    b->proto_ver       = TS_PROTO_VER;
    b->leader_flag     = 1;
    b->_reserved       = 0;
    b->leader_epoch_us = (uint64_t)esp_timer_get_time();
    size_t total = 9 + sizeof(ts_beacon_t);
    /* ESP-IDF esp_ieee802154 transmit: first byte is the PHY length. */
    if (s_tx_in_flight) {           /* previous beacon still owned by the radio */
        s_tx_skipped_busy++;
        return;
    }
    s_tx_buf[0] = (uint8_t)(total + 2);  /* +2 for FCS appended by HW */
    memcpy(&s_tx_buf[1], frame, total);
    s_tx_in_flight = true;
    esp_err_t r = esp_ieee802154_transmit(s_tx_buf, false);
    s_tx_count++;
    if (r != ESP_OK) {
        s_tx_fail++;
        s_tx_in_flight = false;
    }
    /* Diag log every 10 beacons. */
    if ((s_tx_count % 10) == 1) {
        ESP_LOGI(TAG, "tx#%lu (fail=%lu, air_fail=%lu, busy_skip=%lu) rx#%lu (magic_match=%lu) is_leader=%d",
                 (unsigned long)s_tx_count, (unsigned long)s_tx_fail,
                 (unsigned long)s_tx_done_fail, (unsigned long)s_tx_skipped_busy,
                 (unsigned long)s_rx_count, (unsigned long)s_rx_magic_match,
                 (int)s_is_leader);
    }
}

/* 802.15.4 time-sync (ADR-383, 2026-10-05). The esp_ieee802154 callbacks
 * below run in ISR context. Four bugs made this path look dead on C6 (#762)
 * and C5; all fixed here, verified on two ESP32-C5 boards (IDF 5.5.2):
 *  1. RX was armed once at init and rx_when_idle was never set, so after
 *     the first beacon TX the radio went idle. rx_when_idle is now set.
 *     As a defensive extra, RX is also re-armed from task context after
 *     every TX (transmit_done/failed defer rearm_rx() to the timer task;
 *     calling receive() in the ISR itself is what used to bootloop). One
 *     run of a minimal app saw RX not resume after TX despite rx_when_idle,
 *     but three reruns without the re-arm (IDF 5.5.2 and 5.5.4) did not
 *     reproduce it.
 *  2. receive_done called ESP_LOGI(); taking the log lock in ISR context
 *     aborts (lock_acquire_generic). Logging moved to the timer task.
 *  3. The beacon was built in a stack buffer, but esp_ieee802154_transmit()
 *     is asynchronous, so the radio sent whatever reused that stack: peers
 *     got a valid header followed by RAM/register addresses. Static buffer.
 *  4. Wi-Fi + 15.4 coexistence was never enabled. With 15.4 in RX the STA
 *     could not authenticate or even find the AP (reason 2, then 201).
 *     esp_coex_wifi_i154_enable() now runs in c6_timesync_init(), before
 *     Wi-Fi starts.
 * Result: STA joins in ~6 s; the follower received 708/708 beacons intact.
 * Cost: CSI callback yield drops from ~40 to ~27 pps while 15.4 shares the
 * radio, so C6_TIMESYNC_ENABLE stays off by default; ESP-NOW is the default
 * time-sync transport. esp_ieee802154_receive_handle_done() only releases
 * the RX buffer. */
void esp_ieee802154_receive_done(uint8_t *frame, esp_ieee802154_frame_info_t *frame_info)
{
    s_rx_count++;
    /* PHY length is frame[0]; payload starts at frame[1]. */
    if (frame == NULL) return;
    const ts_beacon_t *b = (const ts_beacon_t *)&frame[1 + 9];
    if (frame[0] < (9 + sizeof(ts_beacon_t) + 2) ||
        b->magic != TS_MAGIC || b->proto_ver != TS_PROTO_VER) {
        if (!s_odd_logged && !s_odd_pending) {
            uint8_t n = frame[0] + 1 < sizeof(s_odd_frame) ? frame[0] + 1 : sizeof(s_odd_frame);
            memcpy(s_odd_frame, frame, n);
            s_odd_len = n;
            s_odd_pending = true;
        }
        esp_ieee802154_receive_handle_done(frame);
        return;
    }
    s_rx_magic_match++;
    uint64_t now = (uint64_t)esp_timer_get_time();
    if (b->leader_flag) {
        /* Adopt this leader if its EUI is lower than ours (or unknown). */
        if (s_leader_eui == 0 || b->leader_epoch_us > 0) {
            s_offset_us    = (int64_t)b->leader_epoch_us - (int64_t)now;
            s_last_seen_us = now;
            if (s_is_leader) {
                /* Step down — somebody else is broadcasting; lowest EUI wins
                 * (deferred — for now last-heard wins). */
                s_is_leader = false;
                s_step_down_pending = true;   /* logged from the timer task */
            }
        }
    }
    /* Release the RX buffer; rx_when_idle keeps the radio in RX. */
    esp_ieee802154_receive_handle_done(frame);
}

/* Runs in the timer daemon task (deferred from transmit_done/failed). */
static void rearm_rx(void *arg1, uint32_t arg2)
{
    (void)arg1; (void)arg2;
    esp_ieee802154_receive();
}

static void IRAM_ATTR defer_rearm_rx_from_isr(void)
{
    BaseType_t woken = pdFALSE;
    xTimerPendFunctionCallFromISR(rearm_rx, NULL, 0, &woken);
    portYIELD_FROM_ISR(woken);
}

void esp_ieee802154_transmit_done(const uint8_t *frame,
                                  const uint8_t *ack,
                                  esp_ieee802154_frame_info_t *ack_frame_info)
{
    (void)frame; (void)ack_frame_info;
    /* Beacons are broadcast without an ACK request, but if an ACK frame is
     * ever handed over, its buffer must be released (esp_ieee802154.h). */
    if (ack) esp_ieee802154_receive_handle_done(ack);
    s_tx_in_flight = false;
    defer_rearm_rx_from_isr();
}

void esp_ieee802154_transmit_failed(const uint8_t *frame, esp_ieee802154_tx_error_t error)
{
    (void)frame; (void)error;
    s_tx_done_fail++;   /* ISR context: count only, no logging */
    s_tx_in_flight = false;
    defer_rearm_rx_from_isr();
}

static void beacon_timer_cb(TimerHandle_t t)
{
    (void)t;
    uint64_t now = (uint64_t)esp_timer_get_time();
    if (s_step_down_pending) {
        s_step_down_pending = false;
        ESP_LOGI(TAG, "stepping down — heard another leader beacon");
    }
    if (s_odd_pending) {
        s_odd_pending = false;
        s_odd_logged = true;
        ESP_LOGI(TAG, "first non-beacon frame (%u bytes incl. PHY len):", (unsigned)s_odd_len);
        ESP_LOG_BUFFER_HEX(TAG, s_odd_frame, s_odd_len);
    }
    if (s_is_leader) {
        send_beacon();
    } else if ((now - s_last_seen_us) > (TS_VALID_WINDOW_MS * 1000ULL)) {
        /* Lost the leader — promote self if no one else takes over in 1 s. */
        s_is_leader = true;
        s_leader_eui = s_local_eui;
        ESP_LOGI(TAG, "promoting self to time-leader (no beacons for %u ms)",
                 (unsigned)TS_VALID_WINDOW_MS);
    }
}

esp_err_t c6_timesync_init(uint8_t channel)
{
    /* esp_mac.h: ESP_MAC_IEEE802154 returns 8 bytes ALREADY in EUI-64 format
     * (ff:fe is pre-inserted in bytes 3-4 from the eFuse MAC_EXT). Using a
     * 6-byte buffer here truncates and then double-inserts ff:fe — the bug
     * we hit on the first run (boot log: EUI=206ef1fffefffe17).
     *
     * Correct path: read 8 bytes, pack into uint64 unchanged. Fallback to
     * the base MAC + manual EUI-64 derivation if the 8-byte read errors. */
    uint8_t eui_bytes[8] = {0};
    esp_err_t mac_ret = esp_read_mac(eui_bytes, ESP_MAC_IEEE802154);
    if (mac_ret == ESP_OK) {
        s_local_eui = eui64_bytes_to_u64(eui_bytes);
    } else {
        uint8_t base_mac[6];
        esp_read_mac(base_mac, ESP_MAC_BASE);
        s_local_eui = mac48_to_eui64(base_mac);
    }
    /* Use the 6-byte base MAC for the IEEE 802.15.4 extended address — the
     * radio expects MAC-48-style bytes here, not the EUI-64 derivation. */
    uint8_t mac[6];
    esp_read_mac(mac, ESP_MAC_BASE);
    s_channel   = (channel >= 11 && channel <= 26) ? channel : 15;

#if CONFIG_ESP_COEX_SW_COEXIST_ENABLE
    /* Wi-Fi + 15.4 coexistence must be switched on explicitly (as Espressif's
     * ot_br and zigbee_gateway examples do). Without it, 15.4 RX starved the
     * Wi-Fi STA (reason 2, then 201 NO_AP_FOUND). Must run before Wi-Fi
     * starts; c6_timesync_init() runs before wifi_init_sta(). */
    esp_err_t coex_ret = esp_coex_wifi_i154_enable();
    ESP_LOGI(TAG, "esp_coex_wifi_i154_enable: %s", esp_err_to_name(coex_ret));
#endif
    esp_err_t ret = esp_ieee802154_enable();
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "ieee802154_enable failed: %s", esp_err_to_name(ret));
        return ret;
    }
    /* promiscuous=true so we accept broadcast frames addressed to 0xFFFF.
     * In non-promiscuous mode the radio filters to frames addressed to
     * our short or extended address. Our beacon protocol uses broadcast. */
    esp_ieee802154_set_promiscuous(true);
    esp_ieee802154_set_panid(0xCAFE);
    esp_ieee802154_set_short_address(0x0000);
    esp_ieee802154_set_extended_address(mac);
    esp_ieee802154_set_channel(s_channel);
#if CONFIG_ESP_COEX_SW_COEXIST_ENABLE && ESP_IDF_VERSION >= ESP_IDF_VERSION_VAL(5, 5, 0)
    /* (set_coex_config is declared in esp_ieee802154.h from IDF 5.5.)
     * The C5/C6 share one RF front end between Wi-Fi and 15.4. Parked in RX,
     * 15.4 starved Wi-Fi (join took ~3 min). Give 15.4 idle-RX the lowest
     * priority and TX/RX low, so Wi-Fi (and CSI capture) wins arbitration. */
    esp_ieee802154_coex_config_t coex = {
        .idle    = IEEE802154_IDLE,
        .txrx    = IEEE802154_LOW,
        .txrx_at = IEEE802154_MIDDLE,
    };
    esp_ieee802154_set_coex_config(coex);
#endif
    /* Stay in RX whenever not transmitting (see the RX path note above). */
    esp_ieee802154_set_rx_when_idle(true);
    esp_ieee802154_receive();

    /* Start as candidate leader; first received beacon will demote us if needed. */
    s_is_leader    = true;
    s_leader_eui   = s_local_eui;
    s_last_seen_us = (uint64_t)esp_timer_get_time();

    s_beacon_timer = xTimerCreate("c6ts_beacon", pdMS_TO_TICKS(TS_BEACON_MS),
                                  pdTRUE, NULL, beacon_timer_cb);
    if (s_beacon_timer == NULL) {
        ESP_LOGE(TAG, "xTimerCreate failed");
        return ESP_ERR_NO_MEM;
    }
    xTimerStart(s_beacon_timer, 0);

    ESP_LOGI(TAG, "init done: channel=%u EUI=%016llx leader=yes(candidate)",
             (unsigned)s_channel, (unsigned long long)s_local_eui);
    return ESP_OK;
}

uint64_t c6_timesync_get_epoch_us(void)
{
    return (uint64_t)((int64_t)esp_timer_get_time() + s_offset_us);
}

bool c6_timesync_is_leader(void) { return s_is_leader; }
int64_t c6_timesync_get_offset_us(void) { return s_offset_us; }

bool c6_timesync_is_valid(void)
{
    if (s_is_leader) return true;
    uint64_t now = (uint64_t)esp_timer_get_time();
    return (now - s_last_seen_us) < (TS_VALID_WINDOW_MS * 1000ULL);
}

#endif  /* CONFIG_IDF_TARGET_ESP32C6 && CONFIG_IEEE802154_ENABLED */
