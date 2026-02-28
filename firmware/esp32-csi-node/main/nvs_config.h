/**
 * @file nvs_config.h
 * @brief Runtime configuration via NVS (Non-Volatile Storage).
 *
 * Reads WiFi credentials and aggregator target from NVS.
 * Falls back to compile-time Kconfig defaults if NVS keys are absent.
 * This allows a single firmware binary to be shipped and configured
 * per-device using the provisioning script.
 */

#ifndef NVS_CONFIG_H
#define NVS_CONFIG_H

#include <stdint.h>

/** Maximum lengths for NVS string fields. */
#define NVS_CFG_SSID_MAX     33
#define NVS_CFG_PASS_MAX     65
#define NVS_CFG_IP_MAX       16

/** Runtime configuration loaded from NVS or Kconfig defaults. */
typedef struct {
    char     wifi_ssid[NVS_CFG_SSID_MAX];
    char     wifi_password[NVS_CFG_PASS_MAX];
    char     target_ip[NVS_CFG_IP_MAX];
    uint16_t target_port;
    uint8_t  node_id;
} nvs_config_t;

/**
 * Load configuration from NVS, falling back to Kconfig defaults.
 *
 * Must be called after nvs_flash_init().
 *
 * @param cfg  Output configuration struct.
 */
void nvs_config_load(nvs_config_t *cfg);

#endif /* NVS_CONFIG_H */
