/**
 * @file c5_lltf8.h
 * @brief C5L8/v1 producer for ESP32-C5 8-bit L-LTF CSI.
 *
 * Wire layout (142 bytes), per RuView docs/validation/2026-11-10-c5-native-lltf8-contract.md
 * and the server decoder room_learner/native_c5.rs:
 *   [0..20)    ADR-018 header: magic 0xC5110001, node, antennas=1, slots=53, freq MHz,
 *              sequence, rssi, noise, PPDU=0 (non-HT), flags (only bit4 sync / bit5 sanitized)
 *   [20..126)  106 bytes of 8-bit L-LTF I/Q exactly as the SDK delivered them
 *              (first 4 bytes zeroed when the SDK marks the first word invalid)
 *   [126..142) suffix: "C5L8", version 1, component bits 8, imag/real order ID 1,
 *              L-LTF 53-tone order ID 1, matched-format readback 1, original BB format 1,
 *              secondary 0, STA bandwidth readback 20, estimate valid 1,
 *              first-word-invalid 0/1, original byte count 106 (LE u16)
 *
 * The builder only emits a frame when every claimed field is true for this callback; anything
 * else is refused with a reason and no raw samples. This is a format claim, not a hardware,
 * phase or sensing qualification.
 *
 * Pure C: no ESP-IDF includes, so the same code is compiled into the host test harness.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define C5L8_HEADER_BYTES     20
#define C5L8_SLOTS            53
#define C5L8_IQ_BYTES         (C5L8_SLOTS * 2)   /* 106 */
#define C5L8_SUFFIX_BYTES     16
#define C5L8_FRAME_BYTES      (C5L8_HEADER_BYTES + C5L8_IQ_BYTES + C5L8_SUFFIX_BYTES) /* 142 */

#define C5L8_MAGIC            0xC5110001u
#define C5L8_FLAG_SYNC_VALID  0x10u   /* ADR-018 bit4: advisory cross-node sync valid */
#define C5L8_FLAG_SANITIZED   0x20u   /* ADR-018 bit5: first word zeroed */

/** SDK RX_BB_FORMAT_11G == RX_BB_FORMAT_11A == 1 (original non-HT OFDM). */
#define C5L8_BB_FORMAT_NON_HT_OFDM 1u
#define C5L8_DEFAULT_CHANNEL  48u     /* 5240 MHz */

typedef enum {
    C5L8_OK = 0,
    C5L8_REJECT_NO_READBACK,      /* SDK CSI config readback did not match the 8-bit L-LTF request */
    C5L8_REJECT_STA_BANDWIDTH,    /* STA bandwidth readback is not 20 MHz */
    C5L8_REJECT_BB_FORMAT,        /* not original non-HT OFDM (11a/11g) */
    C5L8_REJECT_SECONDARY,        /* secondary channel present */
    C5L8_REJECT_ESTIMATE_INVALID, /* SDK channel estimate not valid */
    C5L8_REJECT_LENGTH,           /* callback length is not 106 bytes */
    C5L8_REJECT_CHANNEL,          /* not a supported 20 MHz 5 GHz primary channel */
    C5L8_REJECT_BUFFER,           /* output buffer too small or NULL input */
    C5L8_REJECT_COUNT
} c5l8_status_t;

/** Everything the builder needs from one CSI callback plus the boot-time SDK readback. */
typedef struct {
    uint8_t        node_id;
    uint32_t       sequence;
    uint8_t        primary_channel;     /* rx_ctrl.channel */
    int8_t         rssi;
    int8_t         noise_floor;
    uint8_t        bb_format;           /* rx_ctrl.cur_bb_format */
    uint8_t        secondary_channel;   /* rx_ctrl.second */
    bool           estimate_valid;      /* rx_ctrl.rx_channel_estimate_info_vld */
    bool           first_word_invalid;  /* info->first_word_invalid */
    bool           sync_valid;          /* ESP-NOW / 802.15.4 sync helper reports valid */
    bool           format_readback_ok;  /* esp_wifi_get_csi_config matched 8-bit forced L-LTF */
    uint8_t        sta_bandwidth_mhz;   /* esp_wifi_get_bandwidths readback for the active band */
    const uint8_t *iq;
    uint16_t       iq_len;
} c5l8_input_t;

/** True for IEEE 20 MHz 5 GHz primaries 36-64, 100-144 (step 4) and 149-177 (step 4). */
bool c5l8_channel_supported(uint8_t channel);

/** Builds one C5L8/v1 datagram. On C5L8_OK, *out_len == C5L8_FRAME_BYTES. */
c5l8_status_t c5l8_build(const c5l8_input_t *in, uint8_t *buf, size_t buf_len, size_t *out_len);

/** Number of self-test cases c5l8_selftest_frame() knows. */
#define C5L8_SELFTEST_CASES 2

/**
 * Builds a SYNTHETIC C5L8/v1 frame for the USB serial format check (no Wi-Fi, no radio):
 * deterministic IQ pattern, readback/bandwidth inputs asserted rather than read from the SDK.
 *   case 0: channel 48 (5240 MHz), sync valid, first word valid   -> flags 0x10
 *   case 1: channel 48 (5240 MHz), sync valid, first word invalid -> flags 0x30
 * It checks the serializer only; it is never sensing, packing or hardware evidence.
 */
c5l8_status_t c5l8_selftest_frame(unsigned which, uint8_t node_id, uint8_t *buf, size_t buf_len,
                                  size_t *out_len);

/** Static reason string for logs and counters; never contains sample data. */
const char *c5l8_status_str(c5l8_status_t status);

#ifdef __cplusplus
}
#endif
