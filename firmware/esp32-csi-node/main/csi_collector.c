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
#include "c6_timesync.h"  /* ADR-110: 802.15.4 epoch for cross-node alignment */
#include "c6_sync_espnow.h" /* ADR-110 §A0.11: mesh-aligned epoch for sync packet */
#include "csi_he_gate.h"     /* ADR-423 D1: HE-LTF validity gate */
#include "c5_lltf8.h"        /* C5L8/v1 8-bit L-LTF producer */

#include <string.h>
#include <stdio.h>
#include "esp_log.h"
#include "esp_wifi.h"
#include "esp_timer.h"
#include "sdkconfig.h"
#include "esp_netif.h"          /* #954: STA gateway lookup for self-ping CSI source */
#include "ping/ping_sock.h"     /* #954: esp_ping gateway traffic generator */
#include "lwip/ip_addr.h"       /* #954: ip_addr_t target for esp_ping */
#ifdef CONFIG_CSI_C5_LLTF8_V1
#include "esp_idf_version.h"     /* C5L8/v1 PHY readback gate */
#include "soc/soc.h"            /* REG_READ for the C5L8/v1 PHY readback */
#endif

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

#if defined(CONFIG_CSI_AP_ONLY_SOURCE)
/* AP-only source filter (all targets). MEASURED 2026-11-25 with two C5s on one
 * AP: without it, 53-65% of emitted C5L8 frames came from the peer C5 and other
 * stations, all passing the non-HT/106-byte/valid-estimate checks, so one
 * "source" silently mixed transmitters. The BSSID is learned at association
 * (WIFI_EVENT_STA_CONNECTED, or the AP record at CSI init), refreshed on
 * reconnect/roam and cleared on disconnect. An explicit NVS filter_mac wins.
 * Bypassed while channel hopping. With no AP known, C5L8/v1 withholds every
 * frame (its claim needs the AP); other builds pass frames through. */
static portMUX_TYPE s_ap_src_mux = portMUX_INITIALIZER_UNLOCKED;
static uint8_t  s_ap_src_bssid[6] = {0};
static bool     s_ap_src_set = false;
static uint32_t s_ap_src_drop = 0;
#endif

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

#ifndef CONFIG_CSI_SELF_PING_HZ
#define CONFIG_CSI_SELF_PING_HZ 50
#endif

#if CONFIG_CSI_SELF_PING_HZ < 10 || CONFIG_CSI_SELF_PING_HZ > 50
#error "CONFIG_CSI_SELF_PING_HZ must stay within the hardware-qualified 10-50 Hz range"
#endif

#define CSI_SELF_PING_INTERVAL_MS (1000U / CONFIG_CSI_SELF_PING_HZ)

#ifndef CONFIG_EDGE_DSP_SAMPLE_HZ
#if CONFIG_IDF_TARGET_ESP32C6 || CONFIG_IDF_TARGET_ESP32C5
#define CONFIG_EDGE_DSP_SAMPLE_HZ 8
#else
#define CONFIG_EDGE_DSP_SAMPLE_HZ 20
#endif
#endif

#if CONFIG_EDGE_DSP_SAMPLE_HZ < 8 || CONFIG_EDGE_DSP_SAMPLE_HZ > 50
#error "CONFIG_EDGE_DSP_SAMPLE_HZ must stay within the supported 8-50 Hz range"
#endif

#define EDGE_DSP_MIN_INTERVAL_US (1000000U / CONFIG_EDGE_DSP_SAMPLE_HZ)
static int64_t s_next_edge_enqueue_us = 0;
static uint32_t s_edge_rate_skip = 0;

/**
 * Minimum interval between UDP sends in microseconds.
 * CSI callbacks can fire hundreds of times per second in promiscuous mode.
 * We cap the send rate to avoid exhausting lwIP packet buffers (ENOMEM).
 * Default: 20 ms = 50 Hz max send rate.
 */
#define CSI_MIN_SEND_INTERVAL_US  (20 * 1000)
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
/* Mesh-epoch window index of the last accepted frame. UINT64_MAX means "none
 * yet", which no real bucket can collide with. */
static uint64_t s_last_gate_bucket __attribute__((unused)) = UINT64_MAX;
static uint32_t s_early_drop = 0;

/* ADR-423 D1: HE-LTF validity gate. Counted for every HE PPDU the driver
 * delivers (before the rate gate), so the reported ratio is the radio's, not
 * the post-gate sample's. */
#if defined(CONFIG_CSI_HE_INVALID_POLICY_TAG)
#define CSI_HE_POLICY CSI_HE_POLICY_TAG
#elif defined(CONFIG_CSI_HE_INVALID_POLICY_PASS)
#define CSI_HE_POLICY CSI_HE_POLICY_PASS
#else
#define CSI_HE_POLICY CSI_HE_POLICY_DROP
#endif
static uint32_t s_he_valid = 0;
static uint32_t s_he_invalid = 0;

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

#ifdef CONFIG_CSI_C5_LLTF8_V1
/* C5L8/v1 claim inputs fixed at init by SDK readback (never assumed). */
static bool     s_c5l8_readback_ok = false;
static uint8_t  s_c5l8_sta_bw_mhz  = 0;
static uint32_t s_c5l8_emitted = 0;
static uint32_t s_c5l8_reject[C5L8_REJECT_COUNT] = {0};
/* C5L8/v1 cadence diagnostics: every callback by cur_bb_format, and the
 * qualifying frames the pacer withheld. */
static uint32_t s_c5l8_bb_seen[16] = {0};
static uint32_t s_c5l8_pace_drop = 0;
#if CONFIG_C5_LLTF8_PACE_HZ > 0
#define C5L8_PACE_INTERVAL_US (1000000U / CONFIG_C5_LLTF8_PACE_HZ)
static int64_t  s_c5l8_pace_deadline_us = 0;
static uint64_t s_c5l8_pace_last_accept_us = UINT64_MAX;  /* local us of last accept */
#endif
#if CONFIG_C5_LLTF8_STATS_PERIOD_S > 0
static int64_t  s_c5l8_stats_t0_us = 0;
static uint32_t s_c5l8_stats_prev[4 + 16 + C5L8_REJECT_COUNT] = {0};
#endif
#endif

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

    /* ADR-110: PPDU type (byte 18) + bandwidth/flags (byte 19).
     * Previously reserved-zero, now optionally populated when CONFIG_CSI_FRAME_HE_TAGGING.
     * Readers that don't know about the extension see zeros — backward compatible.
     *
     * The struct that backs info->rx_ctrl is target-conditional in IDF v5.4
     * (esp_wifi/include/local/esp_wifi_types_native.h):
     *
     *   CONFIG_SOC_WIFI_HE_SUPPORT=y  (C6/C5)  →  esp_wifi_rxctrl_t with cur_bb_format, second
     *   otherwise                     (S3 etc) →  legacy struct with sig_mode, cwb, stbc
     *
     * Byte-18 PPDU type encoding stays the same across targets:
     *   0=HT/legacy bucket, 1=HE-SU, 2=HE-MU, 3=HE-TB, 0xFF=unknown
     */
#ifdef CONFIG_CSI_FRAME_HE_TAGGING
    uint8_t ppdu_type = 0xFF;
    uint8_t flags     = 0;
#if CONFIG_SOC_WIFI_HE_SUPPORT
    /* HE-capable chips: read cur_bb_format (0=11b, 1=11g, 2=HT, 3=VHT, 4=HE-SU,
     * 5=HE-MU, 6=HE-ERSU, 7=HE-TB) and 'second' (40 MHz secondary chan offset). */
    switch (info->rx_ctrl.cur_bb_format) {
        case 0:
        case 1:
        case 2:  ppdu_type = 0; break;  /* 11b/g/a/HT bucket */
        case 3:  ppdu_type = 0; break;  /* VHT — rare on 2.4 GHz, HT bucket */
        case 4:  ppdu_type = 1; break;  /* HE-SU */
        case 5:  ppdu_type = 2; break;  /* HE-MU */
        case 6:  ppdu_type = 1; break;  /* HE-ER-SU collapses to HE-SU */
        case 7:  ppdu_type = 3; break;  /* HE-TB */
        default: ppdu_type = 0xFF; break;
    }
    if (info->rx_ctrl.second != 0) flags |= 0x1;  /* bw 40 MHz */
#else
    /* Pre-HE chips (S3 etc): use legacy sig_mode + cwb + stbc fields. */
    switch (info->rx_ctrl.sig_mode) {
        case 0: ppdu_type = 0; break;  /* non-HT (11b/g) */
        case 1: ppdu_type = 0; break;  /* HT (11n) */
        case 3: ppdu_type = 0; break;  /* VHT — bucket as HT for storage */
        default: ppdu_type = 0xFF; break;
    }
    if (info->rx_ctrl.cwb) flags |= 0x1;            /* bw 40 MHz */
    if (info->rx_ctrl.stbc) flags |= (1 << 2);      /* STBC */
#endif  /* CONFIG_SOC_WIFI_HE_SUPPORT */
    /* ADR-018 byte 19 bit 4 = "cross-node sync valid". Two transports can
     * set it: the original 802.15.4 c6_timesync (broken in IDF v5.4 — D1)
     * and the ESP-NOW workaround c6_sync_espnow (measured working in §A0.7-
     * §A0.10). OR them together so frames signal sync from whichever
     * transport is alive on this node. Host can pair against the sync
     * packet (§A0.12) once it sees this bit. */
#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_C6_TIMESYNC_ENABLE)
    if (c6_timesync_is_valid()) flags |= (1 << 4);  /* 15.4 sync valid */
#endif
    if (c6_sync_espnow_is_valid()) flags |= (1 << 4);  /* ESP-NOW sync valid (D1 workaround) */
    buf[18] = ppdu_type;
    buf[19] = flags;
#else
    buf[18] = 0;
    buf[19] = 0;
#endif

    /* I/Q data. ESP-IDF documents that the first four CSI bytes are invalid
     * when first_word_invalid is set. Never let those hardware artifacts feed
     * either the host or edge DSP path. Preserve the fixed ADR-018 geometry by
     * zeroing rather than removing bytes, and mark the sanitation in bit 5. */
    memcpy(&buf[CSI_HEADER_SIZE], info->buf, iq_len);
    if (info->first_word_invalid && iq_len > 0) {
        size_t invalid_len = iq_len < 4 ? iq_len : 4;
        memset(&buf[CSI_HEADER_SIZE], 0, invalid_len);
        buf[19] |= CSI_FLAG_FIRST_WORD_SANITIZED;
    }

    return frame_size;
}

/**
 * WiFi CSI callback — invoked by ESP-IDF when CSI data is available.
 */

static void wifi_csi_callback(void *ctx, wifi_csi_info_t *info)
{
    (void)ctx;
#ifdef CONFIG_CSI_C5_LLTF8_V1
    s_c5l8_bb_seen[info->rx_ctrl.cur_bb_format & 0xFu]++;
#if CONFIG_C5_LLTF8_STATS_PERIOD_S > 0
    {
        /* One line per period: callbacks by PPDU format, emitted frames and
         * withheld reasons as deltas, so cadence is readable from serial alone. */
        const int64_t t = esp_timer_get_time();
        if (s_c5l8_stats_t0_us == 0) {
            s_c5l8_stats_t0_us = t;
        } else if (t - s_c5l8_stats_t0_us >= (int64_t)CONFIG_C5_LLTF8_STATS_PERIOD_S * 1000000LL) {
            uint32_t cur[4 + 16 + C5L8_REJECT_COUNT];
            cur[0] = s_c5l8_emitted; cur[1] = s_c5l8_pace_drop; cur[2] = s_early_drop;
#if defined(CONFIG_CSI_AP_ONLY_SOURCE)
            cur[3] = s_ap_src_drop;
#else
            cur[3] = 0;
#endif
            for (unsigned i = 0; i < 16; i++) cur[4 + i] = s_c5l8_bb_seen[i];
            for (unsigned i = 0; i < C5L8_REJECT_COUNT; i++) cur[20 + i] = s_c5l8_reject[i];
            uint32_t d[4 + 16 + C5L8_REJECT_COUNT];
            for (unsigned i = 0; i < sizeof(cur) / sizeof(cur[0]); i++) d[i] = cur[i] - s_c5l8_stats_prev[i];
            const double secs = (double)(t - s_c5l8_stats_t0_us) / 1e6;
            ESP_LOGI(TAG, "C5L8 cadence %.1fs: emitted=%lu (%.2f Hz) bb 11b=%lu legacy=%lu ht=%lu vht=%lu hesu=%lu hemu=%lu other=%lu "
                     "| withheld fmt=%lu sec=%lu est=%lu len=%lu ch=%lu rb=%lu bw=%lu src=%lu pace=%lu gate=%lu",
                     secs, (unsigned long)d[0], d[0] / secs,
                     (unsigned long)d[4 + 0], (unsigned long)d[4 + 1], (unsigned long)d[4 + 2],
                     (unsigned long)d[4 + 3], (unsigned long)d[4 + 4], (unsigned long)d[4 + 5],
                     (unsigned long)(d[4 + 6] + d[4 + 7] + d[4 + 11]),
                     (unsigned long)d[20 + C5L8_REJECT_BB_FORMAT], (unsigned long)d[20 + C5L8_REJECT_SECONDARY],
                     (unsigned long)d[20 + C5L8_REJECT_ESTIMATE_INVALID], (unsigned long)d[20 + C5L8_REJECT_LENGTH],
                     (unsigned long)d[20 + C5L8_REJECT_CHANNEL], (unsigned long)d[20 + C5L8_REJECT_NO_READBACK],
                     (unsigned long)d[20 + C5L8_REJECT_STA_BANDWIDTH], (unsigned long)d[3],
                     (unsigned long)d[1], (unsigned long)d[2]);
            memcpy(s_c5l8_stats_prev, cur, sizeof(cur));
            s_c5l8_stats_t0_us = t;
        }
    }
#endif
#endif

    /* ADR-060: MAC address filtering — MOVED ABOVE the rate gate.
     *
     * The gate below stamps s_last_process_us before this filter ran, so a
     * frame from any foreign transmitter claimed the 20 ms window and was then
     * discarded here — starving the filtered stream. Measured on an ESP32-C6
     * locked to its associated AP: 1.7 CSI/s instead of ~20 Hz. Filtering
     * first preserves the gate's crash protection (processing is still capped
     * at 50 Hz) while letting the matched transmitter actually fill the slots.
     * With no filter configured the behaviour is byte-for-byte unchanged.
     *
     * Uses defensively-copied s_filter_mac instead of g_nvs_config (which can
     * be corrupted by wifi_init_sta — same root cause as the node_id clobber). */
    if (s_filter_mac_set) {
        if (memcmp(info->mac, s_filter_mac, 6) != 0) {
            return;  /* Source MAC doesn't match filter — skip frame. */
        }
    }
#if defined(CONFIG_CSI_AP_ONLY_SOURCE)
    else if (s_hop_count <= 1) {
        bool known, from_ap;
        portENTER_CRITICAL_ISR(&s_ap_src_mux);
        known = s_ap_src_set;
        from_ap = known && memcmp(info->mac, s_ap_src_bssid, 6) == 0;
        portEXIT_CRITICAL_ISR(&s_ap_src_mux);
#if defined(CONFIG_CSI_C5_LLTF8_V1)
        if (!from_ap) {                 /* C5L8/v1: AP frames only, AP must be known */
#else
        if (known && !from_ap) {        /* other builds: no AP known -> unfiltered */
#endif
            s_ap_src_drop++;
            return;
        }
    }
#endif

    /* ADR-423 D1: decide on HE estimate validity BEFORE the rate gate, so an
     * invalid frame cannot claim the 20 ms slot that a valid one would fill
     * (the same starvation the ADR-060 move above fixed for MAC filtering). */
    csi_he_action_t he_action = CSI_HE_ACTION_ACCEPT;
#if CONFIG_SOC_WIFI_HE_SUPPORT
    {
        unsigned bb = info->rx_ctrl.cur_bb_format;
        bool vld = info->rx_ctrl.rx_channel_estimate_info_vld;
        if (bb >= CSI_BB_FORMAT_HE_MIN) {
            if (vld) { s_he_valid++; } else { s_he_invalid++; }
        }
        he_action = csi_he_gate_action(bb, vld, CSI_HE_POLICY);
        if (he_action == CSI_HE_ACTION_DROP) {
            return;
        }
    }
#endif

#ifdef CONFIG_CSI_C5_LLTF8_V1
    /* C5L8/v1: a callback that can never carry the C5L8/v1 claim
     * (HT/VHT/HE, secondary channel, invalid estimate, not 106 bytes) must
     * not claim the 20 ms slot a qualifying non-HT frame would fill. Same
     * starvation reasoning as the ADR-060 move above. c5l8_build() re-checks. */
    if (info->rx_ctrl.cur_bb_format != C5L8_BB_FORMAT_NON_HT_OFDM
        || info->rx_ctrl.second != 0
        || !info->rx_ctrl.rx_channel_estimate_info_vld
        || info->len != C5L8_IQ_BYTES) {
        c5l8_status_t pre = info->rx_ctrl.cur_bb_format != C5L8_BB_FORMAT_NON_HT_OFDM ? C5L8_REJECT_BB_FORMAT
                          : info->rx_ctrl.second != 0 ? C5L8_REJECT_SECONDARY
                          : !info->rx_ctrl.rx_channel_estimate_info_vld ? C5L8_REJECT_ESTIMATE_INVALID
                          : C5L8_REJECT_LENGTH;
        s_c5l8_reject[pre]++;
        return;
    }
#endif

    /* Early rate gate: drop excess callbacks to ~50 Hz to prevent
     * SPI flash cache crash in WiFi ISR (wDev_ProcessFiq).
     *
     * HOW the rate is limited decides whether cross-node fusion is possible.
     *
     * The original gate is "at least 20 ms since MY last accept". Each node
     * therefore has an independent phase: node A can accept at t=0,20,40 ms
     * while node B accepts at 7,27,47. Both are perfectly healthy and both hit
     * 50 Hz, yet they accept disjoint frames and nothing can be paired.
     *
     * MEASURED 2026-08-30, two boards side by side: they HEARD the same
     * transmissions 72% of the time but only 25% of accepted frames were
     * common. The frames were there; the gate was throwing away the pairing.
     *
     * So gate on a bucket of the MESH-ALIGNED epoch instead: every node takes
     * the first frame it hears in the same absolute 20 ms window. Two nodes
     * that heard the same frame now both accept it. Mesh sync is good to
     * roughly 490 us against a 20 ms bucket -- about 2.5% of a window -- so
     * boundary disagreements are rare.
     *
     * Falls back to the original elapsed-time gate whenever mesh sync is not
     * valid (no leader heard yet, or a node that just booted), because an
     * unsynced epoch would otherwise gate on a meaningless number. A node
     * running unsynced simply gets the old behaviour rather than no gate.
     *
     * The half-interval floor below keeps the crash protection honest: bucket
     * gating alone could accept at the end of one window and the start of the
     * next, back to back. The floor bounds that to ~100 Hz for a single pair
     * while the average stays at 50 Hz. Do not remove it; the gate exists for
     * a crash, not for tidiness. */
    int64_t now_us = esp_timer_get_time();
    bool take;
#if defined(CONFIG_CSI_C5_LLTF8_V1) && CONFIG_C5_LLTF8_PACE_HZ > 0
    /* C5L8/v1 with the pacer on: the pacer (<= PACE_HZ, after the C5L8 checks,
     * before any send) is the rate limit. MEASURED 2026-11-25 on one C5: legacy
     * frames arrive in bursts, and this 20 ms gate dropped ~70% of qualifying
     * frames before they could be sent. */
    take = true;
#elif defined(CONFIG_CSI_GATE_MESH_ALIGNED)
    if (c6_sync_espnow_is_valid()) {
        uint64_t bucket = c6_sync_espnow_get_epoch_us()
                        / (uint64_t)CSI_MIN_PROCESS_INTERVAL_US;
        take = (bucket != s_last_gate_bucket)
            && ((now_us - s_last_process_us) >= CSI_MIN_PROCESS_INTERVAL_US / 2);
        if (take) {
            s_last_gate_bucket = bucket;
        }
    } else {
        take = (now_us - s_last_process_us) >= CSI_MIN_PROCESS_INTERVAL_US;
    }
#else
    take = (now_us - s_last_process_us) >= CSI_MIN_PROCESS_INTERVAL_US;
#endif
    if (!take) {
        s_early_drop++;
        return;
    }
    s_last_process_us = now_us;

    s_cb_count++;

    if (s_cb_count <= 3 || (s_cb_count % 100) == 0) {
        ESP_LOGI(TAG, "CSI cb #%lu: len=%d rssi=%d ch=%d",
                 (unsigned long)s_cb_count, info->len,
                 info->rx_ctrl.rssi, info->rx_ctrl.channel);
    }
#if CONFIG_SOC_WIFI_HE_SUPPORT
    if ((s_cb_count % 1000) == 0) {
        ESP_LOGI(TAG, "ADR-423 HE estimate: valid=%lu invalid=%lu policy=%d",
                 (unsigned long)s_he_valid, (unsigned long)s_he_invalid, (int)CSI_HE_POLICY);
    }
#endif

    uint8_t frame_buf[CSI_MAX_FRAME_SIZE];
#ifdef CONFIG_CSI_C5_LLTF8_V1
    /* C5L8/v1: emit ONLY the exact C5L8/v1 datagram. Callbacks that
     * do not meet every claimed field are counted and dropped without samples;
     * a bare 106-byte ADR-018 frame is never sent (the host rejects it). */
    size_t frame_len = 0;
    {
        c5l8_input_t c5in = {
            .node_id            = s_node_id,
            .sequence           = s_sequence,
            .primary_channel    = (uint8_t)info->rx_ctrl.channel,
            .rssi               = (int8_t)info->rx_ctrl.rssi,
            .noise_floor        = (int8_t)info->rx_ctrl.noise_floor,
            .bb_format          = (uint8_t)info->rx_ctrl.cur_bb_format,
            .secondary_channel  = (uint8_t)info->rx_ctrl.second,
            .estimate_valid     = info->rx_ctrl.rx_channel_estimate_info_vld != 0,
            .first_word_invalid = info->first_word_invalid,
            .sync_valid         = c6_sync_espnow_is_valid(),
            .format_readback_ok = s_c5l8_readback_ok,
            .sta_bandwidth_mhz  = s_c5l8_sta_bw_mhz,
            .iq                 = (const uint8_t *)info->buf,
            .iq_len             = (uint16_t)info->len,
        };
        c5l8_status_t st = c5l8_build(&c5in, frame_buf, sizeof(frame_buf), &frame_len);
        if (st != C5L8_OK) {
            s_c5l8_reject[st]++;
            uint32_t n = s_c5l8_reject[st];
            if (n <= 3 || (n % 1000) == 0) {
                ESP_LOGW(TAG, "C5L8/v1 withheld: %s (count=%lu, len=%d bb=%u ch=%u)",
                         c5l8_status_str(st), (unsigned long)n, info->len,
                         (unsigned)info->rx_ctrl.cur_bb_format, (unsigned)info->rx_ctrl.channel);
            }
            return;
        }
#if CONFIG_C5_LLTF8_PACE_HZ > 0
        {
            /* C5L8/v1 pacer: with a qualifying source faster than PACE_HZ this yields
             * ~PACE_HZ, which the learner's two-sided Arrival18To22 band needs. Only
             * frames that passed every C5L8 check can claim a slot. */
            const int64_t pnow = esp_timer_get_time();
            /* Deadline pacer. MEASURED: absolute 50 ms buckets with a half-window
             * floor emitted only 17.1-20.0 Hz from 35-41 qualifying/s.
             * Accept the first qualifying frame at/after the deadline, then advance the
             * deadline by exactly one interval, so a late frame is followed by an
             * earlier one and the average stays at PACE_HZ. Catch-up is capped at one
             * interval after a source outage; minimum spacing is a quarter interval. */
            if (s_c5l8_pace_deadline_us == 0) {
                s_c5l8_pace_deadline_us = pnow;            /* deadline */
            }
            if (pnow < s_c5l8_pace_deadline_us
                || (s_c5l8_pace_last_accept_us != UINT64_MAX
                    && (uint64_t)pnow - s_c5l8_pace_last_accept_us < (uint64_t)(C5L8_PACE_INTERVAL_US / 4))) {
                s_c5l8_pace_drop++;
                return;
            }
            s_c5l8_pace_deadline_us += C5L8_PACE_INTERVAL_US;
            if (s_c5l8_pace_deadline_us < pnow - (int64_t)C5L8_PACE_INTERVAL_US) {
                s_c5l8_pace_deadline_us = pnow + C5L8_PACE_INTERVAL_US / 2;   /* outage: no burst */
            }
            s_c5l8_pace_last_accept_us = (uint64_t)pnow;   /* last accept time */
        }
#endif
        s_sequence++;
        s_c5l8_emitted++;
        if (s_c5l8_emitted <= 3 || (s_c5l8_emitted % 1000) == 0) {
            ESP_LOGI(TAG, "C5L8 emitted #%lu (%u B, flags=0x%02x, %lu MHz)",
                     (unsigned long)s_c5l8_emitted, (unsigned)frame_len, frame_buf[19],
                     (unsigned long)(5000u + 5u * info->rx_ctrl.channel));
        }
    }
    const size_t edge_iq_len = C5L8_IQ_BYTES;
#else
    size_t frame_len = csi_serialize_frame(info, frame_buf, sizeof(frame_buf));
    if (he_action == CSI_HE_ACTION_TAG && frame_len > 19) {
        frame_buf[19] |= CSI_FRAME_FLAG_HE_ESTIMATE_INVALID;  /* ADR-423 D1 */
    }
    const size_t edge_iq_len = frame_len > CSI_HEADER_SIZE ? frame_len - CSI_HEADER_SIZE : 0;
#endif

    if (frame_len > 0) {
        /* Rate-limit UDP sends to avoid ENOMEM from lwIP pbuf exhaustion.
         * In promiscuous mode, CSI callbacks can fire 100-500+ times/sec.
         * We only need 20-50 Hz for the sensing pipeline. */
        int64_t now = esp_timer_get_time();
#if defined(CONFIG_CSI_C5_LLTF8_V1) && CONFIG_C5_LLTF8_PACE_HZ > 0
        /* C5L8/v1 with the pacer on: the pacer already bounds the rate (<= PACE_HZ,
         * min spacing a quarter interval). MEASURED 2026-11-25 on one C5: this 20 ms
         * limiter then discarded ~1% of paced catch-up frames after their sequence
         * number was used (seq_lost 41-77 per 300 s, each a ~90-105 ms gap);
         * without it seq_lost was 0 at a flat 20.00 Hz. */
        if (true) {
#else
        if ((now - s_last_send_us) >= CSI_MIN_SEND_INTERVAL_US) {
#endif
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

    /* ADR-039 / ADR-347: Raw CSI stays at the independent network cadence,
     * while the on-device Tier 1/2 pipeline receives a uniform, sustainable
     * stream. Enqueuing every burst frame overloaded the unicore C6 DSP and
     * turned 30-40 callback pps into an irregular approximately 8 Hz subset. */
    /* ADR-423 D1: a tagged (invalid-estimate) frame is streamed for study but
     * never reaches presence/vitals/top-K. */
    if (frame_len > CSI_HEADER_SIZE && he_action != CSI_HE_ACTION_TAG) {
        if (s_next_edge_enqueue_us == 0) {
            s_next_edge_enqueue_us = now_us;
        }

        if (now_us >= s_next_edge_enqueue_us) {
            /* Reuse the sanitized ADR-018 payload. Feeding info->buf here
             * would reintroduce first_word_invalid artifacts on device. */
            (void)edge_enqueue_csi(&frame_buf[CSI_HEADER_SIZE],
                                   (uint16_t)edge_iq_len,
                                   (int8_t)info->rx_ctrl.rssi, info->rx_ctrl.channel);

            /* Preserve the configured sample clock instead of resetting it to
             * each irregular callback. With roughly 35 raw callbacks per
             * second, a last-seen 100 ms gate selected every fourth callback
             * and drifted to roughly 8 Hz. Advancing the deadline by complete
             * periods alternates the available callbacks around the configured
             * phase and prevents both drift and catch-up bursts. */
            int64_t periods = ((now_us - s_next_edge_enqueue_us) /
                               EDGE_DSP_MIN_INTERVAL_US) + 1;
            s_next_edge_enqueue_us += periods * EDGE_DSP_MIN_INTERVAL_US;
        } else {
            s_edge_rate_skip++;
        }
    }

    /* ADR-110 §A0.11/§A0.12 — Emit a sync-packet every N CSI frames so the
     * host aggregator can pair node-local sequence numbers with the mesh-aligned
     * epoch coming out of c6_sync_espnow_get_epoch_us(). Backwards-compatible
     * with the ADR-018 frame format: new packet uses a distinct magic so the
     * existing CSI parser can dispatch by first 4 bytes.
     *
     * Cadence is operator-tunable via CONFIG_C6_SYNC_EVERY_N_FRAMES (default 20).
     * At 10 Hz observed CSI rate that's ~2 s between sync packets; raise to 50
     * for ~5 s (less overhead, slower convergence), lower to 5 for ~0.5 s
     * (heavier wire, tighter ADR-029/030 multistatic alignment window). */
    {
#ifndef CONFIG_C6_SYNC_EVERY_N_FRAMES
#define CONFIG_C6_SYNC_EVERY_N_FRAMES 20
#endif
        if ((s_cb_count % CONFIG_C6_SYNC_EVERY_N_FRAMES) == 0) {
            uint8_t sync[32];
            uint32_t sync_magic = 0xC511A110u;    /* CSI-ADR-110 sync packet */
            uint64_t local_us = (uint64_t)esp_timer_get_time();
            uint64_t epoch_us = c6_sync_espnow_get_epoch_us();
            int64_t  off_smooth = c6_sync_espnow_get_offset_us_smoothed();
            uint8_t  flags = 0;
            if (c6_sync_espnow_is_leader()) flags |= 0x01;
            if (c6_sync_espnow_is_valid())  flags |= 0x02;
            if (off_smooth != 0)            flags |= 0x04;

            memcpy(&sync[0],  &sync_magic, 4);
            sync[4] = s_node_id;
            sync[5] = 0x01;                       /* protocol version */
            sync[6] = flags;
            sync[7] = 0;                          /* reserved */
            memcpy(&sync[8],  &local_us, 8);
            memcpy(&sync[16], &epoch_us, 8);
            memcpy(&sync[24], &s_sequence, 4);    /* high-water seq for pairing */
            uint32_t zero32 = 0;
            memcpy(&sync[28], &zero32, 4);        /* reserved (room for leader_id low32) */
            /* Sync packets are 32 B at ~0.5 Hz — priority path so the CSI
             * ENOMEM backoff can't starve cross-node time alignment (#1183). */
            int sr = stream_sender_send_priority(sync, sizeof(sync));
            static uint32_t s_sync_count = 0;
            s_sync_count++;
            if (s_sync_count <= 3 || (s_sync_count % 60) == 0) {
                ESP_LOGI(TAG, "sync-pkt #%lu (sr=%d) node=%u flags=0x%02x "
                              "local_us=%llu epoch_us=%llu seq=%lu",
                         (unsigned long)s_sync_count, sr,
                         (unsigned)s_node_id, (unsigned)flags,
                         (unsigned long long)local_us,
                         (unsigned long long)epoch_us,
                         (unsigned long)s_sequence);
            }
        }
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

/* ---- RuView#521/#954: connected-STA CSI traffic source (additive) ----
 *
 * The ESP32 CSI engine only produces CSI for received OFDM frames (L-LTF/HT-LTF).
 * On a quiet network — or on a display-enabled build where the #893 MGMT->MGMT+DATA
 * promiscuous upgrade is skipped (has_display=true) — the only CSI-eligible frames
 * are sparse beacons (often non-OFDM DSSS), so wifi_csi_callback can starve to
 * yield=0pps -> DEGRADED -> motion/presence=0 (#521, #954).
 *
 * Ping the STA's gateway to request unicast traffic. Some APs send ICMP replies
 * at 802.11b rates, which produce no CSI (#2287); this is not a guaranteed
 * OFDM floor. Such APs need a LAN host sending unicast UDP to the node.
 * It is ADDITIVE — promiscuous capture (#396/#893) is left fully intact so
 * multistatic/multi-node sensing still hears other stations' frames. Mirrors
 * Espressif's esp-csi csi_recv_router reference.
 */
static esp_ping_handle_t s_self_ping = NULL;
static void csi_ping_cb_noop(esp_ping_handle_t hdl, void *args) { (void)hdl; (void)args; }

static void csi_start_self_ping(void)
{
    if (s_self_ping != NULL) {
        return;  /* already running */
    }

    esp_netif_t *sta = esp_netif_get_handle_from_ifkey("WIFI_STA_DEF");
    esp_netif_ip_info_t ip;
    if (sta == NULL || esp_netif_get_ip_info(sta, &ip) != ESP_OK || ip.gw.addr == 0) {
        ESP_LOGW(TAG, "self-ping: no gateway IP yet; CSI relies on ambient frames (#954)");
        return;
    }

    char gw_str[16];
    esp_ip4addr_ntoa(&ip.gw, gw_str, sizeof(gw_str));

    ip_addr_t target;
    memset(&target, 0, sizeof(target));
    ipaddr_aton(gw_str, &target);

    esp_ping_config_t cfg = ESP_PING_DEFAULT_CONFIG();
    cfg.target_addr     = target;
    cfg.count           = ESP_PING_COUNT_INFINITE;
    cfg.interval_ms     = CSI_SELF_PING_INTERVAL_MS;
    cfg.timeout_ms      = CSI_SELF_PING_INTERVAL_MS;
    cfg.data_size       = 1;
    cfg.task_stack_size = 4096;

    esp_ping_callbacks_t cbs = {
        .cb_args         = NULL,
        .on_ping_success = csi_ping_cb_noop,
        .on_ping_timeout = csi_ping_cb_noop,
        .on_ping_end     = csi_ping_cb_noop,
    };

    if (esp_ping_new_session(&cfg, &cbs, &s_self_ping) == ESP_OK && s_self_ping != NULL) {
        esp_ping_start(s_self_ping);
        ESP_LOGI(TAG, "self-ping started -> %s @%dHz (CSI yield depends on AP reply rate, #2287)",
                 gw_str, CONFIG_CSI_SELF_PING_HZ);
    } else {
        ESP_LOGW(TAG, "self-ping: esp_ping_new_session failed");
        s_self_ping = NULL;
    }
}

void csi_collector_set_ap_source(const uint8_t *bssid, const char *via)
{
#if defined(CONFIG_CSI_AP_ONLY_SOURCE)
    if (bssid == NULL) {
        portENTER_CRITICAL(&s_ap_src_mux);
        s_ap_src_set = false;
        portEXIT_CRITICAL(&s_ap_src_mux);
        ESP_LOGI(TAG, "AP-only source filter cleared (%s)", via ? via : "?");
        return;
    }
    bool changed;
    portENTER_CRITICAL(&s_ap_src_mux);
    changed = !s_ap_src_set || memcmp(s_ap_src_bssid, bssid, 6) != 0;
    memcpy(s_ap_src_bssid, bssid, 6);
    s_ap_src_set = true;
    portEXIT_CRITICAL(&s_ap_src_mux);
    if (changed) {
        /* Masked: never log a full MAC at info level. */
        ESP_LOGI(TAG, "AP-only source filter: AP **:**:**:**:**:%02x via %s%s",
                 bssid[5], via ? via : "?",
                 s_filter_mac_set ? " (NVS filter_mac is set and takes precedence)" : "");
    }
#else
    (void)bssid; (void)via;
#endif
}

uint32_t csi_collector_get_ap_source_drops(void)
{
#if defined(CONFIG_CSI_AP_ONLY_SOURCE)
    return s_ap_src_drop;
#else
    return 0;
#endif
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
        /* Masked: never log a full MAC at info level. */
        ESP_LOGI(TAG, "Early capture filter_mac=**:**:**:**:**:%02x", s_filter_mac[5]);
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
#if defined(CONFIG_CSI_C5_LLTF8_V1)
    uint8_t csi_channel = (uint8_t)CONFIG_C5_LLTF8_CHANNEL;
#else
    uint8_t csi_channel = (uint8_t)CONFIG_CSI_WIFI_CHANNEL;
#endif

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

    /* Enable promiscuous mode — required for reliable CSI callbacks.
     * Without this, CSI only fires on frames destined to this station,
     * which may be very infrequent on a quiet network. */
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous(true));
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous_rx_cb(wifi_promiscuous_cb));

    /* MGMT-only promiscuous filter + active probe injection (RuView#396).
     *
     * DATA frames cause 100-500+ WiFi HW interrupts/sec which crashes Core 0
     * in wDev_ProcessFiq (SPI flash cache race in ESP-IDF WiFi blob).
     * MGMT-only gives ~10 Hz (beacons). Probe request injection at 10 Hz
     * adds ~10 Hz probe responses from APs → ~20 Hz total, matching the
     * edge processing designed sample rate of 20 Hz. */
    wifi_promiscuous_filter_t filt = {
        .filter_mask = WIFI_PROMIS_FILTER_MASK_MGMT,
    };
    ESP_ERROR_CHECK(esp_wifi_set_promiscuous_filter(&filt));

    ESP_LOGI(TAG, "Promiscuous mode enabled (MGMT-only, RuView#396)");

#if defined(CONFIG_CSI_C5_LLTF8_V1)
    /* C5L8/v1: legacy + forced L-LTF only, 8-bit components,
     * zero scaling, nothing else. The claim is only made if the SDK reads the
     * same configuration back. */
    wifi_csi_config_t csi_config;
    memset(&csi_config, 0, sizeof(csi_config));
    csi_config.enable = 1U;
    csi_config.acquire_csi_legacy = 1U;
    csi_config.acquire_csi_force_lltf = 1U;
    csi_config.val_scale_cfg = 0U;
    csi_config.lltf_bit_mode = 1U;   /* 1 = 8-bit I/Q (SDK default 0 = 12-bit) */
    csi_config.dump_ack_en = 0U;
#elif CONFIG_SOC_WIFI_HE_SUPPORT
    /* Wi-Fi 6 targets (e.g. ESP32-C6): wifi_csi_config_t is wifi_csi_acquire_config_t
     * (bitfields), not the legacy 802.11n bool layout used on ESP32-S3. */
    wifi_csi_config_t csi_config;
    memset(&csi_config, 0, sizeof(csi_config));
    csi_config.enable = 1U;
    csi_config.acquire_csi_legacy = 1U;
    csi_config.acquire_csi_ht20 = 1U;
    csi_config.acquire_csi_ht40 = 1U;
    csi_config.acquire_csi_su = 1U;
    csi_config.acquire_csi_mu = 1U;
    csi_config.acquire_csi_dcm = 1U;
    csi_config.acquire_csi_beamformed = 1U;
#if CONFIG_SOC_WIFI_MAC_VERSION_NUM >= 3
    csi_config.acquire_csi_force_lltf = 1U;
    csi_config.acquire_csi_vht = 1U;
    csi_config.acquire_csi_he_stbc_mode = ESP_CSI_ACQUIRE_STBC_SAMPLE_HELTFS;
    csi_config.val_scale_cfg = 0U;
#else
    csi_config.acquire_csi_he_stbc = ESP_CSI_ACQUIRE_STBC_SAMPLE_HELTFS;
    csi_config.val_scale_cfg = 0U;
#endif
    csi_config.dump_ack_en = 0U;
#else
    wifi_csi_config_t csi_config = {
        .lltf_en = true,
        .htltf_en = true,
        .stbc_htltf2_en = true,
        .ltf_merge_en = true,
        .channel_filter_en = false,
        .manu_scale = false,
        .shift = false,
    };
#endif

    ESP_ERROR_CHECK(esp_wifi_set_csi_config(&csi_config));
#ifdef CONFIG_CSI_C5_LLTF8_V1
    {
        wifi_csi_config_t rb;
        memset(&rb, 0, sizeof(rb));
        esp_err_t rb_err = esp_wifi_get_csi_config(&rb);
        /* In the pinned esp-idf-espsdr 6.2.0 C5 blobs, esp_wifi_get_csi_config()
         * (hal_mac_get_csi_filter) rebuilds only bits 0-11 (enable + acquire_*)
         * from MAC registers and leaves val_scale_cfg / dump_ack_en / lltf_bit_mode
         * as the caller passed them, so those fields are NOT a readback. The set
         * path (ic_csi_set_config -> phy_lltf_mask_en / phy_chan_dump_cfg) writes
         * them to PHY register 0x600A790C: bit12 = lltf_bit_mode (1 = 8-bit),
         * bit3 = val_scale_cfg bit0. The format fields are read from that register. */
        const bool api_ok = rb_err == ESP_OK && rb.enable == 1U
            && rb.acquire_csi_legacy == 1U && rb.acquire_csi_force_lltf == 1U
            && rb.acquire_csi_ht20 == 0U && rb.acquire_csi_ht40 == 0U
            && rb.acquire_csi_vht == 0U && rb.acquire_csi_su == 0U
            && rb.acquire_csi_mu == 0U && rb.acquire_csi_dcm == 0U
            && rb.acquire_csi_beamformed == 0U;
#if CONFIG_IDF_TARGET_ESP32C5 && ESP_IDF_VERSION == ESP_IDF_VERSION_VAL(6, 2, 0)
        const uint32_t phy_csi = REG_READ(0x600A790CU);
        const bool phy_ok = ((phy_csi >> 12) & 1U) == 1U && ((phy_csi >> 3) & 1U) == 0U;
#else
        const uint32_t phy_csi = 0;
        const bool phy_ok = false;   /* register map verified only for the pinned C5 SDK */
#endif
        s_c5l8_readback_ok = api_ok && phy_ok;
        rb.lltf_bit_mode = (phy_csi >> 12) & 1U;   /* log the register, not the echo */
        rb.val_scale_cfg = (phy_csi >> 3) & 1U;
        ESP_LOGI(TAG, "C5L8/v1 readback sources: api_bits0-11=%s phy_0x600A790C=0x%08lx",
                 api_ok ? "match" : "MISMATCH", (unsigned long)phy_csi);
        wifi_bandwidths_t bw = {0};
        esp_err_t bw_err = esp_wifi_get_bandwidths(WIFI_IF_STA, &bw);
        s_c5l8_sta_bw_mhz = (bw_err == ESP_OK && bw.ghz_5g == WIFI_BW20) ? 20 : 0;
        ESP_LOGI(TAG, "C5L8/v1 readback: csi_config=%s (err=%s lltf_bit_mode=%u scale=%u) "
                 "sta_bw_5g=%s (err=%s)",
                 s_c5l8_readback_ok ? "match" : "MISMATCH", esp_err_to_name(rb_err),
                 (unsigned)rb.lltf_bit_mode, (unsigned)rb.val_scale_cfg,
                 s_c5l8_sta_bw_mhz == 20 ? "20MHz" : "NOT-20MHz", esp_err_to_name(bw_err));
        if (!s_c5l8_readback_ok || s_c5l8_sta_bw_mhz != 20) {
            ESP_LOGE(TAG, "C5L8/v1 claim cannot be made; every CSI frame will be withheld");
        }
    }
#endif
    ESP_ERROR_CHECK(esp_wifi_set_csi_rx_cb(wifi_csi_callback, NULL));
    ESP_ERROR_CHECK(esp_wifi_set_csi(true));

    if (g_nvs_config.filter_mac_set) {
        ESP_LOGI(TAG, "MAC filter active: **:**:**:**:**:%02x", g_nvs_config.filter_mac[5]);
    }

    ESP_LOGI(TAG, "CSI collection initialized (node_id=%u, channel=%u)",
             (unsigned)s_node_id, (unsigned)csi_channel);
    ESP_LOGI(TAG, "edge DSP cadence=%dHz; raw CSI network cadence remains independent",
             CONFIG_EDGE_DSP_SAMPLE_HZ);

    /* RuView#521/#954: start the connected-STA traffic source so the CSI engine
     * receives a guaranteed OFDM unicast floor even when promiscuous capture is
     * starved (display builds / quiet networks). Additive to #396/#893. */
#if defined(CONFIG_CSI_AP_ONLY_SOURCE)
    {
        wifi_ap_record_t ap_rec;     /* usually associated before CSI init */
        if (esp_wifi_sta_get_ap_info(&ap_rec) == ESP_OK) {
            csi_collector_set_ap_source(ap_rec.bssid, "ap_info");
        }
    }
#endif
    csi_start_self_ping();
}

#ifdef CONFIG_CSI_C5_LLTF8_V1
void csi_collector_c5l8_serial_selftest(void)
{
    /* USB/UART frame-format check that needs no Wi-Fi: each line carries one
     * SYNTHETIC C5L8/v1 datagram as hex for the host validator. */
    uint8_t buf[C5L8_FRAME_BYTES];
    for (unsigned k = 0; k < C5L8_SELFTEST_CASES; k++) {
        size_t len = 0;
        c5l8_status_t st = c5l8_selftest_frame(k, s_node_id, buf, sizeof(buf), &len);
        printf("C5L8-SELFTEST-SYNTHETIC v1 case=%u status=%s len=%u hex=", k, c5l8_status_str(st),
               (unsigned)len);
        for (size_t i = 0; i < len; i++) printf("%02x", buf[i]);
        printf("\n");
    }
    printf("C5L8-SELFTEST-END cases=%u\n", (unsigned)C5L8_SELFTEST_CASES);
    fflush(stdout);
}

void csi_collector_get_c5l8_counts(uint32_t *emitted, uint32_t *rejected_total)
{
    uint32_t r = 0;
    for (unsigned i = 1; i < C5L8_REJECT_COUNT; i++) r += s_c5l8_reject[i];
    if (emitted) *emitted = s_c5l8_emitted;
    if (rejected_total) *rejected_total = r;
}
#endif

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

uint32_t csi_collector_get_send_ok_count(void)
{
    return s_send_ok;
}

/* The gate here is fixed at compile time (mesh-aligned bucketing, falling
 * back to elapsed time when unsynced), so there is no mode or period to
 * report. Return the explicit sentinel rather than a number that reads as
 * a real setting. */
uint8_t csi_collector_get_gate_mode(void)
{
    return CSI_GATE_NOT_CONFIGURABLE;
}

uint8_t csi_collector_get_gate_seq_period(void)
{
    return CSI_GATE_NOT_CONFIGURABLE;
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

void csi_collector_enable_data_capture(void)
{
    /* MGMT-only (RuView#396) starves the CSI callback on display-less boards
     * (RuView#521/#893): beacons alone are sparse, yield collapses to 0 pps.
     * Without a display there is no QSPI/SPI-flash cache contention with the
     * DATA-frame interrupt load, so capture DATA frames too. */
    wifi_promiscuous_filter_t filt = {
        .filter_mask = WIFI_PROMIS_FILTER_MASK_MGMT | WIFI_PROMIS_FILTER_MASK_DATA,
    };
    esp_err_t err = esp_wifi_set_promiscuous_filter(&filt);
    if (err == ESP_OK) {
        ESP_LOGI(TAG, "CSI filter upgraded to MGMT+DATA (no display, RuView#893)");
    } else {
        ESP_LOGW(TAG, "Failed to enable DATA-frame CSI capture: %s", esp_err_to_name(err));
    }
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

void csi_collector_get_he_validity(uint32_t *valid, uint32_t *invalid)
{
    if (valid)   *valid = s_he_valid;
    if (invalid) *invalid = s_he_invalid;
}
