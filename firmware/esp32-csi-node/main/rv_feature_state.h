/**
 * @file rv_feature_state.h
 * @brief ADR-081 Layer 4 — Compact on-wire feature state packet.
 *
 * The default upstream payload from a node. Replaces raw ADR-018 CSI as the
 * primary stream; ADR-018 raw frames remain available as a debug stream
 * gated by the controller / channel plan.
 *
 * Magic numbers in use across the firmware:
 *   0xC5110001 — ADR-018 raw CSI frame  (csi_collector.h)
 *   0xC5110002 — ADR-039 vitals packet  (edge_processing.h)
 *   0xC5110003 — ADR-069 feature vector (edge_processing.h)
 *   0xC5110004 — ADR-063 fused vitals   (edge_processing.h)
 *   0xC5110005 — ADR-039 compressed CSI (edge_processing.h)
 *   0xC5110006 — ADR-081 feature state  (this file) ← new
 */

#ifndef RV_FEATURE_STATE_H
#define RV_FEATURE_STATE_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Magic number for ADR-081 rv_feature_state_t. */
#define RV_FEATURE_STATE_MAGIC  0xC5110006u

/** Quality flag bits. */
#define RV_QFLAG_PRESENCE_VALID      (1u << 0)
#define RV_QFLAG_RESPIRATION_VALID   (1u << 1)
#define RV_QFLAG_HEARTBEAT_VALID     (1u << 2)
#define RV_QFLAG_ANOMALY_TRIGGERED   (1u << 3)
#define RV_QFLAG_ENV_SHIFT_DETECTED  (1u << 4)
#define RV_QFLAG_DEGRADED_MODE       (1u << 5)
#define RV_QFLAG_CALIBRATING         (1u << 6)
#define RV_QFLAG_RECOMMEND_RECAL     (1u << 7)

/**
 * Compact per-node sensing state. Sent at 1-10 Hz by default, replacing the
 * raw ADR-018 stream as the primary upstream payload.
 *
 * Mode field carries the rv_capture_profile_t value of the dominant window
 * — receivers can use it to weight features (a sample emitted under
 * RV_PROFILE_FAST_MOTION will have a stale respiration_bpm, etc.).
 *
 * CRC32 is the IEEE polynomial computed over bytes [0 .. sizeof - 4].
 */
typedef struct __attribute__((packed)) {
    uint32_t magic;             /**< RV_FEATURE_STATE_MAGIC. */
    uint8_t  node_id;           /**< Source node id. */
    uint8_t  mode;              /**< rv_capture_profile_t at emit time. */
    uint16_t seq;               /**< Monotonic per-node sequence. */
    uint64_t ts_us;             /**< Node-local microseconds. */
    float    motion_score;      /**< 0..1, 100 ms window. */
    float    presence_score;    /**< 0..1, 1 s window. */
    float    respiration_bpm;   /**< Breaths per minute. */
    float    respiration_conf;  /**< 0..1. */
    float    heartbeat_bpm;     /**< Beats per minute. */
    float    heartbeat_conf;    /**< 0..1. */
    float    anomaly_score;     /**< 0..1, z-score-derived. */
    float    env_shift_score;   /**< 0..1, baseline drift. */
    float    node_coherence;    /**< 0..1, multi-link agreement. */
    uint16_t quality_flags;     /**< RV_QFLAG_* bitmap. */
    uint16_t reserved;
    uint32_t crc32;             /**< IEEE CRC32 over bytes [0..end-4]. */
} rv_feature_state_t;

_Static_assert(sizeof(rv_feature_state_t) == 60,
               "rv_feature_state_t must be 60 bytes on the wire");

/**
 * Compute IEEE CRC32 over a byte buffer.
 *
 * Provided here (not in a separate util) because the firmware does not yet
 * have a shared CRC32 helper — only zlib's via lwIP, which is not always
 * exposed. This implementation is bit-by-bit; ~80 bytes/packet at low
 * cadence has negligible CPU cost.
 *
 * @param data  Input buffer.
 * @param len   Input length in bytes.
 * @return IEEE CRC32 of the input.
 */
uint32_t rv_feature_state_crc32(const uint8_t *data, size_t len);

/**
 * Finalize an rv_feature_state_t by populating magic, seq, ts_us, and crc32.
 * Caller fills the remaining fields in-place before calling this. After
 * finalize() the packet is ready to send on the wire.
 *
 * @param pkt        Packet to finalize (caller-owned).
 * @param node_id    Source node id (typically csi_collector_get_node_id()).
 * @param seq        Monotonic sequence (caller-managed).
 * @param ts_us      Node-local microseconds (typically esp_timer_get_time()).
 * @param mode       Active rv_capture_profile_t.
 */
void rv_feature_state_finalize(rv_feature_state_t *pkt,
                               uint8_t node_id,
                               uint16_t seq,
                               uint64_t ts_us,
                               uint8_t mode);

#ifdef __cplusplus
}
#endif

#endif /* RV_FEATURE_STATE_H */
