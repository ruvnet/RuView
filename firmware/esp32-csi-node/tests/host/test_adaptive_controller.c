/*
 * Host unit test for adaptive_controller_decide().
 *
 * The ADR-081 controller decision function is deliberately pure: it takes
 * (cfg, current_state, observation) and produces a decision. No FreeRTOS,
 * no ESP-IDF, no side effects. This test exercises every documented branch
 * of the policy.
 *
 * Build + run (from this directory):
 *   make -f Makefile
 *   ./test_adaptive_controller
 */

#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#include "adaptive_controller.h"
#include "rv_radio_ops.h"

static int g_pass = 0, g_fail = 0;

#define CHECK(cond, msg) do {                                   \
    if (cond) { g_pass++; }                                     \
    else { g_fail++; printf("  FAIL: %s (line %d)\n", msg, __LINE__); } \
} while (0)

static adapt_config_t default_cfg(void) {
    adapt_config_t c = {
        .fast_loop_ms = 200,
        .medium_loop_ms = 1000,
        .slow_loop_ms = 30000,
        .aggressive = false,
        .enable_channel_switch = false,
        .enable_role_change = false,
        .motion_threshold = 0.20f,
        .anomaly_threshold = 0.60f,
        .min_pkt_yield = 5,
    };
    return c;
}

static adapt_observation_t quiet_obs(void) {
    adapt_observation_t o = {
        .pkt_yield_per_sec = 50,
        .send_fail_count = 0,
        .rssi_median_dbm = -60,
        .noise_floor_dbm = -95,
        .motion_score = 0.01f,
        .presence_score = 0.0f,
        .anomaly_score = 0.0f,
        .node_coherence = 1.0f,
    };
    return o;
}

static void test_degraded_gate_on_pkt_yield_collapse(void) {
    printf("test: degraded gate on pkt yield collapse\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();
    obs.pkt_yield_per_sec = 2;  /* below min_pkt_yield=5 */

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);

    CHECK(dec.change_state, "should change state");
    CHECK(dec.new_state == ADAPT_STATE_DEGRADED, "new state == DEGRADED");
    CHECK(dec.new_profile == RV_PROFILE_PASSIVE_LOW_RATE,
          "profile pinned to PASSIVE_LOW_RATE in degraded");
    CHECK(dec.suggested_vital_interval_ms == 2000,
          "cadence relaxed to 2s in degraded");
}

static void test_degraded_gate_on_coherence_loss(void) {
    printf("test: degraded gate on coherence loss\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();
    obs.node_coherence = 0.15f;  /* below 0.20 threshold */

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);
    CHECK(dec.new_state == ADAPT_STATE_DEGRADED, "coherence loss → DEGRADED");
}

static void test_anomaly_trumps_motion(void) {
    printf("test: anomaly trumps motion\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();
    obs.motion_score = 0.9f;  /* high motion */
    obs.anomaly_score = 0.8f; /* but anomaly is above threshold */

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);

    CHECK(dec.new_state == ADAPT_STATE_ALERT, "anomaly → ALERT");
    CHECK(dec.new_profile == RV_PROFILE_FAST_MOTION,
          "alert uses FAST_MOTION profile");
    CHECK(dec.suggested_vital_interval_ms == 100, "alert cadence 100ms");
}

static void test_motion_triggers_sense_active(void) {
    printf("test: motion → SENSE_ACTIVE\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();
    obs.motion_score = 0.50f;

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);

    CHECK(dec.new_state == ADAPT_STATE_SENSE_ACTIVE, "motion → SENSE_ACTIVE");
    CHECK(dec.new_profile == RV_PROFILE_FAST_MOTION, "profile FAST_MOTION");
    CHECK(dec.suggested_vital_interval_ms == 200,
          "non-aggressive cadence 200ms");
}

static void test_aggressive_cadence(void) {
    printf("test: aggressive cadence is tighter\n");
    adapt_config_t cfg = default_cfg();
    cfg.aggressive = true;
    adapt_observation_t obs = quiet_obs();
    obs.motion_score = 0.50f;

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);
    CHECK(dec.suggested_vital_interval_ms == 100,
          "aggressive motion cadence 100ms");
}

static void test_stable_presence_uses_resp_high_sens(void) {
    printf("test: stable presence → RESP_HIGH_SENS\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();
    obs.presence_score = 0.8f;
    obs.motion_score = 0.01f;

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);
    CHECK(dec.new_profile == RV_PROFILE_RESP_HIGH_SENS,
          "stable presence uses respiration profile");
    CHECK(dec.suggested_vital_interval_ms == 1000,
          "respiration cadence 1s");
}

static void test_empty_room_default_is_passive(void) {
    printf("test: empty room → PASSIVE_LOW_RATE\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);
    CHECK(dec.new_profile == RV_PROFILE_PASSIVE_LOW_RATE,
          "empty → passive low rate");
}

static void test_hysteresis_no_flap(void) {
    printf("test: no change_state when already in target state\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();
    obs.motion_score = 0.50f;

    adapt_decision_t dec;
    adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_ACTIVE, &obs, &dec);
    CHECK(!dec.change_state,
          "already in SENSE_ACTIVE — no redundant change_state");
}

static void test_null_safety(void) {
    printf("test: NULL args are no-ops (no crash)\n");
    adapt_decision_t dec = {0};
    adaptive_controller_decide(NULL, ADAPT_STATE_SENSE_IDLE, NULL, &dec);
    /* if we got here, no segfault — pass */
    g_pass++;
    printf("  OK\n");
}

static void benchmark_decide(void) {
    printf("bench: adaptive_controller_decide() throughput\n");
    adapt_config_t cfg = default_cfg();
    adapt_observation_t obs = quiet_obs();
    adapt_decision_t dec;

    const int N = 10000000;
    struct timespec a, b;
    clock_gettime(CLOCK_MONOTONIC, &a);
    for (int i = 0; i < N; i++) {
        /* Vary input slightly so the compiler can't fold the call. */
        obs.motion_score = (i & 0xff) / 255.0f;
        adaptive_controller_decide(&cfg, ADAPT_STATE_SENSE_IDLE, &obs, &dec);
    }
    clock_gettime(CLOCK_MONOTONIC, &b);
    double ns_per_call = ((b.tv_sec - a.tv_sec) * 1e9 +
                          (b.tv_nsec - a.tv_nsec)) / (double)N;
    printf("  %d calls, %.1f ns/call\n", N, ns_per_call);
    /* Sanity: decide() is O(constant) — must be under 10us even on a
     * slow emulator. Real ESP32 will be ~100-300ns. */
    CHECK(ns_per_call < 10000.0, "decide() must be under 10us/call");
}

int main(void) {
    printf("=== adaptive_controller_decide() host tests ===\n\n");

    test_degraded_gate_on_pkt_yield_collapse();
    test_degraded_gate_on_coherence_loss();
    test_anomaly_trumps_motion();
    test_motion_triggers_sense_active();
    test_aggressive_cadence();
    test_stable_presence_uses_resp_high_sens();
    test_empty_room_default_is_passive();
    test_hysteresis_no_flap();
    test_null_safety();
    benchmark_decide();

    printf("\n=== result: %d pass, %d fail ===\n", g_pass, g_fail);
    return g_fail > 0 ? 1 : 0;
}
