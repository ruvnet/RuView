#ifndef SERIAL_ONBOARDING_H
#define SERIAL_ONBOARDING_H

#include "esp_err.h"
#include "nvs_config.h"

/** Start the bounded USB serial onboarding listener.
 *
 * The listener exposes device metadata and accepts one nonce-bound
 * configuration transaction. It never prints WiFi credentials and reboots
 * only after a successful NVS commit (and matching persisted readback for the
 * dedicated radar command). Storage failure may leave partially applied keys;
 * re-identify before retrying rather than claiming transactional rollback.
 */
esp_err_t serial_onboarding_start(const nvs_config_t *config);

#endif
