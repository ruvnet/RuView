/**
 * @file esp_stubs.c
 * @brief Implementation of ESP-IDF stubs for host-based fuzz testing.
 *
 * Must be compiled with: -Istubs -I../main
 * so that ESP-IDF headers resolve to stubs/ and firmware headers
 * resolve to ../main/.
 */

#include "esp_stubs.h"
#ifndef TEST_DEVICE_DIGEST
#include "edge_processing.h"
#include "wasm_runtime.h"
#endif
#include <stdint.h>
#include <string.h>

/** Monotonically increasing microsecond counter for esp_timer_get_time(). */
static int64_t s_fake_time_us = 0;

int64_t esp_timer_get_time(void)
{
    /* Advance by 50ms each call (~20 Hz CSI rate simulation). */
    s_fake_time_us += 50000;
    return s_fake_time_us;
}

/* ---- stream_sender stubs ---- */

int stream_sender_send(const uint8_t *data, size_t len)
{
    (void)data;
    return (int)len;
}

int stream_sender_init(void)
{
    return 0;
}

int stream_sender_init_with(const char *ip, uint16_t port)
{
    (void)ip; (void)port;
    return 0;
}

void stream_sender_deinit(void)
{
}

/* ---- MAC address stubs ---- */
static uint8_t g_stub_mac[6] = {0};
static esp_err_t g_stub_mac_result = ESP_OK;

void esp_stub_set_mac(const uint8_t *mac, size_t len) {
    if (len > sizeof(g_stub_mac)) len = sizeof(g_stub_mac);
    memcpy(g_stub_mac, mac, len);
}

void esp_stub_set_base_mac_result(esp_err_t result) {
    g_stub_mac_result = result;
}

esp_err_t esp_base_mac_addr_get(uint8_t mac[6]) {
    if (g_stub_mac_result != ESP_OK) return g_stub_mac_result;
    memcpy(mac, g_stub_mac, sizeof(g_stub_mac));
    return ESP_OK;
}

/* ---- wasm_runtime stubs ---- */
#ifndef TEST_DEVICE_DIGEST

void wasm_runtime_on_frame(const float *phases, const float *amplitudes,
                           const float *variances, uint16_t n_sc,
                           const edge_vitals_pkt_t *vitals)
{
    (void)phases; (void)amplitudes; (void)variances;
    (void)n_sc; (void)vitals;
}

esp_err_t wasm_runtime_init(void) { return ESP_OK; }
esp_err_t wasm_runtime_load(const uint8_t *d, uint32_t l, uint8_t *id) { (void)d; (void)l; (void)id; return ESP_OK; }
esp_err_t wasm_runtime_start(uint8_t id) { (void)id; return ESP_OK; }
esp_err_t wasm_runtime_stop(uint8_t id) { (void)id; return ESP_OK; }
esp_err_t wasm_runtime_unload(uint8_t id) { (void)id; return ESP_OK; }
void wasm_runtime_on_timer(void) {}
void wasm_runtime_get_info(wasm_module_info_t *info, uint8_t *count) { (void)info; if(count) *count = 0; }
esp_err_t wasm_runtime_set_manifest(uint8_t id, const char *n, uint32_t c, uint32_t m) { (void)id; (void)n; (void)c; (void)m; return ESP_OK; }

/* ---- mmwave_sensor stubs (ADR-063) ---- */

#include "mmwave_sensor.h"

static mmwave_state_t s_stub_mmwave = {0};

esp_err_t mmwave_sensor_init(int tx, int rx) { (void)tx; (void)rx; return ESP_ERR_NOT_FOUND; }
bool mmwave_sensor_get_state(mmwave_state_t *s) { if (s) *s = s_stub_mmwave; return false; }
const char *mmwave_type_name(mmwave_type_t t) { (void)t; return "None"; }

/* ADR-110 iter 38 — fuzz-harness stub for c6_sync_espnow_is_valid.
 * Real implementation lives in main/c6_sync_espnow.c; the fuzz target
 * (`fuzz_serialize`) only links csi_collector.c against esp_stubs.c, so
 * iter-11's `if (c6_sync_espnow_is_valid()) flags |= (1 << 4);` needs a
 * symbol here or `clang -fsanitize=fuzzer` fails with an undefined-reference
 * linker error. Returning false means the bit-4 cross-node-sync-valid flag
 * stays 0 in fuzz inputs, which is the natural fuzz semantic. */
#include <stdbool.h>
bool c6_sync_espnow_is_valid(void) { return false; }

#endif /* TEST_DEVICE_DIGEST */
