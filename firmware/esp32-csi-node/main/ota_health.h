/* First-boot health check for an OTA-delivered image (ADR-379).
 *
 * With CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE the bootloader starts a freshly
 * OTA'd image in ESP_OTA_IMG_PENDING_VERIFY. Until the app confirms itself,
 * the next reset reverts to the previous slot and esp_ota_begin() refuses a
 * second OTA. Nothing confirmed it before this module existed, so every OTA
 * on a rollback-enabled node was undone by the next power cycle.
 *
 * The decision logic is a pure function kept in this header, like
 * thermal_next_state(), so the host test exercises the exact code the device
 * runs. ota_health.c only feeds it events and a clock and acts on the verdict. */
#pragma once

#include <stdbool.h>
#include <stdint.h>

typedef enum {
    OTA_HEALTH_WAIT = 0,     /* keep checking */
    OTA_HEALTH_MARK_VALID,   /* confirm the image: esp_ota_mark_app_valid_cancel_rollback() */
    OTA_HEALTH_ROLLBACK,     /* esp_ota_mark_app_invalid_rollback_and_reboot() */
} ota_health_verdict_t;

typedef struct {
    uint32_t timeout_ms;     /* deadline, measured from boot */
    uint32_t min_uptime_ms;  /* earliest the image may be confirmed; clamped to timeout_ms */
    bool     force_fail;     /* test hook: roll back where it would have confirmed */
} ota_health_cfg_t;

typedef struct {
    bool got_ip;             /* STA associated and holds an IPv4 address (latched) */
    bool csi_delivered;      /* stream sender accepted >= 1 CSI frame (latched) */
    ota_health_verdict_t verdict;
    const char *reason;      /* set once verdict leaves WAIT; static string */
} ota_health_sm_t;

static inline void ota_health_sm_init(ota_health_sm_t *sm)
{
    sm->got_ip        = false;
    sm->csi_delivered = false;
    sm->verdict       = OTA_HEALTH_WAIT;
    sm->reason        = "waiting";
}

/**
 * Advance the health check.
 *
 * Pure: no globals, no hardware. Both signals latch, because each proves a
 * capability the image has, and a later AP outage is not evidence against the
 * image. Once the verdict leaves WAIT it is terminal: later calls return it
 * unchanged, so a caller that polls after acting cannot act twice.
 *
 * @param sm             State, initialised with ota_health_sm_init().
 * @param cfg            Timeout, minimum uptime, force-fail hook.
 * @param got_ip         Event seen since the last step: STA got an IP.
 * @param csi_delivered  Event seen since the last step: a CSI frame was sent.
 * @param now_ms         Milliseconds since boot.
 * @return The verdict. sm->reason names why, for the log.
 */
static inline ota_health_verdict_t ota_health_step(ota_health_sm_t *sm,
                                                   const ota_health_cfg_t *cfg,
                                                   bool got_ip,
                                                   bool csi_delivered,
                                                   uint32_t now_ms)
{
    if (sm->verdict != OTA_HEALTH_WAIT) {
        return sm->verdict;
    }

    sm->got_ip        = sm->got_ip || got_ip;
    sm->csi_delivered = sm->csi_delivered || csi_delivered;

    /* A minimum uptime above the timeout could never be satisfied; that would
     * turn every OTA into a rollback. Clamp rather than trust the config. */
    uint32_t min_up = cfg->min_uptime_ms < cfg->timeout_ms
                    ? cfg->min_uptime_ms : cfg->timeout_ms;

    if (sm->got_ip && sm->csi_delivered && now_ms >= min_up) {
        if (cfg->force_fail) {
            sm->verdict = OTA_HEALTH_ROLLBACK;
            sm->reason  = "forced failure (CONFIG_OTA_HEALTH_FORCE_FAIL)";
        } else {
            sm->verdict = OTA_HEALTH_MARK_VALID;
            sm->reason  = "Wi-Fi got IP and CSI frame delivered";
        }
        return sm->verdict;
    }

    if (now_ms >= cfg->timeout_ms) {
        sm->verdict = OTA_HEALTH_ROLLBACK;
        if (!sm->got_ip && !sm->csi_delivered) {
            sm->reason = "timeout: no IP and no CSI frame delivered";
        } else if (!sm->got_ip) {
            sm->reason = "timeout: no IP";
        } else {
            sm->reason = "timeout: no CSI frame delivered";
        }
        return sm->verdict;
    }

    return OTA_HEALTH_WAIT;
}

/* Device side (ota_health.c). No-ops when the bootloader has no rollback. */

/** Call once, early in app_main. Starts the check only if the running image
 *  is ESP_OTA_IMG_PENDING_VERIFY; otherwise logs the state and returns. */
void ota_health_start(void);

/** Call from the IP_EVENT_STA_GOT_IP handler. Cheap; safe from any task. */
void ota_health_note_got_ip(void);
