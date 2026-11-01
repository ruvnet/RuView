/* First-boot health check for an OTA-delivered image. See ota_health.h and
 * ADR-379 for why it exists; the decision itself is ota_health_step(). */
#include "ota_health.h"

#include "sdkconfig.h"

#ifdef CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE

#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_ota_ops.h"
#include "esp_timer.h"

#include "csi_collector.h"

static const char *TAG = "ota_health";

#define OTA_HEALTH_POLL_MS 1000

/* Written from the event loop task, read from the health task. A single
 * aligned bool is enough; it only ever goes false -> true. */
static volatile bool s_got_ip;

static const char *state_name(esp_ota_img_states_t st)
{
    switch (st) {
    case ESP_OTA_IMG_NEW:            return "NEW";
    case ESP_OTA_IMG_PENDING_VERIFY: return "PENDING_VERIFY";
    case ESP_OTA_IMG_VALID:          return "VALID";
    case ESP_OTA_IMG_INVALID:        return "INVALID";
    case ESP_OTA_IMG_ABORTED:        return "ABORTED";
    case ESP_OTA_IMG_UNDEFINED:      return "UNDEFINED";
    }
    return "?";
}

static void ota_health_task(void *arg)
{
    (void)arg;
    const ota_health_cfg_t cfg = {
        .timeout_ms    = (uint32_t)CONFIG_OTA_HEALTH_TIMEOUT_S * 1000u,
        .min_uptime_ms = (uint32_t)CONFIG_OTA_HEALTH_MIN_UPTIME_S * 1000u,
#ifdef CONFIG_OTA_HEALTH_FORCE_FAIL
        .force_fail    = true,
#else
        .force_fail    = false,
#endif
    };
    ota_health_sm_t sm;
    ota_health_sm_init(&sm);

    for (;;) {
        uint32_t now_ms = (uint32_t)(esp_timer_get_time() / 1000);
        ota_health_verdict_t v = ota_health_step(&sm, &cfg, s_got_ip,
                                                 csi_collector_get_send_ok_count() > 0,
                                                 now_ms);
        if (v == OTA_HEALTH_MARK_VALID) {
            esp_err_t err = esp_ota_mark_app_valid_cancel_rollback();
            if (err == ESP_OK) {
                ESP_LOGI(TAG, "marked valid after %s in %lu ms",
                         sm.reason, (unsigned long)now_ms);
            } else {
                ESP_LOGE(TAG, "mark valid failed: %s (image stays PENDING_VERIFY "
                              "and reverts on the next reset)", esp_err_to_name(err));
            }
            break;
        }
        if (v == OTA_HEALTH_ROLLBACK) {
            ESP_LOGE(TAG, "health check failed: %s, rolling back (%lu ms)",
                     sm.reason, (unsigned long)now_ms);
            /* Returns only on failure: no other bootable slot. Rebooting then
             * would just loop, so stay up; the next reset aborts this image. */
            esp_err_t err = esp_ota_mark_app_invalid_rollback_and_reboot();
            ESP_LOGE(TAG, "rollback not possible: %s -- staying on this image",
                     esp_err_to_name(err));
            break;
        }
        vTaskDelay(pdMS_TO_TICKS(OTA_HEALTH_POLL_MS));
    }
    vTaskDelete(NULL);
}

void ota_health_start(void)
{
    const esp_partition_t *running = esp_ota_get_running_partition();
    esp_ota_img_states_t st;
    esp_err_t err = esp_ota_get_state_partition(running, &st);

    /* ESP_ERR_NOT_SUPPORTED: running from factory. ESP_ERR_NOT_FOUND: no
     * otadata entry for this slot (erased otadata). Neither can be rolled
     * back, so there is nothing to confirm. */
    if (err != ESP_OK) {
        ESP_LOGI(TAG, "running %s has no OTA state (%s): no health check",
                 running ? running->label : "?", esp_err_to_name(err));
        return;
    }

    /* VALID: USB-flashed with ota_data_initial (the bootloader writes VALID)
     * or already confirmed. UNDEFINED: selected by a build without rollback;
     * the bootloader boots it unconditionally. NEW: the bootloader has no
     * rollback support, so it never promoted the image to PENDING_VERIFY.
     * None of these revert on reset. */
    if (st != ESP_OTA_IMG_PENDING_VERIFY) {
        ESP_LOGI(TAG, "running %s state %s: no health check",
                 running->label, state_name(st));
        return;
    }

    ESP_LOGW(TAG, "OTA image pending verify on %s: need IP + CSI frame within %d s "
                  "(min uptime %d s)%s",
             running->label, CONFIG_OTA_HEALTH_TIMEOUT_S,
             CONFIG_OTA_HEALTH_MIN_UPTIME_S,
#ifdef CONFIG_OTA_HEALTH_FORCE_FAIL
             " -- FORCE_FAIL test build, will roll back"
#else
             ""
#endif
             );

    if (xTaskCreate(ota_health_task, "ota_health", 4096, NULL, 2, NULL) != pdPASS) {
        /* Without the task nothing confirms the image, which is the same as
         * failing the check: the next reset reverts it. Say so. */
        ESP_LOGE(TAG, "health task create failed; image will revert on next reset");
    }
}

void ota_health_note_got_ip(void)
{
    s_got_ip = true;
}

#else /* !CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE */

/* Without bootloader rollback no image is ever PENDING_VERIFY. */
void ota_health_start(void) {}
void ota_health_note_got_ip(void) {}

#endif /* CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE */
