/**
 * @file uplink_wd_core.h
 * @brief Uplink watchdog decision logic -- pure, no ESP-IDF.
 *
 * main.c's uplink_watchdog_tick() feeds this every ~10 s and reboots when it
 * returns a rule. Split out so tests/host/test_uplink_wd.c can replay real
 * send-counter traces through it without hardware.
 *
 * Two rules, both over CONFIG_UPLINK_WATCHDOG_TIMEOUT_S:
 *   1. no accepted bulk send at all (the original watchdog);
 *   2. every tick for the whole timeout saw < UWD_MIN_OK_PCT % of its bulk
 *      sends accepted, with >= UWD_MIN_ATTEMPTS attempts in each.
 *
 * Rule 2 exists because of a measured TX-path stall: a node stayed associated
 * and capturing for ~10 h with every CSI send failing, but the ENOMEM backoff
 * in stream_sender.c still lets a real sendto() through every <= 2 s, so an
 * occasionally accepted packet kept rule 1's idle timer from ever reaching
 * the limit. The self-healing silences that set the default 1200 s timeout
 * (median 210 s, max 225 s, see Kconfig) are far shorter than the timeout, so
 * neither rule fires on them.
 */
#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define UWD_MIN_OK_PCT    5
#define UWD_MIN_ATTEMPTS  10

#define UWD_RULE_NONE     0
#define UWD_RULE_NO_SEND  1
#define UWD_RULE_LOW_OK   2

typedef struct {
    bool     primed;          /* prev_* hold a real sample */
    uint32_t prev_ok;
    uint32_t prev_fail;
    int64_t  low_since_us;    /* 0 = the last tick was not low */
} uwd_state_t;

/**
 * One tick. `ok_total`/`fail_total` are the monotonic since-boot bulk send
 * counters (u32, wrap-safe by unsigned subtraction); `last_bulk_ok_us` is the
 * esp_timer time of the last accepted bulk send, 0 if none since boot (the
 * watchdog is not armed until the first one). Returns a UWD_RULE_*; on a
 * non-NONE result `*stalled_s` is how long that rule's condition has held.
 */
uint8_t uwd_step(uwd_state_t *st, int64_t now_us,
                 uint32_t ok_total, uint32_t fail_total,
                 int64_t last_bulk_ok_us, uint32_t timeout_s,
                 int64_t *stalled_s);

#ifdef __cplusplus
}
#endif
