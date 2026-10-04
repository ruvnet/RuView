/**
 * @file csi_stall_policy.c
 * @brief Pure CSI stall escalation policy (RuView#1941). See the header.
 */
#include "csi_stall_policy.h"

#include <string.h>

/* True when a is at or after b, across uint32 wrap. */
static bool at_or_after(uint32_t a, uint32_t b)
{
    return (int32_t)(a - b) >= 0;
}

void csi_stall_init(csi_stall_state_t *s, uint32_t timeout_ms, uint32_t now_ms)
{
    memset(s, 0, sizeof(*s));
    s->timeout_ms = timeout_ms;
    s->ref_ms = now_ms;
    s->stage_since_ms = now_ms;
}

csi_stall_action_t csi_stall_step(csi_stall_state_t *s, uint32_t now_ms,
                                  uint32_t last_activity_ms, bool link_up)
{
    if (last_activity_ms == 0) {
        /* Never captured anything: not armed, so a node that has not yet
         * reached the network cannot be driven into a restart loop. */
        s->ref_ms = now_ms;
        return CSI_STALL_NONE;
    }
    if (!s->armed) {
        s->armed = true;
        s->ref_ms = now_ms;
    }

    if (!link_up) {
        /* Pause: neither the idle clock nor the stage clock runs while the
         * reconnect path owns the link. */
        s->ref_ms = now_ms;
        s->stage_since_ms = now_ms;
        return CSI_STALL_NONE;
    }

    if (s->stage > 0) {
        if ((int32_t)(last_activity_ms - s->stage_since_ms) > 0) {
            s->stage = 0;
            s->ref_ms = now_ms;
            return CSI_STALL_RECOVERED;
        }
        if (s->stage >= 3) {
            return CSI_STALL_NONE;       /* restart already requested */
        }
        if ((uint32_t)(now_ms - s->stage_since_ms) >= s->timeout_ms) {
            s->stage++;
            s->stage_since_ms = now_ms;
            return (s->stage == 2) ? CSI_STALL_REASSOC : CSI_STALL_RESTART;
        }
        return CSI_STALL_NONE;
    }

    /* Healthy: idle is measured from the newer of the last activity and the
     * grace reference, so a link that just came back gets a full window. */
    uint32_t newest = at_or_after(last_activity_ms, s->ref_ms) ? last_activity_ms
                                                               : s->ref_ms;
    if ((uint32_t)(now_ms - newest) >= s->timeout_ms) {
        s->stage = 1;
        s->stage_since_ms = now_ms;
        return CSI_STALL_REARM;
    }
    return CSI_STALL_NONE;
}
