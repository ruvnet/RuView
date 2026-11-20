/**
 * @file fuzz_csi_serialize.c
 * @brief libFuzzer target for csi_serialize_frame() (ADR-061 Layer 6).
 *
 * Takes fuzz input and constructs wifi_csi_info_t structs with random
 * field values including extreme boundaries. Verifies that
 * csi_serialize_frame() never crashes, triggers ASAN, or causes UBSAN.
 *
 * Build (Linux/macOS with clang):
 *   make fuzz_serialize
 *
 * Run:
 *   ./fuzz_serialize corpus/ -max_len=2048
 */

#include "esp_stubs.h"

/* Provide the globals that csi_collector.c references. */
#include "nvs_config.h"
nvs_config_t g_nvs_config;

/* Pull in the serialization function. */
#include "csi_collector.h"

#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <stdlib.h>

/**
 * Helper: read a value from the fuzz data, advancing the cursor.
 * Returns 0 if insufficient data remains.
 */
static size_t fuzz_read(const uint8_t **data, size_t *size,
                        void *out, size_t n)
{
    if (*size < n) {
        memset(out, 0, n);
        return 0;
    }
    memcpy(out, *data, n);
    *data += n;
    *size -= n;
    return n;
}

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size)
{
    if (size < 8) {
        return 0;  /* Need at least a few control bytes. */
    }

    const uint8_t *cursor = data;
    size_t remaining = size;

    /* Parse control bytes from fuzz input. */
    uint8_t  test_case;
    int16_t  iq_len_raw;
    int8_t   rssi;
    uint8_t  channel;
    int8_t   noise_floor;
    uint8_t  out_buf_scale;  /* Controls output buffer size: 0-255. */
    /* ADR-110: fuzz the new HE-branch + legacy-branch input fields too so
     * the byte 18/19 encoding code path is exercised. */
    uint8_t  he_inputs[2] = {0};  /* cur_bb_format (4 bits) + second (4 bits) packed */
    uint8_t  legacy_inputs = 0;   /* sig_mode (2) + cwb (1) + stbc (1) packed */

    fuzz_read(&cursor, &remaining, &test_case, 1);
    fuzz_read(&cursor, &remaining, &iq_len_raw, 2);
    fuzz_read(&cursor, &remaining, &rssi, 1);
    fuzz_read(&cursor, &remaining, &channel, 1);
    fuzz_read(&cursor, &remaining, &noise_floor, 1);
    fuzz_read(&cursor, &remaining, &out_buf_scale, 1);
    fuzz_read(&cursor, &remaining, he_inputs, 2);
    fuzz_read(&cursor, &remaining, &legacy_inputs, 1);

    /* --- Test case 0: Normal operation with fuzz-controlled values --- */

    wifi_csi_info_t info;
    memset(&info, 0, sizeof(info));
    info.rx_ctrl.rssi = rssi;
    info.rx_ctrl.channel = channel & 0x0F;  /* 4-bit field */
    info.rx_ctrl.noise_floor = noise_floor;
    /* ADR-110: feed both branch families. Only the active branch (chosen
     * at compile time by CONFIG_SOC_WIFI_HE_SUPPORT) will read its fields;
     * the other set is set-but-not-read. Both must be assignable without
     * triggering UBSAN bitfield-overflow. */
    info.rx_ctrl.cur_bb_format = he_inputs[0] & 0x0F;   /* 0..15 valid input space */
    info.rx_ctrl.second        = he_inputs[1] & 0x0F;
    info.rx_ctrl.sig_mode      = legacy_inputs & 0x03;
    info.rx_ctrl.cwb           = (legacy_inputs >> 2) & 0x01;
    info.rx_ctrl.stbc          = (legacy_inputs >> 3) & 0x01;
    info.first_word_invalid    = (test_case & 0x80) != 0;

    /* Use remaining fuzz data as I/Q buffer content. */
    uint16_t iq_len;
    if (iq_len_raw < 0) {
        iq_len = 0;
    } else if (iq_len_raw > (int16_t)remaining) {
        iq_len = (uint16_t)remaining;
    } else {
        iq_len = (uint16_t)iq_len_raw;
    }

    int8_t iq_buf[CSI_MAX_FRAME_SIZE];
    if (iq_len > 0 && remaining > 0) {
        uint16_t copy = (iq_len > remaining) ? (uint16_t)remaining : iq_len;
        memcpy(iq_buf, cursor, copy);
        /* Zero-fill the rest if iq_len > available data. */
        if (copy < iq_len) {
            memset(iq_buf + copy, 0, iq_len - copy);
        }
        info.buf = iq_buf;
    } else {
        info.buf = iq_buf;
        memset(iq_buf, 0, sizeof(iq_buf));
    }
    info.len = (int16_t)iq_len;

    /* Output buffer: scale from tiny (1 byte) to full size. */
    uint8_t out_buf[CSI_MAX_FRAME_SIZE + 64];
    size_t out_len;
    if (out_buf_scale == 0) {
        out_len = 0;
    } else if (out_buf_scale < 20) {
        /* Small buffer: test buffer-too-small path. */
        out_len = (size_t)out_buf_scale;
    } else {
        /* Normal/large buffer. */
        out_len = sizeof(out_buf);
    }

    /* Call the function under test. Must not crash. */
    size_t result = csi_serialize_frame(&info, out_buf, out_len);

    /* Basic sanity: result must be 0 (error) or <= out_len. */
    if (result > out_len) {
        __builtin_trap();  /* Buffer overflow detected. */
    }
    if (result >= CSI_HEADER_SIZE + iq_len && info.first_word_invalid && iq_len > 0) {
        size_t invalid_len = iq_len < 4 ? iq_len : 4;
        for (size_t i = 0; i < invalid_len; i++) {
            if (out_buf[CSI_HEADER_SIZE + i] != 0) {
                __builtin_trap();  /* Invalid hardware prefix escaped sanitation. */
            }
        }
        if ((out_buf[19] & CSI_FLAG_FIRST_WORD_SANITIZED) == 0) {
            __builtin_trap();  /* Sanitized payload must carry provenance. */
        }
    }

    /* --- Test case 1: NULL info pointer --- */
    if (test_case & 0x01) {
        result = csi_serialize_frame(NULL, out_buf, sizeof(out_buf));
        if (result != 0) {
            __builtin_trap();  /* NULL info should return 0. */
        }
    }

    /* --- Test case 2: NULL output buffer --- */
    if (test_case & 0x02) {
        result = csi_serialize_frame(&info, NULL, sizeof(out_buf));
        if (result != 0) {
            __builtin_trap();  /* NULL buf should return 0. */
        }
    }

    /* --- Test case 3: NULL I/Q buffer in info --- */
    if (test_case & 0x04) {
        wifi_csi_info_t null_iq_info = info;
        null_iq_info.buf = NULL;
        result = csi_serialize_frame(&null_iq_info, out_buf, sizeof(out_buf));
        if (result != 0) {
            __builtin_trap();  /* NULL info->buf should return 0. */
        }
    }

    /* --- Test case 4: Extreme channel values --- */
    if (test_case & 0x08) {
        wifi_csi_info_t extreme_info = info;
        extreme_info.buf = iq_buf;

        /* Channel 0 (invalid). */
        extreme_info.rx_ctrl.channel = 0;
        csi_serialize_frame(&extreme_info, out_buf, sizeof(out_buf));

        /* Channel 15 (max 4-bit value, invalid for WiFi). */
        extreme_info.rx_ctrl.channel = 15;
        csi_serialize_frame(&extreme_info, out_buf, sizeof(out_buf));
    }

    /* --- Test case 5: Extreme RSSI values --- */
    if (test_case & 0x10) {
        wifi_csi_info_t rssi_info = info;
        rssi_info.buf = iq_buf;

        rssi_info.rx_ctrl.rssi = -128;
        csi_serialize_frame(&rssi_info, out_buf, sizeof(out_buf));

        rssi_info.rx_ctrl.rssi = 127;
        csi_serialize_frame(&rssi_info, out_buf, sizeof(out_buf));
    }

    /* --- Test case 6: Zero-length I/Q --- */
    if (test_case & 0x20) {
        wifi_csi_info_t zero_info = info;
        zero_info.buf = iq_buf;
        zero_info.len = 0;
        result = csi_serialize_frame(&zero_info, out_buf, sizeof(out_buf));
        /* len=0 means frame_size = CSI_HEADER_SIZE + 0 = 20 bytes. */
        if (result != 0 && result != CSI_HEADER_SIZE) {
            /* Either 0 (rejected) or exactly the header size is acceptable. */
        }
    }

    /* --- Test case 7: Output buffer exactly header size --- */
    if (test_case & 0x40) {
        wifi_csi_info_t hdr_info = info;
        hdr_info.buf = iq_buf;
        hdr_info.len = 4;  /* Small I/Q. */
        /* Buffer exactly header_size + iq_len = 24 bytes. */
        uint8_t tight_buf[CSI_HEADER_SIZE + 4];
        result = csi_serialize_frame(&hdr_info, tight_buf, sizeof(tight_buf));
        if (result > sizeof(tight_buf)) {
            __builtin_trap();
        }
    }

    return 0;
}
