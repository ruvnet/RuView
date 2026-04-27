/**
 * @file rv_radio_ops_mock.c
 * @brief ADR-081 Layer 1 — Mock binding for QEMU / offline testing.
 *
 * When CONFIG_CSI_MOCK_ENABLED is set (ADR-061 QEMU flow), there is no
 * real WiFi driver to wrap. This binding provides the same ops table as
 * the ESP32 binding but records state into in-process statics and
 * accepts every call. It exists primarily to satisfy ADR-081's
 * portability acceptance test: a second binding must compile against
 * the same controller and mesh-plane code without modification.
 *
 * Only compiled when CONFIG_CSI_MOCK_ENABLED is set. Registered from
 * main.c in the mock branch.
 */

#include "sdkconfig.h"

#ifdef CONFIG_CSI_MOCK_ENABLED

#include "rv_radio_ops.h"
#include "mock_csi.h"

#include <string.h>
#include "esp_err.h"
#include "esp_log.h"

static const char *TAG = "rv_radio_mock";

static uint8_t s_channel = 6;
static uint8_t s_bw      = 20;
static uint8_t s_profile = RV_PROFILE_PASSIVE_LOW_RATE;
static uint8_t s_mode    = RV_RADIO_MODE_PASSIVE_RX;
static bool    s_csi_on  = true;

static int mock_init(void)
{
    ESP_LOGI(TAG, "mock radio ops: init");
    return ESP_OK;
}

static int mock_set_channel(uint8_t ch, uint8_t bw)
{
    s_channel = ch;
    s_bw      = (bw == 40) ? 40 : 20;
    return ESP_OK;
}

static int mock_set_mode(uint8_t mode)
{
    s_mode = mode;
    return ESP_OK;
}

static int mock_set_csi_enabled(bool en)
{
    s_csi_on = en;
    return ESP_OK;
}

static int mock_set_capture_profile(uint8_t profile_id)
{
    if (profile_id >= RV_PROFILE_COUNT) return ESP_ERR_INVALID_ARG;
    s_profile = profile_id;
    return ESP_OK;
}

static int mock_get_health(rv_radio_health_t *out)
{
    if (out == NULL) return ESP_ERR_INVALID_ARG;
    memset(out, 0, sizeof(*out));

    /* Mock yield: mirror mock_csi's generator rate so the adaptive
     * controller sees a sensible pkt_yield in QEMU. */
    out->pkt_yield_per_sec = 20;  /* MOCK_CSI_INTERVAL_MS = 50 → 20 Hz */
    out->rssi_median_dbm   = -55;
    out->noise_floor_dbm   = -95;
    out->current_channel   = s_channel;
    out->current_bw_mhz    = s_bw;
    out->current_profile   = s_profile;
    return ESP_OK;
}

static const rv_radio_ops_t s_mock_ops = {
    .init                 = mock_init,
    .set_channel          = mock_set_channel,
    .set_mode             = mock_set_mode,
    .set_csi_enabled      = mock_set_csi_enabled,
    .set_capture_profile  = mock_set_capture_profile,
    .get_health           = mock_get_health,
};

void rv_radio_ops_mock_register(void)
{
    rv_radio_ops_register(&s_mock_ops);
    ESP_LOGI(TAG, "mock radio ops registered (QEMU / offline mode)");
}

#endif /* CONFIG_CSI_MOCK_ENABLED */
