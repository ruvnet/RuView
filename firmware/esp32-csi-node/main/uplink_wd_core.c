/**
 * @file uplink_wd_core.c
 * @brief Uplink watchdog decision logic. See uplink_wd_core.h.
 */
#include "uplink_wd_core.h"

#include <stddef.h>

uint8_t uwd_step(uwd_state_t *st, int64_t now_us,
                 uint32_t ok_total, uint32_t fail_total,
                 int64_t last_bulk_ok_us, uint32_t timeout_s,
                 int64_t *stalled_s)
{
    /* Deltas first, every tick, armed or not: the first armed tick must see
     * only its own window, not everything since boot. */
    uint32_t d_ok = 0, d_fail = 0;
    if (st->primed) {
        d_ok   = ok_total - st->prev_ok;
        d_fail = fail_total - st->prev_fail;
    }
    bool have_delta = st->primed;
    st->prev_ok = ok_total;
    st->prev_fail = fail_total;
    st->primed = true;

    if (last_bulk_ok_us == 0) {
        st->low_since_us = 0;
        return UWD_RULE_NONE;          /* never delivered -- not armed yet */
    }

    int64_t timeout_us = (int64_t)timeout_s * 1000000LL;

    /* Rule 1: no accepted bulk send at all. */
    int64_t idle_us = now_us - last_bulk_ok_us;
    if (idle_us >= timeout_us) {
        if (stalled_s) *stalled_s = idle_us / 1000000LL;
        return UWD_RULE_NO_SEND;
    }

    /* Rule 2: sustained near-total bulk failure. A tick with too few
     * attempts to judge is not low, so it resets the streak -- a node with
     * nothing to send must never accumulate one. */
    uint64_t attempts = (uint64_t)d_ok + d_fail;
    bool low = have_delta && attempts >= UWD_MIN_ATTEMPTS &&
               (uint64_t)d_ok * 100u < attempts * UWD_MIN_OK_PCT;
    if (!low) {
        st->low_since_us = 0;
        return UWD_RULE_NONE;
    }
    if (st->low_since_us == 0) {
        /* Counted from the END of the first low tick: conservative by one
         * tick, never early. */
        st->low_since_us = now_us;
        return UWD_RULE_NONE;
    }
    int64_t low_us = now_us - st->low_since_us;
    if (low_us >= timeout_us) {
        if (stalled_s) *stalled_s = low_us / 1000000LL;
        return UWD_RULE_LOW_OK;
    }
    return UWD_RULE_NONE;
}

uint8_t uwd_path_step(uwd_path_state_t *st, int64_t now_us,
                      uint32_t ping_success, bool associated,
                      uint32_t timeout_s, int64_t *stalled_s)
{
    if (!associated) {
        st->dead_since_us = 0;          /* AP gone: the reconnect loop owns it */
        return UWD_RULE_NONE;
    }
    if (ping_success > 0) {
        st->armed = true;
        st->dead_since_us = 0;
        return UWD_RULE_NONE;
    }
    if (!st->armed) {
        return UWD_RULE_NONE;           /* gateway never answered this boot */
    }
    if (st->dead_since_us == 0) {
        /* Counted from the END of the first dead window: one window late,
         * never early (same convention as rule 2). */
        st->dead_since_us = now_us;
        return UWD_RULE_NONE;
    }
    int64_t dead_us = now_us - st->dead_since_us;
    if (dead_us >= (int64_t)timeout_s * 1000000LL) {
        if (stalled_s) *stalled_s = dead_us / 1000000LL;
        return UWD_RULE_DEAD_PATH;
    }
    return UWD_RULE_NONE;
}
