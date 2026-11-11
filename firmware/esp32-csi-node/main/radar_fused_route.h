#ifndef RUVIEW_RADAR_FUSED_ROUTE_H
#define RUVIEW_RADAR_FUSED_ROUTE_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <math.h>

/* Existing ADR-063 little-endian 48-byte MR60 fused packet; never raw CSI. */
static inline bool ruview_mr60_fused_packet(const uint8_t *data, size_t len, uint8_t node_id)
{
    if (data == NULL || len != 48 || data[0] != 4 || data[1] != 0 ||
        data[2] != 0x11 || data[3] != 0xC5 || data[4] != node_id || data[14] != 1)
        return false;
    float motion, score, hr, br, range;
    memcpy(&motion, data + 16, 4); memcpy(&score, data + 20, 4);
    memcpy(&hr, data + 28, 4); memcpy(&br, data + 32, 4); memcpy(&range, data + 36, 4);
    return isfinite(motion) && motion >= 0 && motion <= 1e6f &&
        isfinite(score) && score >= 0 && score <= 1e6f &&
        isfinite(hr) && hr >= 0 && hr <= 250 &&
        isfinite(br) && br >= 0 && br <= 60 &&
        isfinite(range) && range >= 0 && range <= 3000 &&
        data[15] <= 100 && data[41] <= 100 && data[40] <= 1 &&
        ((data[5] & 8) != 0) == (data[40] == 1);
}
#endif
