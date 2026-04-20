/**
 * @file rv_mesh.c
 * @brief ADR-081 Layer 3 — Mesh Sensing Plane implementation.
 *
 * Encoder/decoder are pure functions (no ESP-IDF deps) and therefore
 * host-unit-testable. The send helpers wrap stream_sender so the
 * firmware can use a single upstream socket for all payload types.
 */

#include "rv_mesh.h"
#include "rv_feature_state.h"
#include "rv_radio_ops.h"

#include <string.h>

#ifndef RV_MESH_HOST_TEST
#include "esp_log.h"
#include "esp_timer.h"
#include "stream_sender.h"
#include "csi_collector.h"
#include "adaptive_controller.h"
static const char *TAG = "rv_mesh";
#endif

/* ---- Encoder ---- */

size_t rv_mesh_encode(uint8_t type,
                      uint8_t sender_role,
                      uint8_t auth_class,
                      uint32_t epoch,
                      const void *payload,
                      uint16_t payload_len,
                      uint8_t *buf,
                      size_t buf_cap)
{
    if (buf == NULL) return 0;
    if (payload == NULL && payload_len != 0) return 0;
    if (payload_len > RV_MESH_MAX_PAYLOAD) return 0;

    size_t total = sizeof(rv_mesh_header_t) + (size_t)payload_len + 4u;
    if (buf_cap < total) return 0;

    rv_mesh_header_t hdr;
    hdr.magic       = RV_MESH_MAGIC;
    hdr.version     = (uint8_t)RV_MESH_VERSION;
    hdr.type        = type;
    hdr.sender_role = sender_role;
    hdr.auth_class  = auth_class;
    hdr.epoch       = epoch;
    hdr.payload_len = payload_len;
    hdr.reserved    = 0;

    memcpy(buf, &hdr, sizeof(hdr));
    if (payload_len > 0) {
        memcpy(buf + sizeof(hdr), payload, payload_len);
    }

    /* IEEE CRC32 over header + payload. Reuses the CRC32 from
     * rv_feature_state.c so there is exactly one implementation. */
    uint32_t crc = rv_feature_state_crc32(buf, sizeof(hdr) + payload_len);
    memcpy(buf + sizeof(hdr) + payload_len, &crc, 4);

    return total;
}

esp_err_t rv_mesh_decode(const uint8_t *buf, size_t buf_len,
                         rv_mesh_header_t *out_hdr,
                         const uint8_t **out_payload,
                         uint16_t *out_payload_len)
{
    if (buf == NULL || out_hdr == NULL ||
        out_payload == NULL || out_payload_len == NULL) {
        return ESP_ERR_INVALID_ARG;
    }
    if (buf_len < sizeof(rv_mesh_header_t) + 4u) {
        return ESP_ERR_INVALID_SIZE;
    }

    rv_mesh_header_t hdr;
    memcpy(&hdr, buf, sizeof(hdr));

    if (hdr.magic != RV_MESH_MAGIC) {
        return ESP_ERR_INVALID_VERSION;  /* repurpose: wrong magic */
    }
    if (hdr.version != RV_MESH_VERSION) {
        return ESP_ERR_INVALID_VERSION;
    }
    if (hdr.payload_len > RV_MESH_MAX_PAYLOAD) {
        return ESP_ERR_INVALID_SIZE;
    }

    size_t needed = sizeof(hdr) + (size_t)hdr.payload_len + 4u;
    if (buf_len < needed) {
        return ESP_ERR_INVALID_SIZE;
    }

    uint32_t got_crc;
    memcpy(&got_crc, buf + sizeof(hdr) + hdr.payload_len, 4);
    uint32_t want_crc = rv_feature_state_crc32(buf,
                          sizeof(hdr) + hdr.payload_len);
    if (got_crc != want_crc) {
        return ESP_ERR_INVALID_CRC;
    }

    *out_hdr         = hdr;
    *out_payload     = (hdr.payload_len > 0) ? buf + sizeof(hdr) : NULL;
    *out_payload_len = hdr.payload_len;
    return ESP_OK;
}

/* ---- Typed convenience encoders ---- */

size_t rv_mesh_encode_health(uint8_t sender_role,
                             uint32_t epoch,
                             const rv_node_status_t *status,
                             uint8_t *buf, size_t buf_cap)
{
    if (status == NULL) return 0;
    return rv_mesh_encode(RV_MSG_HEALTH, sender_role, RV_AUTH_NONE,
                          epoch, status, sizeof(*status), buf, buf_cap);
}

size_t rv_mesh_encode_anomaly_alert(uint8_t sender_role,
                                    uint32_t epoch,
                                    const rv_anomaly_alert_t *alert,
                                    uint8_t *buf, size_t buf_cap)
{
    if (alert == NULL) return 0;
    return rv_mesh_encode(RV_MSG_ANOMALY_ALERT, sender_role, RV_AUTH_NONE,
                          epoch, alert, sizeof(*alert), buf, buf_cap);
}

size_t rv_mesh_encode_feature_delta(uint8_t sender_role,
                                    uint32_t epoch,
                                    const rv_feature_state_t *fs,
                                    uint8_t *buf, size_t buf_cap)
{
    if (fs == NULL) return 0;
    return rv_mesh_encode(RV_MSG_FEATURE_DELTA, sender_role, RV_AUTH_NONE,
                          epoch, fs, sizeof(*fs), buf, buf_cap);
}

size_t rv_mesh_encode_time_sync(uint8_t sender_role,
                                uint32_t epoch,
                                const rv_time_sync_t *ts,
                                uint8_t *buf, size_t buf_cap)
{
    if (ts == NULL) return 0;
    return rv_mesh_encode(RV_MSG_TIME_SYNC, sender_role, RV_AUTH_HMAC_SESSION,
                          epoch, ts, sizeof(*ts), buf, buf_cap);
}

size_t rv_mesh_encode_role_assign(uint8_t sender_role,
                                  uint32_t epoch,
                                  const rv_role_assign_t *ra,
                                  uint8_t *buf, size_t buf_cap)
{
    if (ra == NULL) return 0;
    return rv_mesh_encode(RV_MSG_ROLE_ASSIGN, sender_role, RV_AUTH_HMAC_SESSION,
                          epoch, ra, sizeof(*ra), buf, buf_cap);
}

size_t rv_mesh_encode_channel_plan(uint8_t sender_role,
                                   uint32_t epoch,
                                   const rv_channel_plan_t *cp,
                                   uint8_t *buf, size_t buf_cap)
{
    if (cp == NULL) return 0;
    return rv_mesh_encode(RV_MSG_CHANNEL_PLAN, sender_role, RV_AUTH_ED25519_BATCH,
                          epoch, cp, sizeof(*cp), buf, buf_cap);
}

size_t rv_mesh_encode_calibration_start(uint8_t sender_role,
                                        uint32_t epoch,
                                        const rv_calibration_start_t *cs,
                                        uint8_t *buf, size_t buf_cap)
{
    if (cs == NULL) return 0;
    return rv_mesh_encode(RV_MSG_CALIBRATION_START, sender_role,
                          RV_AUTH_ED25519_BATCH, epoch, cs, sizeof(*cs),
                          buf, buf_cap);
}

/* ---- Send helpers (firmware-only; hidden from host tests) ---- */

#ifndef RV_MESH_HOST_TEST

esp_err_t rv_mesh_send(const uint8_t *frame, size_t len)
{
    if (frame == NULL || len == 0) return ESP_ERR_INVALID_ARG;
    int sent = stream_sender_send(frame, len);
    if (sent < 0) {
        ESP_LOGW(TAG, "rv_mesh_send: stream_sender failed (len=%u)",
                 (unsigned)len);
        return ESP_FAIL;
    }
    return ESP_OK;
}

esp_err_t rv_mesh_send_health(uint8_t role, uint32_t epoch,
                              const uint8_t node_id[8])
{
    if (node_id == NULL) return ESP_ERR_INVALID_ARG;

    rv_node_status_t st;
    memset(&st, 0, sizeof(st));
    memcpy(st.node_id, node_id, 8);
    st.local_time_us = (uint64_t)esp_timer_get_time();
    st.role          = role;

    const rv_radio_ops_t *ops = rv_radio_ops_get();
    if (ops != NULL && ops->get_health != NULL) {
        rv_radio_health_t h;
        if (ops->get_health(&h) == ESP_OK) {
            st.current_channel = h.current_channel;
            st.current_bw      = h.current_bw_mhz;
            st.noise_floor_dbm = h.noise_floor_dbm;
            st.pkt_yield       = h.pkt_yield_per_sec;
        }
    }

    uint8_t buf[RV_MESH_MAX_FRAME_BYTES];
    size_t n = rv_mesh_encode_health(role, epoch, &st, buf, sizeof(buf));
    if (n == 0) return ESP_FAIL;
    return rv_mesh_send(buf, n);
}

esp_err_t rv_mesh_send_anomaly(uint8_t role, uint32_t epoch,
                               const uint8_t node_id[8],
                               uint8_t reason,
                               uint8_t severity,
                               float anomaly_score,
                               float motion_score)
{
    if (node_id == NULL) return ESP_ERR_INVALID_ARG;
    rv_anomaly_alert_t a;
    memset(&a, 0, sizeof(a));
    memcpy(a.node_id, node_id, 8);
    a.ts_us         = (uint64_t)esp_timer_get_time();
    a.reason        = reason;
    a.severity      = severity;
    a.anomaly_score = anomaly_score;
    a.motion_score  = motion_score;

    uint8_t buf[RV_MESH_MAX_FRAME_BYTES];
    size_t n = rv_mesh_encode_anomaly_alert(role, epoch, &a, buf, sizeof(buf));
    if (n == 0) return ESP_FAIL;
    return rv_mesh_send(buf, n);
}

#endif /* !RV_MESH_HOST_TEST */
