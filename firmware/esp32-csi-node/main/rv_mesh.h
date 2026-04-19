/**
 * @file rv_mesh.h
 * @brief ADR-081 Layer 3 — Mesh Sensing Plane.
 *
 * Defines node roles, the 7 on-wire message types, and the
 * rv_node_status_t health payload that nodes exchange to behave as a
 * distributed sensor rather than a collection of independent radios.
 *
 * Framing: every mesh message starts with rv_mesh_header_t (magic,
 * version, type, sender_role, epoch, length) so a receiver can dispatch
 * without reading the whole body. The trailing 4 bytes of every message
 * are an IEEE CRC32 over the preceding bytes. Authentication
 * (HMAC-SHA256 + replay window) is layered on top by
 * wifi-densepose-hardware/src/esp32/secure_tdm.rs (ADR-032) for control
 * messages that cross the swarm; FEATURE_DELTA uses the integrity
 * protection already present in rv_feature_state_t (CRC + monotonic seq).
 */

#ifndef RV_MESH_H
#define RV_MESH_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include "esp_err.h"
#include "rv_feature_state.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Magic + version ---- */

/** ADR-081 mesh envelope magic. Distinct from the ADR-018 CSI magic. */
#define RV_MESH_MAGIC        0xC5118100u

/** Protocol version. Bumped on any wire-format change. */
#define RV_MESH_VERSION      1u

/** Maximum mesh payload size (excluding header + CRC). */
#define RV_MESH_MAX_PAYLOAD  256u

/* ---- Node roles (ADR-081 Layer 3) ---- */

typedef enum {
    RV_ROLE_UNASSIGNED   = 0,
    RV_ROLE_ANCHOR       = 1,  /**< Emits timed probes + global time beacons. */
    RV_ROLE_OBSERVER     = 2,  /**< Captures CSI + local metadata. */
    RV_ROLE_FUSION_RELAY = 3,  /**< Aggregates summaries, forwards deltas. */
    RV_ROLE_COORDINATOR  = 4,  /**< Elects channels, assigns roles. */
    RV_ROLE_COUNT
} rv_mesh_role_t;

/* ---- Authorization classes for control messages ---- */

typedef enum {
    RV_AUTH_NONE          = 0,  /**< Telemetry; integrity via CRC only. */
    RV_AUTH_HMAC_SESSION  = 1,  /**< HMAC-SHA256 with session key (ADR-032). */
    RV_AUTH_ED25519_BATCH = 2,  /**< Ed25519 signature at batch/session. */
} rv_mesh_auth_class_t;

/* ---- Message types ---- */

typedef enum {
    RV_MSG_TIME_SYNC         = 0x01,
    RV_MSG_ROLE_ASSIGN       = 0x02,
    RV_MSG_CHANNEL_PLAN      = 0x03,
    RV_MSG_CALIBRATION_START = 0x04,
    RV_MSG_FEATURE_DELTA     = 0x05,  /**< Carries rv_feature_state_t. */
    RV_MSG_HEALTH            = 0x06,
    RV_MSG_ANOMALY_ALERT     = 0x07,
} rv_mesh_msg_type_t;

/* ---- Common envelope header (16 bytes) ---- */

typedef struct __attribute__((packed)) {
    uint32_t magic;        /**< RV_MESH_MAGIC. */
    uint8_t  version;      /**< RV_MESH_VERSION. */
    uint8_t  type;         /**< rv_mesh_msg_type_t. */
    uint8_t  sender_role;  /**< rv_mesh_role_t of the sender at send time. */
    uint8_t  auth_class;   /**< rv_mesh_auth_class_t. */
    uint32_t epoch;        /**< Monotonic epoch or session counter. */
    uint16_t payload_len;  /**< Body length excluding header + trailing CRC. */
    uint16_t reserved;
} rv_mesh_header_t;

_Static_assert(sizeof(rv_mesh_header_t) == 16,
               "rv_mesh_header_t must be 16 bytes");

/* ---- Node health payload (RV_MSG_HEALTH) ---- */

typedef struct __attribute__((packed)) {
    uint8_t  node_id[8];      /**< 8-byte node identity. */
    uint64_t local_time_us;   /**< Sender-local microseconds. */
    uint8_t  role;            /**< rv_mesh_role_t. */
    uint8_t  current_channel;
    uint8_t  current_bw;      /**< MHz (20, 40). */
    int8_t   noise_floor_dbm;
    uint16_t pkt_yield;       /**< CSI callbacks/sec over the last window. */
    uint16_t sync_error_us;   /**< Absolute drift vs. anchor. */
    uint16_t health_flags;
    uint16_t reserved;
} rv_node_status_t;

_Static_assert(sizeof(rv_node_status_t) == 28,
               "rv_node_status_t must be 28 bytes");

/* ---- TIME_SYNC payload ---- */

typedef struct __attribute__((packed)) {
    uint64_t anchor_time_us;  /**< Anchor's local µs at emit. */
    uint32_t cycle_id;
    uint32_t cycle_period_us;
} rv_time_sync_t;

_Static_assert(sizeof(rv_time_sync_t) == 16,
               "rv_time_sync_t must be 16 bytes");

/* ---- ROLE_ASSIGN payload ---- */

typedef struct __attribute__((packed)) {
    uint8_t  target_node_id[8];
    uint8_t  new_role;     /**< rv_mesh_role_t. */
    uint8_t  reserved[3];
    uint32_t effective_epoch;
} rv_role_assign_t;

_Static_assert(sizeof(rv_role_assign_t) == 16,
               "rv_role_assign_t must be 16 bytes");

/* ---- CHANNEL_PLAN payload ---- */

#define RV_CHANNEL_PLAN_MAX 8

typedef struct __attribute__((packed)) {
    uint8_t  target_node_id[8];
    uint8_t  channel_count;
    uint8_t  dwell_ms_hi;     /**< dwell_ms, big-endian to fit u16 in two bytes */
    uint8_t  dwell_ms_lo;
    uint8_t  debug_raw_csi;   /**< 1 = enable raw ADR-018 stream; 0 = feature_state only. */
    uint8_t  channels[RV_CHANNEL_PLAN_MAX];
    uint32_t effective_epoch;
} rv_channel_plan_t;

_Static_assert(sizeof(rv_channel_plan_t) == 24,
               "rv_channel_plan_t must be 24 bytes");

/* ---- CALIBRATION_START payload ---- */

typedef struct __attribute__((packed)) {
    uint64_t t0_anchor_us;    /**< Start time on anchor clock. */
    uint32_t duration_ms;
    uint32_t effective_epoch;
    uint8_t  calibration_profile;  /**< rv_capture_profile_t (usually CALIBRATION). */
    uint8_t  reserved[3];
} rv_calibration_start_t;

_Static_assert(sizeof(rv_calibration_start_t) == 20,
               "rv_calibration_start_t must be 20 bytes");

/* ---- ANOMALY_ALERT payload ---- */

typedef struct __attribute__((packed)) {
    uint8_t  node_id[8];
    uint64_t ts_us;
    uint8_t  severity;        /**< 0..255 scaled anomaly. */
    uint8_t  reason;          /**< rv_anomaly_reason_t. */
    uint16_t reserved;
    float    anomaly_score;
    float    motion_score;
} rv_anomaly_alert_t;

_Static_assert(sizeof(rv_anomaly_alert_t) == 28,
               "rv_anomaly_alert_t must be 28 bytes");

typedef enum {
    RV_ANOMALY_NONE              = 0,
    RV_ANOMALY_PHYSICS_VIOLATION = 1,
    RV_ANOMALY_MULTI_LINK_MISMATCH = 2,
    RV_ANOMALY_PKT_YIELD_COLLAPSE = 3,
    RV_ANOMALY_FALL               = 4,
    RV_ANOMALY_COHERENCE_LOSS     = 5,
} rv_anomaly_reason_t;

/* ---- Encoder / decoder API ---- */

/** Maximum on-wire mesh frame: header + max payload + crc. */
#define RV_MESH_MAX_FRAME_BYTES  (sizeof(rv_mesh_header_t) + RV_MESH_MAX_PAYLOAD + 4u)

/**
 * Encode a typed mesh message into a contiguous buffer.
 *
 * Writes header(16) + payload(payload_len) + crc32(4). The caller owns
 * the buffer; buf_cap must be at least sizeof(rv_mesh_header_t) +
 * payload_len + 4. The payload pointer may be NULL iff payload_len == 0.
 *
 * @return bytes written on success, or 0 on error (bad args / overflow).
 */
size_t rv_mesh_encode(uint8_t type,
                      uint8_t sender_role,
                      uint8_t auth_class,
                      uint32_t epoch,
                      const void *payload,
                      uint16_t payload_len,
                      uint8_t *buf,
                      size_t buf_cap);

/**
 * Validate + parse a mesh frame received from the wire.
 *
 * Checks magic, version, sizeof(rv_mesh_header_t) bounds, payload_len
 * bounds, and CRC32. On success, fills *out_hdr with the header and sets
 * *out_payload to point at the payload inside buf (aliasing, not copied)
 * plus *out_payload_len to the payload byte count.
 *
 * @return ESP_OK on success, or an ESP_ERR_* code on failure.
 */
esp_err_t rv_mesh_decode(const uint8_t *buf, size_t buf_len,
                         rv_mesh_header_t *out_hdr,
                         const uint8_t **out_payload,
                         uint16_t *out_payload_len);

/**
 * Convenience helpers — encode a specific message type into buf.
 * Each returns the number of bytes written, 0 on error.
 */
size_t rv_mesh_encode_health(uint8_t sender_role,
                             uint32_t epoch,
                             const rv_node_status_t *status,
                             uint8_t *buf, size_t buf_cap);

size_t rv_mesh_encode_anomaly_alert(uint8_t sender_role,
                                    uint32_t epoch,
                                    const rv_anomaly_alert_t *alert,
                                    uint8_t *buf, size_t buf_cap);

size_t rv_mesh_encode_feature_delta(uint8_t sender_role,
                                    uint32_t epoch,
                                    const rv_feature_state_t *fs,
                                    uint8_t *buf, size_t buf_cap);

size_t rv_mesh_encode_time_sync(uint8_t sender_role,
                                uint32_t epoch,
                                const rv_time_sync_t *ts,
                                uint8_t *buf, size_t buf_cap);

size_t rv_mesh_encode_role_assign(uint8_t sender_role,
                                  uint32_t epoch,
                                  const rv_role_assign_t *ra,
                                  uint8_t *buf, size_t buf_cap);

size_t rv_mesh_encode_channel_plan(uint8_t sender_role,
                                   uint32_t epoch,
                                   const rv_channel_plan_t *cp,
                                   uint8_t *buf, size_t buf_cap);

size_t rv_mesh_encode_calibration_start(uint8_t sender_role,
                                        uint32_t epoch,
                                        const rv_calibration_start_t *cs,
                                        uint8_t *buf, size_t buf_cap);

/* ---- Send API ---- */

/**
 * Send a pre-encoded mesh frame over the primary upstream UDP socket
 * (the same one stream_sender uses for ADR-018 and rv_feature_state_t).
 *
 * @return ESP_OK on success.
 */
esp_err_t rv_mesh_send(const uint8_t *frame, size_t len);

/**
 * Convenience: build + send a HEALTH message for this node.
 *
 * Fills the rv_node_status_t from the live radio ops + controller
 * observation, then encodes and sends in one call. Safe to call from a
 * FreeRTOS timer.
 */
esp_err_t rv_mesh_send_health(uint8_t role, uint32_t epoch,
                              const uint8_t node_id[8]);

/**
 * Convenience: build + send an ANOMALY_ALERT.
 */
esp_err_t rv_mesh_send_anomaly(uint8_t role, uint32_t epoch,
                               const uint8_t node_id[8],
                               uint8_t reason,
                               uint8_t severity,
                               float anomaly_score,
                               float motion_score);

#ifdef __cplusplus
}
#endif

#endif /* RV_MESH_H */
