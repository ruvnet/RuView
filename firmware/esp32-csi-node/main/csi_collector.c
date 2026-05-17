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
#include <stdlib.h>
#include "esp_log.h"
#include "esp_wifi.h"
#include "esp_timer.h"
#include "nvs.h"
#include "nvs_flash.h"
#include "sdkconfig.h"

/* ADR-060: Access the global NVS config for MAC filter and channel override. */
extern nvs_config_t g_nvs_config;

/* Defensive fix (#232, #375, #385, #386, #390): capture NVS config fields into
 * module-local statics BEFORE wifi_init_sta() runs, because WiFi driver init
 * can corrupt g_nvs_config (confirmed on device 80:b5:4e:c1:be:b8).
 * main.c calls csi_collector_set_node_id() immediately after nvs_config_load(),
 * and all runtime paths use the local copies exclusively. */
static uint8_t s_node_id = 1;
static bool s_node_id_early_set = false;

/* Defensive copy of MAC filter config — the CSI callback fires at 100-500 Hz
 * and reads filter_mac_set + filter_mac on every invocation. If wifi_init_sta()
 * corrupts g_nvs_config, the callback would read garbage, potentially causing
 * LoadProhibited panics (observed: Core 0 panic after ~2400 callbacks). */
static uint8_t s_filter_mac[6] = {0};
static bool    s_filter_mac_set = false;

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

/* ──────────────────────────────────────────────────────────────────
 * ADR-100: Gain Lock (AGC + FFT scale).
 *
 * ESP32 WiFi PHY applies automatic gain control per packet, which
 * manifests as a 20-30 % slow drift in CSI amplitude even with a
 * completely static room — masking the real modulation caused by
 * body motion. Ported from Francesco Pace's ESPectre (GPLv3,
 * https://github.com/francescopace/espectre).
 *
 * The first ~300 packets after boot are sampled. We take the median
 * AGC + FFT gain values and freeze them with two undocumented PHY
 * routines from the IDF blob. If the median AGC is below the safe
 * threshold (sensor sits very close to the AP), we *don't* lock —
 * forcing a low gain causes the RX path to freeze.
 * Supported targets: ESP32-S3 / C3 / C6. Older parts skip silently.
 * ──────────────────────────────────────────────────────────────── */
#if CONFIG_IDF_TARGET_ESP32S3 || CONFIG_IDF_TARGET_ESP32C3 || CONFIG_IDF_TARGET_ESP32C6
#define RV_GAIN_LOCK_SUPPORTED 1
/* Overlay struct on wifi_csi_info_t.rx_ctrl exposing the hidden agc/fft fields. */
typedef struct {
    unsigned : 32; unsigned : 32; unsigned : 32;
    unsigned : 32; unsigned : 32; unsigned : 16;
    signed   fft_gain : 8;
    unsigned agc_gain : 8;
    unsigned : 32; unsigned : 32;
    unsigned : 32; unsigned : 32; unsigned : 32;
    unsigned : 32;
} rv_phy_rx_ctrl_t;
extern void phy_fft_scale_force(bool force_en, int8_t force_value);
extern void phy_force_rx_gain(int force_en, int force_value);

/* ── ADR-108: NVS persistence of gain-lock values ────────────────
 * After the first successful gain-lock, save AGC/FFT medians into NVS
 * (namespace "csi_cfg", keys "gl_agc"/"gl_fft"). On subsequent boots
 * the FW loads them and immediately forces the gain — reboot → CSI
 * ready in ~0.5 s instead of ~3 s waiting for 300 calibration packets.
 *
 * Stored values are tied to: this sensor location + this AP MAC +
 * this channel + this antenna orientation. If any of those change,
 * the saved values may be wrong — but harmless: the WiFi PHY will
 * just receive slightly off-optimal CSI until the operator triggers
 * a re-calibration (today: clear NVS, reboot; future: dedicated REST).
 */
#define RV_GAIN_NVS_NS      "csi_cfg"
#define RV_GAIN_NVS_K_AGC   "gl_agc"
#define RV_GAIN_NVS_K_FFT   "gl_fft"
/* ADR-111: BSSID of the AP that gain-lock was calibrated against.
 * 6-byte blob. On boot, if the currently-connected AP MAC differs from
 * the saved value, the cached AGC/FFT are ignored and a full calibration
 * runs (gain-lock is tied to a specific AP path; swapping APs invalidates
 * it). The new MAC is written alongside AGC/FFT after re-calibration. */
#define RV_GAIN_NVS_K_AP_MAC "gl_ap_mac"

static esp_err_t rv_gain_load_from_nvs(uint8_t *agc_out, int8_t *fft_out,
                                        uint8_t mac_out[6])
{
    nvs_handle_t h;
    esp_err_t err = nvs_open(RV_GAIN_NVS_NS, NVS_READONLY, &h);
    if (err != ESP_OK) return err;
    uint8_t agc = 0;
    int8_t  fft = 0;
    err = nvs_get_u8(h, RV_GAIN_NVS_K_AGC, &agc);
    if (err == ESP_OK) err = nvs_get_i8(h, RV_GAIN_NVS_K_FFT, &fft);
    /* AP MAC is optional — older NVS blobs predate ADR-111 and have only
     * AGC+FFT. Treat a missing MAC as a wildcard match so a one-time
     * upgrade doesn't force every node to do a full re-cal. */
    if (err == ESP_OK && mac_out != NULL) {
        size_t want = 6;
        esp_err_t mac_err = nvs_get_blob(h, RV_GAIN_NVS_K_AP_MAC, mac_out, &want);
        if (mac_err != ESP_OK || want != 6) {
            memset(mac_out, 0, 6);
        }
    }
    nvs_close(h);
    if (err == ESP_OK) { *agc_out = agc; *fft_out = fft; }
    return err;
}

static void rv_gain_save_to_nvs(uint8_t agc, int8_t fft, const uint8_t mac[6])
{
    nvs_handle_t h;
    esp_err_t err = nvs_open(RV_GAIN_NVS_NS, NVS_READWRITE, &h);
    if (err != ESP_OK) {
        ESP_LOGW("csi_collector", "gain-lock NVS save: nvs_open failed: %s",
                 esp_err_to_name(err));
        return;
    }
    nvs_set_u8(h, RV_GAIN_NVS_K_AGC, agc);
    nvs_set_i8(h, RV_GAIN_NVS_K_FFT, fft);
    if (mac != NULL) {
        nvs_set_blob(h, RV_GAIN_NVS_K_AP_MAC, mac, 6);
    }
    nvs_commit(h);
    nvs_close(h);
}
#define RV_GAIN_CAL_PACKETS  300u
#define RV_GAIN_MIN_SAFE_AGC 30u     /* < 30 → forcing freezes RX. */
static uint8_t  s_agc_samples[RV_GAIN_CAL_PACKETS];
static int8_t   s_fft_samples[RV_GAIN_CAL_PACKETS];
static uint16_t s_gain_pkt_count = 0;
static bool     s_gain_locked = false;
static bool     s_gain_skipped_strong = false;
static uint8_t  s_gain_agc_value = 0;
static int8_t   s_gain_fft_value = 0;

static int rv_cmp_u8(const void *a, const void *b) {
    return (int)*(const uint8_t *)a - (int)*(const uint8_t *)b;
}
static int rv_cmp_i8(const void *a, const void *b) {
    return (int)*(const int8_t *)a - (int)*(const int8_t *)b;
}

static void rv_gain_lock_process(const wifi_csi_info_t *info)
{
    if (s_gain_locked || info == NULL) return;

    /* ADR-108: short-circuit calibration if previous values are in NVS.
     * ADR-111: also compare the saved BSSID with the currently-connected
     * AP. If they differ, the cached gain is invalid (different AP path
     * → different multipath, different optimal AGC) — discard it and run
     * a full calibration against the new AP. */
    static bool s_nvs_checked = false;
    if (!s_nvs_checked) {
        s_nvs_checked = true;
        uint8_t agc = 0; int8_t fft = 0; uint8_t saved_mac[6] = {0};
        if (rv_gain_load_from_nvs(&agc, &fft, saved_mac) == ESP_OK &&
            agc >= RV_GAIN_MIN_SAFE_AGC)
        {
            /* Read the current AP MAC. If we can't (not connected yet)
             * the gain-lock callback should not be firing at all — but
             * be defensive and skip the cache if AP info is unavailable. */
            wifi_ap_record_t ap;
            bool ap_ok = (esp_wifi_sta_get_ap_info(&ap) == ESP_OK);
            bool wildcard = true;
            for (int i = 0; i < 6; i++) {
                if (saved_mac[i] != 0) { wildcard = false; break; }
            }
            if (ap_ok && (wildcard ||
                          memcmp(saved_mac, ap.bssid, 6) == 0))
            {
                phy_fft_scale_force(true, fft);
                phy_force_rx_gain(1, (int)agc);
                s_gain_agc_value = agc;
                s_gain_fft_value = fft;
                s_gain_locked = true;
                ESP_LOGI("csi_collector",
                    "gain-lock RESTORED from NVS: AGC=%u FFT=%d "
                    "AP=%02x:%02x:%02x:%02x:%02x:%02x%s",
                    (unsigned)agc, (int)fft,
                    ap.bssid[0], ap.bssid[1], ap.bssid[2],
                    ap.bssid[3], ap.bssid[4], ap.bssid[5],
                    wildcard ? " (legacy NVS, no MAC stored)" : "");
                return;
            }
            if (ap_ok) {
                ESP_LOGW("csi_collector",
                    "gain-lock NVS MISS: saved AP=%02x:%02x:%02x:%02x:%02x:%02x "
                    "→ current=%02x:%02x:%02x:%02x:%02x:%02x. Re-calibrating.",
                    saved_mac[0], saved_mac[1], saved_mac[2],
                    saved_mac[3], saved_mac[4], saved_mac[5],
                    ap.bssid[0], ap.bssid[1], ap.bssid[2],
                    ap.bssid[3], ap.bssid[4], ap.bssid[5]);
            }
        }
    }

    const rv_phy_rx_ctrl_t *phy = (const rv_phy_rx_ctrl_t *)info;

    if (s_gain_pkt_count < RV_GAIN_CAL_PACKETS) {
        s_agc_samples[s_gain_pkt_count] = phy->agc_gain;
        s_fft_samples[s_gain_pkt_count] = phy->fft_gain;
        s_gain_pkt_count++;
        if (s_gain_pkt_count == RV_GAIN_CAL_PACKETS / 4 ||
            s_gain_pkt_count == RV_GAIN_CAL_PACKETS / 2 ||
            s_gain_pkt_count == (3u * RV_GAIN_CAL_PACKETS) / 4u) {
            ESP_LOGI(TAG, "gain-lock cal %u%% (%u/%u, AGC=%u FFT=%d)",
                     (unsigned)((s_gain_pkt_count * 100u) / RV_GAIN_CAL_PACKETS),
                     (unsigned)s_gain_pkt_count, (unsigned)RV_GAIN_CAL_PACKETS,
                     (unsigned)phy->agc_gain, (int)phy->fft_gain);
        }
        return;
    }

    /* Reached the calibration target — compute medians, lock or skip. */
    qsort(s_agc_samples, RV_GAIN_CAL_PACKETS, sizeof(uint8_t), rv_cmp_u8);
    qsort(s_fft_samples, RV_GAIN_CAL_PACKETS, sizeof(int8_t),  rv_cmp_i8);
    s_gain_agc_value = s_agc_samples[RV_GAIN_CAL_PACKETS / 2];
    s_gain_fft_value = s_fft_samples[RV_GAIN_CAL_PACKETS / 2];

    if (s_gain_agc_value < RV_GAIN_MIN_SAFE_AGC) {
        s_gain_skipped_strong = true;
        ESP_LOGW(TAG,
            "gain-lock SKIPPED: AGC median=%u < %u (signal too strong, "
            "forcing would freeze RX). Move sensor 2-3 m from AP.",
            (unsigned)s_gain_agc_value, (unsigned)RV_GAIN_MIN_SAFE_AGC);
    } else {
        phy_fft_scale_force(true, s_gain_fft_value);
        phy_force_rx_gain(1, (int)s_gain_agc_value);
        ESP_LOGI(TAG,
            "gain-lock APPLIED: AGC=%u FFT=%d (median of %u packets) — "
            "baseline drift should now collapse.",
            (unsigned)s_gain_agc_value, (int)s_gain_fft_value,
            (unsigned)RV_GAIN_CAL_PACKETS);
        /* ADR-108: persist for next boot — short-circuit calibration.
         * ADR-111: also persist the AP BSSID this calibration ran against
         * so the boot-time short-circuit can detect AP swaps and discard
         * stale gain values. */
        uint8_t cur_mac[6] = {0};
        wifi_ap_record_t ap;
        if (esp_wifi_sta_get_ap_info(&ap) == ESP_OK) {
            memcpy(cur_mac, ap.bssid, 6);
        }
        rv_gain_save_to_nvs(s_gain_agc_value, s_gain_fft_value, cur_mac);
        ESP_LOGI(TAG,
                 "gain-lock PERSISTED to NVS (AGC=%u FFT=%d AP=%02x:%02x:%02x:%02x:%02x:%02x)",
                 (unsigned)s_gain_agc_value, (int)s_gain_fft_value,
                 cur_mac[0], cur_mac[1], cur_mac[2],
                 cur_mac[3], cur_mac[4], cur_mac[5]);
    }
    s_gain_locked = true;
}
#else
static inline void rv_gain_lock_process(const wifi_csi_info_t *info) { (void)info; }
#endif

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
/* Send rate cap reduced from 20 ms to 4 ms (250 Hz) so the host calibration
 * UI can show every available frame. The real ceiling is whatever rate the
 * WiFi CSI callback actually fires at (usually 5-50 Hz on a quiet LAN). */
#define CSI_MIN_SEND_INTERVAL_US  (4 * 1000)
static int64_t s_last_send_us = 0;

/**
 * Minimum interval between processing ANY CSI callback in microseconds.
 * Promiscuous MGMT+DATA can fire 100-500+ times/sec. At rates above ~50 Hz,
 * the WiFi FIQ handler (wDev_ProcessFiq) races with SPI flash cache operations,
 * causing Core 0 LoadProhibited panics in cache_ll_l1_resume_icache.
 *
 * This early gate drops excess callbacks BEFORE any processing (serialization,
 * UDP, edge enqueue), keeping the effective callback rate at ~50 Hz while
 * preserving the full MGMT+DATA promiscuous filter and HT-LTF/STBC CSI quality.
 *
 * The WiFi hardware still captures all frames and the CSI data is generated,
 * but we simply discard the excess in software. This reduces the time spent
 * in callback context per second, giving the WiFi ISR more headroom.
 */
#define CSI_MIN_PROCESS_INTERVAL_US  (20 * 1000)  /* 50 Hz */
static int64_t s_last_process_us = 0;
static uint32_t s_early_drop = 0;

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
 *   [20+iq_len .. 20+iq_len+3]  ADR-106: sensor timestamp_us (u32 LE)
 *                               from info->rx_ctrl.timestamp. Trailing
 *                               4 bytes — server parses opportunistically;
 *                               old server tolerant of extra bytes.
 */
size_t csi_serialize_frame(const wifi_csi_info_t *info, uint8_t *buf, size_t buf_len)
{
    if (info == NULL || buf == NULL || info->buf == NULL) {
        return 0;
    }

    uint8_t n_antennas = 1;  /* ESP32-S3 typically reports 1 antenna for CSI */
    uint16_t iq_len = (uint16_t)info->len;
    uint16_t n_subcarriers = iq_len / (2 * n_antennas);

    size_t frame_size = CSI_HEADER_SIZE + iq_len + 4 /* ADR-106 trailing timestamp_us */;
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

    /* ADR-106: trailing sensor µs timestamp from rx_ctrl.timestamp.
     * This is monotonic µs since FW boot (per ESP-IDF docs) and lets
     * the host align frames across nodes within ~µs once the boot
     * offsets are learned. Old server ignores trailing bytes. */
    uint32_t ts_us = info->rx_ctrl.timestamp;
    memcpy(&buf[CSI_HEADER_SIZE + iq_len], &ts_us, 4);

    return frame_size;
}

/**
 * WiFi CSI callback — invoked by ESP-IDF when CSI data is available.
 */
static void wifi_csi_callback(void *ctx, wifi_csi_info_t *info)
{
    (void)ctx;

    /* Early rate gate: drop excess callbacks to ~50 Hz to prevent
     * SPI flash cache crash in WiFi ISR (wDev_ProcessFiq). */
    int64_t now_us = esp_timer_get_time();
    if ((now_us - s_last_process_us) < CSI_MIN_PROCESS_INTERVAL_US) {
        s_early_drop++;
        return;
    }
    s_last_process_us = now_us;

    /* ADR-060: MAC address filtering — drop frames from non-matching sources.
     * Uses defensively-copied s_filter_mac instead of g_nvs_config (which can
     * be corrupted by wifi_init_sta — same root cause as the node_id clobber). */
    if (s_filter_mac_set) {
        if (memcmp(info->mac, s_filter_mac, 6) != 0) {
            return;  /* Source MAC doesn't match filter — skip frame. */
        }
    }

    /* ADR-100: feed the gain-lock calibrator. No-op once locked / on
     * unsupported targets. Runs before the heavy work so calibration
     * happens during the first ~6 s after boot regardless of host traffic. */
    rv_gain_lock_process(info);

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

void csi_collector_set_node_id(uint8_t node_id)
{
    s_node_id = node_id;
    s_node_id_early_set = true;
    ESP_LOGI(TAG, "Early capture node_id=%u (before WiFi init, #232/#390)",
             (unsigned)node_id);

    /* Also capture MAC filter config now — same struct, same corruption risk.
     * The CSI callback reads filter_mac_set on every invocation (100-500 Hz),
     * so a corrupted value could cause erratic filtering or crash. */
    s_filter_mac_set = (g_nvs_config.filter_mac_set != 0);
    if (s_filter_mac_set) {
        memcpy(s_filter_mac, g_nvs_config.filter_mac, 6);
        ESP_LOGI(TAG, "Early capture filter_mac=%02x:%02x:%02x:%02x:%02x:%02x",
                 s_filter_mac[0], s_filter_mac[1], s_filter_mac[2],
                 s_filter_mac[3], s_filter_mac[4], s_filter_mac[5]);
    }
}

void csi_collector_init(void)
{
    if (!s_node_id_early_set) {
        /* Fallback: no early capture — use current g_nvs_config (may be clobbered). */
        s_node_id = g_nvs_config.node_id;
        ESP_LOGW(TAG, "Late capture node_id=%u (no early set_node_id call)",
                 (unsigned)s_node_id);
    } else if (g_nvs_config.node_id != s_node_id) {
        /* Canary: early capture disagrees with current g_nvs_config — corruption
         * happened between nvs_config_load() and here (likely wifi_init_sta). */
        ESP_LOGW(TAG, "node_id clobber CONFIRMED: early=%u g_nvs_config=%u "
                 "(WiFi init likely corrupted struct, using early value)",
                 (unsigned)s_node_id, (unsigned)g_nvs_config.node_id);
    } else {
        ESP_LOGI(TAG, "node_id=%u verified (early capture matches g_nvs_config)",
                 (unsigned)s_node_id);
    }

    /* Canary for filter_mac: check if WiFi init corrupted the filter fields. */
    if (s_node_id_early_set) {
        bool mac_set_now = (g_nvs_config.filter_mac_set != 0);
        if (mac_set_now != s_filter_mac_set) {
            ESP_LOGW(TAG, "filter_mac_set clobber CONFIRMED: early=%d g_nvs_config=%d",
                     (int)s_filter_mac_set, (int)mac_set_now);
        } else if (s_filter_mac_set &&
                   memcmp(s_filter_mac, g_nvs_config.filter_mac, 6) != 0) {
            ESP_LOGW(TAG, "filter_mac clobber CONFIRMED: bytes differ after WiFi init");
        }
    } else {
        /* No early capture — grab filter config now (may already be corrupted). */
        s_filter_mac_set = (g_nvs_config.filter_mac_set != 0);
        if (s_filter_mac_set) {
            memcpy(s_filter_mac, g_nvs_config.filter_mac, 6);
        }
    }

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

    /* Disable WiFi modem sleep — reliable CSI capture needs the radio awake.
     * The ESP-IDF STA default is WIFI_PS_MIN_MODEM, which lets the modem
     * sleep between DTIM beacons; with the MGMT-only promiscuous filter
     * (RuView#396) that starves the CSI callback and the per-second yield
     * collapses toward 0 pps (RuView#521). Operators who want battery
     * duty-cycling opt back in via power_mgmt_init() (provision.py
     * --duty-cycle <N>), which runs after this and re-enables modem sleep. */
    esp_err_t ps_err = esp_wifi_set_ps(WIFI_PS_NONE);
    if (ps_err != ESP_OK) {
        ESP_LOGW(TAG, "esp_wifi_set_ps(WIFI_PS_NONE) failed: %s — CSI yield may be low",
                 esp_err_to_name(ps_err));
    } else {
        ESP_LOGI(TAG, "WiFi modem sleep disabled (WIFI_PS_NONE) for CSI capture");
    }

    /* DO NOT enable promiscuous mode on these ESP32-S3 boards. Empirically,
     * setting esp_wifi_set_promiscuous(true) while STA is connected suppresses
     * the CSI RX callback entirely on this hardware revision — adaptive_ctrl
     * reports yield=0pps forever. FW5.47 (esp32s3_csi_capture) works on the
     * same boards using plain STA-mode CSI (no promiscuous), so we mirror
     * that approach here. CSI fires for every frame the STA actually
     * receives (beacons + unicast → ~10-20 Hz, same as edge_processing
     * expects). */
    ESP_LOGI(TAG, "Promiscuous mode SKIPPED (CSI via STA-only, broken otherwise on this board)");

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
