/**
 * @file csi_stall_policy.h
 * @brief Pure escalation policy for the CSI capture watchdog (RuView#1941).
 *
 * MEASURED 2026-09-15 on an ESP32-C6 (0.8.12): the CSI capture path stopped
 * for 473 s while Wi-Fi, ping, ESP-NOW, the console and the onboarding
 * protocol all stayed healthy. The uplink watchdog cannot see this, because
 * adaptive-controller and mesh packets keep sendto() succeeding.
 *
 * This module decides WHAT to do; csi_watchdog.c does it. It has no ESP-IDF
 * dependency so the ladder is unit-tested on the host (test/test_csi_stall_policy.c).
 *
 * Ladder, each step one timeout apart, reset by any fresh capture activity:
 *   stage 1  REARM    re-apply CSI config, callbacks, promiscuous filter, self-ping
 *   stage 2  REASSOC  drop the association; main.c's reconnect path rejoins
 *   stage 3  RESTART  reboot
 *
 * While the link is down the escalation clock is paused: the reconnect logic
 * owns that state, and a REASSOC must not be mistaken for a fresh stall.
 * All times are uint32 milliseconds and wrap-safe.
 */
#ifndef CSI_STALL_POLICY_H
#define CSI_STALL_POLICY_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    CSI_STALL_NONE = 0,     /**< Healthy, or waiting; nothing to do. */
    CSI_STALL_REARM,        /**< Stage 1: re-arm the capture path. */
    CSI_STALL_REASSOC,      /**< Stage 2: force a Wi-Fi reassociation. */
    CSI_STALL_RESTART,      /**< Stage 3: reboot the node. */
    CSI_STALL_RECOVERED,    /**< Activity resumed after an escalation. */
} csi_stall_action_t;

typedef struct {
    uint32_t timeout_ms;    /**< Idle time per ladder step. */
    uint32_t ref_ms;        /**< Grace reference: armed/link-up/paused time. */
    uint32_t stage_since_ms;/**< When the current stage was entered. */
    uint8_t  stage;         /**< 0 = healthy, 1..3 = ladder position. */
    bool     armed;         /**< True once any activity has been seen. */
} csi_stall_state_t;

/** Reset to healthy with the given per-step timeout. */
void csi_stall_init(csi_stall_state_t *s, uint32_t timeout_ms, uint32_t now_ms);

/**
 * Advance the policy.
 *
 * @param last_activity_ms  Most recent capture activity, 0 if none yet. The
 *                          watchdog passes the OLDER of "last CSI callback"
 *                          and "last CSI frame sent", so a stall in either the
 *                          radio or the send path counts.
 * @param link_up           STA associated with an IP address.
 */
csi_stall_action_t csi_stall_step(csi_stall_state_t *s, uint32_t now_ms,
                                  uint32_t last_activity_ms, bool link_up);

#ifdef __cplusplus
}
#endif

#endif /* CSI_STALL_POLICY_H */
