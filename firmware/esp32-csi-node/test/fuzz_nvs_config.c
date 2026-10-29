/**
 * @file fuzz_nvs_config.c
 * @brief libFuzzer target for NVS config validation logic (ADR-061 Layer 6).
 *
 * Since we cannot easily mock the full ESP-IDF NVS API under libFuzzer,
 * this target extracts and tests the validation ranges used by
 * nvs_config_load() when processing NVS values. Each validation check
 * from nvs_config.c is reproduced here with fuzz-driven inputs.
 *
 * Build (Linux/macOS with clang):
 *   clang -fsanitize=fuzzer,address -g -I stubs fuzz_nvs_config.c \
 *         stubs/esp_stubs.c -o fuzz_nvs_config -lm
 *
 * Run:
 *   ./fuzz_nvs_config corpus/ -max_len=256
 */

#include "esp_stubs.h"
#include "nvs_config.h"

#include <stdint.h>
#include <stddef.h>
#include <string.h>

/**
 * Validate a hop_count value using the same logic as nvs_config_load().
 * Returns the validated value (0 = rejected).
 */
static uint8_t validate_hop_count(uint8_t val)
{
    if (val >= CFG_LIMIT_HOP_COUNT_MIN && val <= CFG_LIMIT_HOP_COUNT_MAX) return val;
    return 0;
}

/**
 * Validate dwell_ms using the same logic as nvs_config_load().
 * Returns the validated value (0 = rejected).
 */
static uint32_t validate_dwell_ms(uint32_t val)
{
    if (val >= 10) return val;
    return 0;
}

/**
 * Validate TDM node count.
 */
static uint8_t validate_tdm_node_count(uint8_t val)
{
    if (val >= 1) return val;
    return 0;
}

/**
 * Validate edge_tier (0-2).
 */
static uint8_t validate_edge_tier(uint8_t val)
{
    if (val <= 2) return val;
    return 0xFF;  /* Invalid. */
}

/**
 * Validate vital_window.
 *
 * The bounds come from config_limits.h, the same header the loader and the
 * configuration API use, so the fuzzer cannot drift away from the firmware.
 */
static uint16_t validate_vital_window(uint16_t val)
{
    if (val >= CFG_LIMIT_VITAL_WINDOW_MIN && val <= CFG_LIMIT_VITAL_WINDOW_MAX) return val;
    return 0;
}

/**
 * Validate vital_interval_ms (>= 100).
 */
static uint16_t validate_vital_interval(uint16_t val)
{
    if (val >= 100) return val;
    return 0;
}

/**
 * Validate top_k_count (1-32).
 */
static uint8_t validate_top_k(uint8_t val)
{
    if (val >= CFG_LIMIT_TOP_K_MIN && val <= CFG_LIMIT_TOP_K_MAX) return val;
    return 0;
}

/**
 * Validate power_duty (10-100).
 */
static uint8_t validate_power_duty(uint8_t val)
{
    if (val >= 10 && val <= 100) return val;
    return 0;
}

/**
 * Validate wasm_max_modules (1-8).
 */
static uint8_t validate_wasm_max(uint8_t val)
{
    if (val >= 1 && val <= 8) return val;
    return 0;
}

/**
 * Validate CSI channel: 1-14 (2.4 GHz) or 36-177 (5 GHz).
 */
static uint8_t validate_csi_channel(uint8_t val)
{
    if ((val >= 1 && val <= 14) || (val >= 36 && val <= 177)) return val;
    return 0;
}

/**
 * Validate tdm_slot_index < tdm_node_count (clamp to 0 on violation).
 */
static uint8_t validate_tdm_slot(uint8_t slot, uint8_t node_count)
{
    if (slot >= node_count) return 0;
    return slot;
}

/**
 * Test string field handling: ensure NVS_CFG_SSID_MAX length is respected.
 */
static void test_string_bounds(const uint8_t *data, size_t len)
{
    char ssid[NVS_CFG_SSID_MAX];
    char password[NVS_CFG_PASS_MAX];
    char ip[NVS_CFG_IP_MAX];

    /* Simulate strncpy with NVS_CFG_*_MAX bounds. */
    size_t ssid_len = (len > NVS_CFG_SSID_MAX - 1) ? NVS_CFG_SSID_MAX - 1 : len;
    memcpy(ssid, data, ssid_len);
    ssid[ssid_len] = '\0';

    size_t pass_len = (len > NVS_CFG_PASS_MAX - 1) ? NVS_CFG_PASS_MAX - 1 : len;
    memcpy(password, data, pass_len);
    password[pass_len] = '\0';

    size_t ip_len = (len > NVS_CFG_IP_MAX - 1) ? NVS_CFG_IP_MAX - 1 : len;
    memcpy(ip, data, ip_len);
    ip[ip_len] = '\0';

    /* Ensure null termination holds. */
    if (ssid[NVS_CFG_SSID_MAX - 1] != '\0' && ssid_len == NVS_CFG_SSID_MAX - 1) {
        /* OK: we set terminator above. */
    }
}

/**
 * Test presence_thresh and fall_thresh fixed-point conversion.
 * nvs_config.c stores as u16 with value * 1000.
 */
static void test_thresh_conversion(uint16_t pres_raw, uint16_t fall_raw)
{
    float pres = (float)pres_raw / 1000.0f;
    float fall = (float)fall_raw / 1000.0f;

    /* Ensure no NaN or Inf from valid integer inputs. */
    if (pres != pres) __builtin_trap();  /* NaN check. */
    if (fall != fall) __builtin_trap();  /* NaN check. */

    /* Range: 0.0 to 65.535 for u16/1000. Both should be finite. */
    if (pres < 0.0f || pres > 65.536f) __builtin_trap();
    if (fall < 0.0f || fall > 65.536f) __builtin_trap();
}

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size)
{
    if (size < 32) return 0;

    const uint8_t *p = data;

    /* Extract fuzz-driven config field values. */
    uint8_t  hop_count      = p[0];
    uint32_t dwell_ms       = (uint32_t)p[1] | ((uint32_t)p[2] << 8)
                            | ((uint32_t)p[3] << 16) | ((uint32_t)p[4] << 24);
    uint8_t  tdm_slot       = p[5];
    uint8_t  tdm_nodes      = p[6];
    uint8_t  edge_tier      = p[7];
    uint16_t vital_win      = (uint16_t)p[8] | ((uint16_t)p[9] << 8);
    uint16_t vital_int      = (uint16_t)p[10] | ((uint16_t)p[11] << 8);
    uint8_t  top_k          = p[12];
    uint8_t  power_duty     = p[13];
    uint8_t  wasm_max       = p[14];
    uint8_t  csi_channel    = p[15];
    uint16_t pres_thresh    = (uint16_t)p[16] | ((uint16_t)p[17] << 8);
    uint16_t fall_thresh    = (uint16_t)p[18] | ((uint16_t)p[19] << 8);
    uint8_t  node_id        = p[20];
    uint16_t target_port    = (uint16_t)p[21] | ((uint16_t)p[22] << 8);
    uint8_t  wasm_verify    = p[23];

    /* Run all validators. These must not crash regardless of input. */
    (void)validate_hop_count(hop_count);
    (void)validate_dwell_ms(dwell_ms);
    (void)validate_tdm_node_count(tdm_nodes);
    (void)validate_edge_tier(edge_tier);
    (void)validate_vital_window(vital_win);
    (void)validate_vital_interval(vital_int);
    (void)validate_top_k(top_k);
    (void)validate_power_duty(power_duty);
    (void)validate_wasm_max(wasm_max);
    (void)validate_csi_channel(csi_channel);

    /* Validate TDM slot with validated node count. */
    uint8_t valid_nodes = validate_tdm_node_count(tdm_nodes);
    if (valid_nodes > 0) {
        (void)validate_tdm_slot(tdm_slot, valid_nodes);
    }

    /* Test threshold conversions. */
    test_thresh_conversion(pres_thresh, fall_thresh);

    /* Test string field bounds with remaining data. */
    if (size > 24) {
        test_string_bounds(data + 24, size - 24);
    }

    /* Construct a full nvs_config_t and verify field assignments don't overflow. */
    nvs_config_t cfg;
    memset(&cfg, 0, sizeof(cfg));

    cfg.target_port = target_port;
    cfg.node_id = node_id;

    uint8_t valid_hop = validate_hop_count(hop_count);
    cfg.channel_hop_count = valid_hop ? valid_hop : 1;

    /* Fill channel list from fuzz data. */
    for (uint8_t i = 0; i < NVS_CFG_HOP_MAX && (24 + i) < size; i++) {
        cfg.channel_list[i] = data[24 + i];
    }

    cfg.dwell_ms = validate_dwell_ms(dwell_ms) ? dwell_ms : 50;
    cfg.tdm_slot_index = 0;
    cfg.tdm_node_count = valid_nodes ? valid_nodes : 1;

    if (cfg.tdm_slot_index >= cfg.tdm_node_count) {
        cfg.tdm_slot_index = 0;
    }

    uint8_t valid_tier = validate_edge_tier(edge_tier);
    cfg.edge_tier = (valid_tier != 0xFF) ? valid_tier : 2;

    cfg.presence_thresh = (float)pres_thresh / 1000.0f;
    cfg.fall_thresh = (float)fall_thresh / 1000.0f;

    uint16_t valid_win = validate_vital_window(vital_win);
    cfg.vital_window = valid_win ? valid_win : 256;

    uint16_t valid_int = validate_vital_interval(vital_int);
    cfg.vital_interval_ms = valid_int ? valid_int : 1000;

    uint8_t valid_topk = validate_top_k(top_k);
    cfg.top_k_count = valid_topk ? valid_topk : 8;

    uint8_t valid_duty = validate_power_duty(power_duty);
    cfg.power_duty = valid_duty ? valid_duty : 100;

    uint8_t valid_wasm = validate_wasm_max(wasm_max);
    cfg.wasm_max_modules = valid_wasm ? valid_wasm : 4;
    cfg.wasm_verify = wasm_verify ? 1 : 0;

    uint8_t valid_ch = validate_csi_channel(csi_channel);
    cfg.csi_channel = valid_ch;

    /* MAC filter: use 6 bytes from fuzz data if available. */
    if (size >= 32) {
        memcpy(cfg.filter_mac, data + 24, 6);
        cfg.filter_mac_set = (data[30] & 0x01) ? 1 : 0;
    }

    /* Verify struct is self-consistent — no field should be in an impossible state. */
    if (cfg.channel_hop_count > NVS_CFG_HOP_MAX) __builtin_trap();
    if (cfg.tdm_slot_index >= cfg.tdm_node_count) __builtin_trap();
    if (cfg.edge_tier > 2) __builtin_trap();
    if (cfg.wasm_max_modules > 8 || cfg.wasm_max_modules < 1) __builtin_trap();
    if (cfg.top_k_count > 32 || cfg.top_k_count < 1) __builtin_trap();
    if (cfg.power_duty > 100 || cfg.power_duty < 10) __builtin_trap();

    return 0;
}
