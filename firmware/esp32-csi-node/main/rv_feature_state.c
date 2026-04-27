/**
 * @file rv_feature_state.c
 * @brief ADR-081 Layer 4 — Feature state packet helpers.
 */

#include "rv_feature_state.h"

#include <string.h>

uint32_t rv_feature_state_crc32(const uint8_t *data, size_t len)
{
    /* IEEE CRC32 (poly 0xEDB88320), bit-by-bit. Small (~80 byte) input at
     * low cadence — no need for a 1 KB lookup table. */
    uint32_t crc = 0xFFFFFFFFu;
    for (size_t i = 0; i < len; i++) {
        crc ^= data[i];
        for (int b = 0; b < 8; b++) {
            uint32_t mask = -(crc & 1u);
            crc = (crc >> 1) ^ (0xEDB88320u & mask);
        }
    }
    return ~crc;
}

void rv_feature_state_finalize(rv_feature_state_t *pkt,
                               uint8_t node_id,
                               uint16_t seq,
                               uint64_t ts_us,
                               uint8_t mode)
{
    if (pkt == NULL) {
        return;
    }
    pkt->magic    = RV_FEATURE_STATE_MAGIC;
    pkt->node_id  = node_id;
    pkt->mode     = mode;
    pkt->seq      = seq;
    pkt->ts_us    = ts_us;
    pkt->reserved = 0;

    /* CRC32 over everything except the trailing crc32 field itself. */
    const size_t crc_offset = sizeof(rv_feature_state_t) - sizeof(uint32_t);
    pkt->crc32 = rv_feature_state_crc32((const uint8_t *)pkt, crc_offset);
}
