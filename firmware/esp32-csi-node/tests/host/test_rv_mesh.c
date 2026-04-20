/*
 * Host unit test for ADR-081 Layer 3 mesh plane encode/decode.
 *
 * rv_mesh_encode() and rv_mesh_decode() are the pure halves of the
 * mesh plane — no ESP-IDF, no sockets — so we exercise them with the
 * RV_MESH_HOST_TEST flag that disables the send helpers.
 */

#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#include "rv_mesh.h"
#include "rv_feature_state.h"
#include "rv_radio_ops.h"  /* for RV_PROFILE_* enum values */

static int g_pass = 0, g_fail = 0;
#define CHECK(cond, msg) do {                                   \
    if (cond) { g_pass++; }                                     \
    else { g_fail++; printf("  FAIL: %s (line %d)\n", msg, __LINE__); } \
} while (0)

static void test_header_size(void) {
    printf("test: rv_mesh_header_t is 16 bytes\n");
    CHECK(sizeof(rv_mesh_header_t) == 16, "sizeof(header) == 16");
}

static void test_encode_health_roundtrip(void) {
    printf("test: HEALTH roundtrip\n");
    rv_node_status_t st;
    memset(&st, 0, sizeof(st));
    st.node_id[0]       = 7;
    st.local_time_us    = 1234567890ULL;
    st.role             = RV_ROLE_OBSERVER;
    st.current_channel  = 6;
    st.current_bw       = 20;
    st.noise_floor_dbm  = -93;
    st.pkt_yield        = 42;
    st.sync_error_us    = 12;

    uint8_t buf[RV_MESH_MAX_FRAME_BYTES];
    size_t n = rv_mesh_encode_health(RV_ROLE_OBSERVER, /*epoch*/ 100,
                                     &st, buf, sizeof(buf));
    CHECK(n > 0, "encode returns non-zero");
    CHECK(n == sizeof(rv_mesh_header_t) + sizeof(st) + 4,
          "encoded size = hdr+payload+crc");

    rv_mesh_header_t hdr;
    const uint8_t *payload = NULL;
    uint16_t payload_len = 0;
    esp_err_t rc = rv_mesh_decode(buf, n, &hdr, &payload, &payload_len);
    CHECK(rc == ESP_OK, "decode OK");
    CHECK(hdr.type == RV_MSG_HEALTH, "type == HEALTH");
    CHECK(hdr.epoch == 100, "epoch survives");
    CHECK(hdr.payload_len == sizeof(st), "payload_len matches");
    CHECK(payload != NULL, "payload pointer set");
    CHECK(memcmp(payload, &st, sizeof(st)) == 0, "payload bytes match");
}

static void test_encode_anomaly_roundtrip(void) {
    printf("test: ANOMALY_ALERT roundtrip\n");
    rv_anomaly_alert_t a;
    memset(&a, 0, sizeof(a));
    a.node_id[0]    = 3;
    a.ts_us         = 999999ULL;
    a.reason        = RV_ANOMALY_FALL;
    a.severity      = 200;
    a.anomaly_score = 0.85f;
    a.motion_score  = 0.9f;

    uint8_t buf[RV_MESH_MAX_FRAME_BYTES];
    size_t n = rv_mesh_encode_anomaly_alert(RV_ROLE_OBSERVER, 7, &a,
                                            buf, sizeof(buf));
    CHECK(n > 0, "encoded");

    rv_mesh_header_t hdr;
    const uint8_t *payload = NULL;
    uint16_t payload_len = 0;
    esp_err_t rc = rv_mesh_decode(buf, n, &hdr, &payload, &payload_len);
    CHECK(rc == ESP_OK, "decoded");
    CHECK(hdr.type == RV_MSG_ANOMALY_ALERT, "type ok");
    rv_anomaly_alert_t got;
    memcpy(&got, payload, sizeof(got));
    CHECK(got.reason == RV_ANOMALY_FALL, "reason survived");
    CHECK(got.severity == 200, "severity survived");
}

static void test_encode_feature_delta_wraps_feature_state(void) {
    printf("test: FEATURE_DELTA wraps rv_feature_state_t\n");
    rv_feature_state_t fs;
    memset(&fs, 0, sizeof(fs));
    fs.motion_score = 0.5f;
    rv_feature_state_finalize(&fs, /*node*/ 9, /*seq*/ 17,
                              /*ts*/ 111ULL, RV_PROFILE_FAST_MOTION);

    uint8_t buf[RV_MESH_MAX_FRAME_BYTES];
    size_t n = rv_mesh_encode_feature_delta(RV_ROLE_OBSERVER, 2, &fs,
                                            buf, sizeof(buf));
    CHECK(n == sizeof(rv_mesh_header_t) + sizeof(fs) + 4, "size check");

    rv_mesh_header_t hdr;
    const uint8_t *payload = NULL;
    uint16_t len = 0;
    CHECK(rv_mesh_decode(buf, n, &hdr, &payload, &len) == ESP_OK,
          "decode OK");
    rv_feature_state_t got;
    memcpy(&got, payload, sizeof(got));
    CHECK(got.magic == RV_FEATURE_STATE_MAGIC, "inner magic preserved");
    CHECK(got.node_id == 9, "inner node_id preserved");
    CHECK(got.seq == 17, "inner seq preserved");
    /* Inner CRC is end-to-end even though the mesh frame has its own
     * CRC too — two checks for two failure modes. */
    uint32_t inner_crc = rv_feature_state_crc32(
        (const uint8_t *)&got, sizeof(got) - sizeof(uint32_t));
    CHECK(inner_crc == got.crc32, "inner feature_state CRC still valid");
}

static void test_decode_rejects_bad_magic(void) {
    printf("test: decode rejects bad magic\n");
    uint8_t buf[sizeof(rv_mesh_header_t) + 4];
    memset(buf, 0xFF, sizeof(buf));

    rv_mesh_header_t hdr;
    const uint8_t *p = NULL;
    uint16_t plen = 0;
    esp_err_t rc = rv_mesh_decode(buf, sizeof(buf), &hdr, &p, &plen);
    CHECK(rc != ESP_OK, "bad magic rejected");
}

static void test_decode_rejects_truncated(void) {
    printf("test: decode rejects truncated frame\n");
    uint8_t buf[sizeof(rv_mesh_header_t) - 1];
    memset(buf, 0, sizeof(buf));
    rv_mesh_header_t hdr;
    const uint8_t *p = NULL;
    uint16_t plen = 0;
    esp_err_t rc = rv_mesh_decode(buf, sizeof(buf), &hdr, &p, &plen);
    CHECK(rc != ESP_OK, "truncated rejected");
}

static void test_decode_rejects_bad_crc(void) {
    printf("test: decode rejects CRC mismatch\n");
    rv_node_status_t st;
    memset(&st, 0, sizeof(st));
    st.role = RV_ROLE_OBSERVER;
    uint8_t buf[RV_MESH_MAX_FRAME_BYTES];
    size_t n = rv_mesh_encode_health(RV_ROLE_OBSERVER, 1, &st,
                                     buf, sizeof(buf));
    CHECK(n > 0, "encoded");

    /* Flip a byte in the payload — CRC must now mismatch. */
    buf[sizeof(rv_mesh_header_t) + 4] ^= 0x10;

    rv_mesh_header_t hdr;
    const uint8_t *p = NULL;
    uint16_t plen = 0;
    esp_err_t rc = rv_mesh_decode(buf, n, &hdr, &p, &plen);
    CHECK(rc != ESP_OK, "CRC mismatch rejected");
}

static void test_encode_rejects_oversize_payload(void) {
    printf("test: encode rejects oversize payload\n");
    uint8_t junk[RV_MESH_MAX_PAYLOAD + 1] = {0};
    uint8_t buf[RV_MESH_MAX_FRAME_BYTES + 8];
    size_t n = rv_mesh_encode(RV_MSG_HEALTH, RV_ROLE_OBSERVER, RV_AUTH_NONE,
                              0, junk, sizeof(junk), buf, sizeof(buf));
    CHECK(n == 0, "oversize payload → 0");
}

static void test_encode_rejects_small_buf(void) {
    printf("test: encode rejects too-small buffer\n");
    rv_node_status_t st = {0};
    uint8_t buf[16];  /* header fits but not payload */
    size_t n = rv_mesh_encode_health(RV_ROLE_OBSERVER, 0, &st,
                                     buf, sizeof(buf));
    CHECK(n == 0, "small buf → 0");
}

static void benchmark_encode(void) {
    printf("bench: encode+decode HEALTH roundtrip\n");
    rv_node_status_t st;
    memset(&st, 0x33, sizeof(st));
    uint8_t buf[RV_MESH_MAX_FRAME_BYTES];

    const int N = 2000000;
    struct timespec a, b;
    clock_gettime(CLOCK_MONOTONIC, &a);
    for (int i = 0; i < N; i++) {
        st.pkt_yield = (uint16_t)i;
        size_t n = rv_mesh_encode_health(RV_ROLE_OBSERVER, (uint32_t)i,
                                         &st, buf, sizeof(buf));
        rv_mesh_header_t hdr;
        const uint8_t *p = NULL;
        uint16_t plen = 0;
        (void)rv_mesh_decode(buf, n, &hdr, &p, &plen);
    }
    clock_gettime(CLOCK_MONOTONIC, &b);
    double ns = ((b.tv_sec - a.tv_sec) * 1e9 +
                 (b.tv_nsec - a.tv_nsec)) / (double)N;
    printf("  %d roundtrips, %.1f ns/call\n", N, ns);
    CHECK(ns < 20000.0, "encode+decode must be under 20us/roundtrip");
}

int main(void) {
    printf("=== rv_mesh encode/decode host tests ===\n\n");
    test_header_size();
    test_encode_health_roundtrip();
    test_encode_anomaly_roundtrip();
    test_encode_feature_delta_wraps_feature_state();
    test_decode_rejects_bad_magic();
    test_decode_rejects_truncated();
    test_decode_rejects_bad_crc();
    test_encode_rejects_oversize_payload();
    test_encode_rejects_small_buf();
    benchmark_encode();
    printf("\n=== result: %d pass, %d fail ===\n", g_pass, g_fail);
    return g_fail > 0 ? 1 : 0;
}
