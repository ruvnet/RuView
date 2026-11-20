/**
 * @file uplink_wd_core.h
 * @brief Uplink watchdog decision logic -- pure, no ESP-IDF.
 *
 * main.c's uplink_watchdog_tick() feeds this every ~10 s and reboots when it
 * returns a rule. Split out so tests/host/test_uplink_wd.c can replay real
 * send-counter traces through it without hardware.
 *
 * Three rules, all over CONFIG_UPLINK_WATCHDOG_TIMEOUT_S:
 *   1. no accepted bulk send at all (the original watchdog);
 *   2. every tick for the whole timeout saw < UWD_MIN_OK_PCT % of its bulk
 *      sends accepted, with >= UWD_MIN_ATTEMPTS attempts in each;
 *   3. associated, but no gateway self-ping answered in any link-watchdog
 *      window for the whole timeout (uwd_path_step, below).
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
#define UWD_RULE_DEAD_PATH 3

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

/**
 * Rule 3: the node is associated but its IP path is dead.
 *
 * Rules 1-2 judge sendto() acceptance. A node was measured staying associated
 * for 94 min, capturing CSI at ~50 fps, with every send accepted and none
 * delivered (no ping answer, zero packets at the server). The ADR-427 self-ping
 * to the gateway saw 100 % loss the whole time, but the link watchdog's guard
 * 1 deliberately ignores outage windows. This rule reads the same windows: no
 * successful gateway ping in any ASSOCIATED window for the whole timeout ->
 * reboot.
 *
 *  - Arms only after a window with >= 1 successful ping since boot, so a node
 *    whose gateway never answers ICMP (or that never started its self-ping)
 *    does not reboot-loop; after a reboot it must reach the gateway again
 *    first.
 *  - An unassociated window resets the streak: reconnecting is main.c's
 *    unbounded-retry job, and an AP-wide outage looks exactly like that.
 *  - Any successful ping resets it, so partial loss (a slow or lossy link,
 *    the link watchdog's domain) never accumulates.
 * Known exposure: the gateway down (or not answering ICMP) for longer than the
 * timeout while the AP stays up reboots each armed node once.
 */
typedef struct {
    bool    armed;            /* a gateway ping has succeeded since boot */
    int64_t dead_since_us;    /* 0 = the last associated window had a success */
} uwd_path_state_t;

/** One completed link-watchdog window (ping_total = successes + timeouts).
 *  Returns UWD_RULE_DEAD_PATH or UWD_RULE_NONE; on fire `*stalled_s` is how
 *  long no associated window has had a successful ping. */
uint8_t uwd_path_step(uwd_path_state_t *st, int64_t now_us,
                      uint32_t ping_success, bool associated,
                      uint32_t timeout_s, int64_t *stalled_s);

#ifdef __cplusplus
}
#endif
