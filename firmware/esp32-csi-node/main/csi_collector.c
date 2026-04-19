/**
 * @file csi_collector.c
 * @brief CSI data collection and ADR-018 binary frame serialization.
 *
 * Registers the ESP-IDF WiFi CSI callback and serializes incoming CSI data
 * into the ADR-018 binary frame format for UDP transmission.
 *
 * ADR-029 extensions:
 *   - Channel-hop table for multi-band sensing (channels 1/6/11 by default)
 *   - Timer-driven channel hopping at configurable dwell intervals
 *   - NDP frame injection stub for sensing-first TX
 */

#include "csi_collector.h"
#include "nvs_config.h"
#include "stream_sender.h"
#include "edge_processing.h"

#include <string.h>
#include "esp_log.h"
#include "esp_wifi.h"
#include "esp_timer.h"
#include "sdkconfig.h"

/* ADR-060: Access the global NVS config for MAC filter and channel override. */
extern nvs_config_t g_nvs_config;

/* Defensive fix (#232, #375, #385, #386, #390): capture node_id at init-time
 * into a module-local static. Using the global g_nvs_config.node_id directly
 * at every callback is vulnerable to any memory corruption that clobbers the
 * struct (which users have reported reverting node_id to the Kconfig default
 * of 1). The local copy is set once at csi_collector_init() and then used
 * exclusively by csi_serialize_frame(). */
static uint8_t s_node_id = 1;

/* ADR-057: Build-time guard — fail early if CSI is not enabled in sdkconfig.
 * Without this, the firmware compiles but crashes at runtime with:
 *   "E (xxxx) wifi:CSI not enabled in menuconfig!"
 * which is confusing for users flashing pre-built binaries. */
#ifndef CONFIG_ESP_WIFI_CSI_ENABLED
#error "CONFIG_ESP_WIFI_CSI_ENABLED must be set in sdkconfig. " \
       "Run: idf.py menuconfig -> Component config -> Wi-Fi -> Enable WiFi CSI, " \
       "or copy sdkconfig.defaults.template to sdkconfig.defaults before building."
#endif

static const char *TAG = "csi_collector";

static uint32_t s_sequence = 0;
static uint32_t s_cb_count = 0;
static uint32_t s_send_ok = 0;
static uint32_t s_send_fail = 0;
static uint32_t s_rate_skip = 0;

/**
 * Minimum interval between UDP sends in microseconds.
 * CSI callbacks can fire hundreds of times per second in promiscuous mode.
 * We cap the send rate to avoid exhausting lwIP packet buffers (ENOMEM).
 * Default: 20 ms = 50 Hz max send rate.
 */
#define CSI_MIN_SEND_INTERVAL_US  (20 * 1000)
static int64_t s_last_send_us = 0;

/* ---- ADR-029: Channel-hop state ---- */

/** Channel hop table (populated from NVS at boot or via set_hop_table). */
static uint8_t  s_hop_channels[CSI_HOP_CHANNELS_MAX] = {1, 6, 11, 36, 40, 44};

/** Number of active channels in the hop table. 1 = single-channel (no hop). */
static uint8_t  s_hop_count   = 1;

/** Dwell time per channel in milliseconds. */
static uint32_t s_dwell_ms    = 50;

/** Current index into s_hop_channels. */
static uint8_t  s_hop_index   = 0;

/** Handle for the periodic hop timer. NULL when timer is not running. */
static esp_timer_handle_t s_hop_timer = NULL;

/**
 * Serialize CSI data into ADR-018 binary frame format.
 *
 * Layout:
 *   [0..3]   Magic: 0xC5110001 (LE)
 *   [4]      Node ID
 *   [5]      Number of antennas (rx_ctrl.rx_ant + 1 if available, else 1)
 *   [6..7]   Number of subcarriers (LE u16) = len / (2 * n_antennas)
 *   [8..11]  Frequency MHz (LE u32) — derived from channel
 *   [12..15] Sequence number (LE u32)
 *   [16]     RSSI (i8)
 *   [17]     Noise floor (i8)
 *   [18..19] Reserved
 *   [20..]   I/Q data (raw bytes from ESP-IDF callback)
 */
size_t csi_serialize_frame(const wifi_csi_info_t *info, uint8_t *buf, size_t buf_len)
{
    if (info == NULL || buf == NULL || info->buf == NULL) {
        return 0;
    }

    uint8_t n_antennas = 1;  /* ESP32-S3 typically reports 1 antenna for CSI */
    uint16_t iq_len = (uint16_t)info->len;
    uint16_t n_subcarriers = iq_len / (2 * n_antennas);

    size_t frame_size = CSI_HEADER_SIZE + iq_len;
    if (frame_size > buf_len) {
        ESP_LOGW(TAG, "Buffer too small: need %u, have %u", (unsigned)frame_size, (unsigned)buf_len);
        return 0;
    }

    /* Derive frequency from channel number */
    uint8_t channel = info->rx_ctrl.channel;
    uint32_t freq_mhz;
    if (channel >= 1 && channel <= 13) {
        freq_mhz = 2412 + (channel - 1) * 5;
    } else if (channel == 14) {
        freq_mhz = 2484;
    } else if (channel >= 36 && channel <= 177) {
        freq_mhz = 5000 + channel * 5;
    } else {
        freq_mhz = 0;
    }

    /* Magic (LE) */
    uint32_t magic = CSI_MAGIC;
    memcpy(&buf[0], &magic, 4);

    /* Node ID (captured at init into s_node_id to survive memory corruption
     * that could clobber g_nvs_config.node_id - see #232/#375/#385/#390). */
    buf[4] = s_node_id;

    /* Number of antennas */
    buf[5] = n_antennas;

    /* Number of subcarriers (LE u16) */
    memcpy(&buf[6], &n_subcarriers, 2);

    /* Frequency MHz (LE u32) */
    memcpy(&buf[8], &freq_mhz, 4);

    /* Sequence number (LE u32) */
    uint32_t seq = s_sequence++;
    memcpy(&buf[12], &seq, 4);

    /* RSSI (i8) */
    buf[16] = (uint8_t)(int8_t)info->rx_ctrl.rssi;

    /* Noise floor (i8) */
    buf[17] = (uint8_t)(int8_t)info->rx_ctrl.noise_floor;

    /* Reserved */
    buf[18] = 0;
    buf[19] = 0;

    /* I/Q data */
    memcpy(&buf[CSI_HEADER_SIZE], info->buf, iq_len);

    return frame_size;
}

/**
 * WiFi CSI callback — invoked by ESP-IDF when CSI data is available.
 */
static void wifi_csi_callback(void *ctx, wifi_csi_info_t *info)
{
    (void)ctx;

    /* ADR-060: MAC address filtering — drop frames from non-matching sources. */
    if (g_nvs_config.filter_mac_set) {
        if (memcmp(info->mac, g_nvs_config.filter_mac, 6) != 0) {
            return;  /* Source MAC doesn't match filter — skip frame. */
        }
    }

    s_cb_count++;

    if (s_cb_count <= 3 || (s_cb_count % 100) == 0) {
        ESP_LOGI(TAG, "CSI cb #%lu: len=%d rssi=%d ch=%d",
                 (unsigned long)s_cb_count, info->len,
                 info->rx_ctrl.rssi, info->rx_ctrl.channel);
    }

    uint8_t frame_buf[CSI_MAX_FRAME_SIZE];
    size_t frame_len = csi_serialize_frame(info, frame_buf, sizeof(frame_buf));

    if (frame_len > 0) {
        /* Rate-limit UDP sends to avoid ENOMEM from lwIP pbuf exhaustion.
         * In promiscuous mode, CSI callbacks can fire 100-500+ times/sec.
         * We only need 20-50 Hz for the sensing pipeline. */
        int64_t now = esp_timer_get_time();
        if ((now - s_last_send_us) >= CSI_MIN_SEND_INTERVAL_US) {
            int ret = stream_sender_send(frame_buf, frame_len);
            if (ret > 0) {
                s_send_ok++;
                s_last_send_us = now;
            } else {
                s_send_fail++;
                if (s_send_fail <= 5) {
                    ESP_LOGW(TAG, "sendto failed (fail #%lu)", (unsigned long)s_send_fail);
                }
            }
        } else {
            s_rate_skip++;
        }
    }

    /* ADR-039: Enqueue raw I/Q into edge processing ring buffer. */
    if (info->buf && info->len > 0) {
        edge_enqueue_csi((const uint8_t *)info->buf, (uint16_t)info->len,
                         (int8_t)info->rx_ctrl.rssi, info->rx_ctrl.channel);
    }
}

/**
 * Promiscuous mode callback — required for CSI to fire on all received frames.
 * We don't need the packet content, just the CSI triggered by reception.
 */
static void wifi_promiscuous_cb(void *buf, wifi_promiscuous_pkt_type_t type)
{
    /* No-op: CSI callback is registered separately and fires in parallel. */
    (void)buf;
    (void)type;
}

void csi_collector_init(void)
{
    /* Capture node_id into module-local static at init time. After this point
     * csi_serialize_frame() uses s_node_id exclusively, isolating the UDP
     * frame node_id field from any memory corruption of g_nvs_config. */
    s_node_id = g_nvs_config.node_id;
    ESP_LOGI(TAG, "Captured node_id=%u at init (defensive copy for #232/#375/#385/#390)",
             (unsigned)s_node_id);

    /* ADR-060: Determine the CSI channel.
     * Priority: 1) NVS override (--channel), 2) connected AP channel, 3) Kconfig default. */
    uint8_t csi_channel = (uint8_t)CONFIG_CSI_WIFI_CHANNEL;

    if (g_nvs_config.csi_channel > 0) {
        /* Explicit NVS override via provision.py --channel */
        csi_channel = g_nvs_config.csi_channel;
        ESP_LOGI(TAG, "Using NVS channel override: %u", (unsigned)csi_channel);
    } else {
        /* Auto-detect from connected AP */
        wifi_ap_record_t ap_info;
        if (esp_wifi_sta_get_ap_info(&ap_info) == ESP_OK && ap_info.primary > 0) {
            csi_channel = ap_info.primary;
            ESP_LOGI(TAG, "Auto-detected AP channel: %u", (unsigned)csi_channel);
        } else {
            ESP_LOGW(TAG, "Could not detect AP channel, using Kconfig default: %u",
                     (unsigned)csi_channel);
        }
    }

    /* Update the hop table's first channel to match. */
    s_hop_channels[0] = csi_channel;

    /* Enable promiscuous mode — required for reliable CSI callbacks.
     * Without this, CSI only fires on frames destined to this station,
     * which may be very infrequent on a quiet network. */
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous(true));
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous_rx_cb(wifi_promiscuous_cb));

    wifi_promiscuous_filter_t filt = {
        .filter_mask = WIFI_PROMIS_FILTER_MASK_MGMT | WIFI_PROMIS_FILTER_MASK_DATA,
    };
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous_filter(&filt));

    ESP_LOGI(TAG, "Promiscuous mode enabled for CSI capture");

    wifi_csi_config_t csi_config = {
        .lltf_en = true,
        .htltf_en = true,
        .stbc_htltf2_en = true,
        .ltf_merge_en = true,
        .channel_filter_en = false,
        .manu_scale = false,
        .shift = false,
    };

    ESP_ERROR_CHECK(esp_wifi_set_csi_config(&csi_config));
    ESP_ERROR_CHECK(esp_wifi_set_csi_rx_cb(wifi_csi_callback, NULL));
    ESP_ERROR_CHECK(esp_wifi_set_csi(true));

    if (g_nvs_config.filter_mac_set) {
        ESP_LOGI(TAG, "MAC filter active: %02x:%02x:%02x:%02x:%02x:%02x",
                 g_nvs_config.filter_mac[0], g_nvs_config.filter_mac[1],
                 g_nvs_config.filter_mac[2], g_nvs_config.filter_mac[3],
                 g_nvs_config.filter_mac[4], g_nvs_config.filter_mac[5]);
    }

    ESP_LOGI(TAG, "CSI collection initialized (node_id=%u, channel=%u)",
             (unsigned)s_node_id, (unsigned)csi_channel);

    /* Clobber-detection canary: if g_nvs_config.node_id no longer matches the
     * value we captured, something corrupted the struct between nvs_config_load
     * and here. This is the historic #232/#375 symptom. */
    if (g_nvs_config.node_id != s_node_id) {
        ESP_LOGW(TAG, "node_id clobber detected: captured=%u but g_nvs_config=%u "
                 "(frames will use captured value %u). Please report to #390.",
                 (unsigned)s_node_id, (unsigned)g_nvs_config.node_id,
                 (unsigned)s_node_id);
    }
}

/* Accessor for other modules that need the authoritative runtime node_id. */
uint8_t csi_collector_get_node_id(void)
{
    return s_node_id;
}

/* ---- ADR-081: packet yield accessor for the radio abstraction layer ---- */

uint16_t csi_collector_get_pkt_yield_per_sec(void)
{
    /* Simple sliding window: record the callback count at ~1 s ago, return
     * the delta. Called from adaptive_controller's fast loop (200 ms), so
     * we update the snapshot every ~5 calls. */
    static int64_t  s_yield_window_start_us = 0;
    static uint32_t s_yield_window_start_cb = 0;
    static uint16_t s_last_yield            = 0;

    int64_t now = esp_timer_get_time();
    if (s_yield_window_start_us == 0) {
        s_yield_window_start_us = now;
        s_yield_window_start_cb = s_cb_count;
        return 0;
    }
    int64_t elapsed = now - s_yield_window_start_us;
    if (elapsed < 1000000LL) {
        return s_last_yield;
    }
    uint32_t delta = s_cb_count - s_yield_window_start_cb;
    /* Scale back to per-second if the window ran long (shouldn't, but be safe). */
    uint64_t per_sec = ((uint64_t)delta * 1000000ULL) / (uint64_t)elapsed;
    if (per_sec > 0xFFFFu) per_sec = 0xFFFFu;
    s_last_yield            = (uint16_t)per_sec;
    s_yield_window_start_us = now;
    s_yield_window_start_cb = s_cb_count;
    return s_last_yield;
}

uint16_t csi_collector_get_send_fail_count(void)
{
    uint32_t f = s_send_fail;
    return (f > 0xFFFFu) ? 0xFFFFu : (uint16_t)f;
}

/* ---- ADR-029: Channel hopping ---- */

void csi_collector_set_hop_table(const uint8_t *channels, uint8_t hop_count, uint32_t dwell_ms)
{
    if (channels == NULL) {
        ESP_LOGW(TAG, "csi_collector_set_hop_table: channels is NULL");
        return;
    }
    if (hop_count == 0 || hop_count > CSI_HOP_CHANNELS_MAX) {
        ESP_LOGW(TAG, "csi_collector_set_hop_table: invalid hop_count=%u (max=%u)",
                 (unsigned)hop_count, (unsigned)CSI_HOP_CHANNELS_MAX);
        return;
    }
    if (dwell_ms < 10) {
        ESP_LOGW(TAG, "csi_collector_set_hop_table: dwell_ms=%lu too small, clamping to 10",
                 (unsigned long)dwell_ms);
        dwell_ms = 10;
    }

    memcpy(s_hop_channels, channels, hop_count);
    s_hop_count = hop_count;
    s_dwell_ms  = dwell_ms;
    s_hop_index = 0;

    ESP_LOGI(TAG, "Hop table set: %u channels, dwell=%lu ms", (unsigned)hop_count,
             (unsigned long)dwell_ms);
    for (uint8_t i = 0; i < hop_count; i++) {
        ESP_LOGI(TAG, "  hop[%u] = channel %u", (unsigned)i, (unsigned)channels[i]);
    }
}

void csi_hop_next_channel(void)
{
    if (s_hop_count <= 1) {
        /* Single-channel mode: no-op for backward compatibility. */
        return;
    }

    s_hop_index = (s_hop_index + 1) % s_hop_count;
    uint8_t channel = s_hop_channels[s_hop_index];

    /*
     * esp_wifi_set_channel() changes the primary channel.
     * The second parameter is the secondary channel offset for HT40;
     * we use HT20 (no secondary) for sensing.
     */
    esp_err_t err = esp_wifi_set_channel(channel, WIFI_SECOND_CHAN_NONE);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Channel hop to %u failed: %s", (unsigned)channel, esp_err_to_name(err));
    } else if ((s_cb_count % 200) == 0) {
        /* Periodic log to confirm hopping is working (not every hop). */
        ESP_LOGI(TAG, "Hopped to channel %u (index %u/%u)",
                 (unsigned)channel, (unsigned)s_hop_index, (unsigned)s_hop_count);
    }
}

/**
 * Timer callback for channel hopping.
 * Called every s_dwell_ms milliseconds from the esp_timer context.
 */
static void hop_timer_cb(void *arg)
{
    (void)arg;
    csi_hop_next_channel();
}

void csi_collector_start_hop_timer(void)
{
    if (s_hop_count <= 1) {
        ESP_LOGI(TAG, "Single-channel mode: hop timer not started");
        return;
    }

    if (s_hop_timer != NULL) {
        ESP_LOGW(TAG, "Hop timer already running");
        return;
    }

    esp_timer_create_args_t timer_args = {
        .callback = hop_timer_cb,
        .arg      = NULL,
        .name     = "csi_hop",
    };

    esp_err_t err = esp_timer_create(&timer_args, &s_hop_timer);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to create hop timer: %s", esp_err_to_name(err));
        return;
    }

    uint64_t period_us = (uint64_t)s_dwell_ms * 1000;
    err = esp_timer_start_periodic(s_hop_timer, period_us);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to start hop timer: %s", esp_err_to_name(err));
        esp_timer_delete(s_hop_timer);
        s_hop_timer = NULL;
        return;
    }

    ESP_LOGI(TAG, "Hop timer started: period=%lu ms, channels=%u",
             (unsigned long)s_dwell_ms, (unsigned)s_hop_count);
}

/* ---- ADR-029: NDP frame injection stub ---- */

esp_err_t csi_inject_ndp_frame(void)
{
    /*
     * TODO: Construct a proper 802.11 Null Data Packet frame.
     *
     * A real NDP is preamble-only (~24 us airtime, no payload) and is the
     * sensing-first TX mechanism described in ADR-029. For now we send a
     * minimal null-data frame as a placeholder so the API is wired up.
     *
     * Frame structure (IEEE 802.11 Null Data):
     *   FC (2) | Duration (2) | Addr1 (6) | Addr2 (6) | Addr3 (6) | SeqCtl (2)
     *   = 24 bytes total, no body, no FCS (hardware appends FCS).
     */
    uint8_t ndp_frame[24];
    memset(ndp_frame, 0, sizeof(ndp_frame));

    /* Frame Control: Type=Data (0x02), Subtype=Null (0x04) -> 0x0048 */
    ndp_frame[0] = 0x48;
    ndp_frame[1] = 0x00;

    /* Duration: 0 (let hardware fill) */

    /* Addr1 (destination): broadcast */
    memset(&ndp_frame[4], 0xFF, 6);

    /* Addr2 (source): will be overwritten by hardware with own MAC */

    /* Addr3 (BSSID): broadcast */
    memset(&ndp_frame[16], 0xFF, 6);

    esp_err_t err = esp_wifi_80211_tx(WIFI_IF_STA, ndp_frame, sizeof(ndp_frame), false);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "NDP inject failed: %s", esp_err_to_name(err));
    }

    return err;
}
