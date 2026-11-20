#ifndef SERIAL_ONBOARDING_H
#define SERIAL_ONBOARDING_H

#include "esp_err.h"
#include "nvs_config.h"

/** Start the bounded USB serial onboarding listener.
 *
 * The listener exposes device metadata and accepts one nonce-bound
 * configuration transaction. It never prints WiFi credentials and reboots
 * only after a successful atomic NVS commit.
 */
esp_err_t serial_onboarding_start(const nvs_config_t *config);

#endif
