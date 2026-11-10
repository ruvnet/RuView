/* Bounded observed-link fence; not packet association attestation. */
#ifndef RUVIEW_C5_LLTF8_LIVE_GATE_H
#define RUVIEW_C5_LLTF8_LIVE_GATE_H
#include <stdbool.h>
#include <stdint.h>
/* One packed lock-free state: bit0 connected, bits31:1 event generation. */
static inline bool c5_lltf8_live_gate_matches(uint32_t before, uint32_t after,
                                             bool phy_matches)
{
    return (before & 1U) != 0 && before == after && phy_matches;
}
#endif
