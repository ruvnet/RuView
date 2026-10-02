/*
 * Host unit tests for the CSI capture stall ladder (RuView#1941).
 * Plain C99, no ESP-IDF. Build: make test_csi_stall; run: ./test_csi_stall
 */
#include <stdio.h>
#include <stdlib.h>

#include "csi_stall_policy.h"

static int s_fail = 0;

#define CHECK(cond, msg) do { \
    if (!(cond)) { printf("FAIL %s:%d %s\n", __FILE__, __LINE__, msg); s_fail++; } \
} while (0)

#define T 15000u   /* per-step timeout, ms */

static void test_unarmed_never_fires(void)
{
    csi_stall_state_t s;
    csi_stall_init(&s, T, 1000);
    for (uint32_t t = 1000; t < 1000 + 10 * T; t += 1000) {
        CHECK(csi_stall_step(&s, t, 0, true) == CSI_STALL_NONE,
              "no activity ever seen must not arm the ladder");
    }
}

static void test_healthy_stream_never_fires(void)
{
    csi_stall_state_t s;
    csi_stall_init(&s, T, 0);
    for (uint32_t t = 1000; t < 600000; t += 1000) {
        CHECK(csi_stall_step(&s, t, t - 20, true) == CSI_STALL_NONE,
              "steady activity must stay healthy");
    }
}

static void test_full_ladder(void)
{
    csi_stall_state_t s;
    csi_stall_init(&s, T, 0);
    uint32_t last = 5000;
    CHECK(csi_stall_step(&s, 5000, last, true) == CSI_STALL_NONE, "arm");

    int rearm = 0, reassoc = 0, restart = 0;
    uint32_t t_rearm = 0, t_reassoc = 0, t_restart = 0;
    for (uint32_t t = 6000; t <= 5000 + 5 * T; t += 1000) {
        csi_stall_action_t a = csi_stall_step(&s, t, last, true);
        if (a == CSI_STALL_REARM)   { rearm++;   t_rearm = t; }
        if (a == CSI_STALL_REASSOC) { reassoc++; t_reassoc = t; }
        if (a == CSI_STALL_RESTART) { restart++; t_restart = t; }
    }
    CHECK(rearm == 1 && reassoc == 1 && restart == 1, "each stage fires exactly once");
    CHECK(t_rearm == 5000 + T, "rearm exactly one timeout after last activity");
    CHECK(t_reassoc == t_rearm + T, "reassoc one timeout after rearm");
    CHECK(t_restart == t_reassoc + T, "restart one timeout after reassoc");
}

static void test_rearm_recovers(void)
{
    csi_stall_state_t s;
    csi_stall_init(&s, T, 0);
    csi_stall_step(&s, 1000, 1000, true);
    CHECK(csi_stall_step(&s, 1000 + T, 1000, true) == CSI_STALL_REARM, "stall detected");
    /* Callbacks resume 300 ms after the re-arm. */
    uint32_t resumed = 1000 + T + 300;
    CHECK(csi_stall_step(&s, resumed + 700, resumed, true) == CSI_STALL_RECOVERED,
          "fresh activity after rearm reports recovery");
    CHECK(s.stage == 0, "recovery returns to healthy");
    CHECK(csi_stall_step(&s, resumed + 1700, resumed + 1680, true) == CSI_STALL_NONE,
          "healthy afterwards");
    /* A second, independent stall starts the ladder from stage 1 again. */
    uint32_t last = resumed + 1680;
    CHECK(csi_stall_step(&s, last + T, last, true) == CSI_STALL_REARM,
          "second stall restarts at stage 1");
}

static void test_link_down_pauses_after_reassoc(void)
{
    csi_stall_state_t s;
    csi_stall_init(&s, T, 0);
    csi_stall_step(&s, 1000, 1000, true);
    uint32_t t = 1000 + T;
    CHECK(csi_stall_step(&s, t, 1000, true) == CSI_STALL_REARM, "rearm");
    t += T;
    CHECK(csi_stall_step(&s, t, 1000, true) == CSI_STALL_REASSOC, "reassoc");
    /* The reassociation drops the link for 40 s: no escalation meanwhile. */
    for (uint32_t k = 0; k < 40; k++) {
        t += 1000;
        CHECK(csi_stall_step(&s, t, 1000, false) == CSI_STALL_NONE,
              "link down must pause the ladder");
    }
    /* Link back, capture still dead: restart only after a full window. */
    t += 1000;
    CHECK(csi_stall_step(&s, t, 1000, true) == CSI_STALL_NONE, "grace after link up");
    CHECK(csi_stall_step(&s, t + T - 2000, 1000, true) == CSI_STALL_NONE, "still in grace");
    CHECK(csi_stall_step(&s, t + T, 1000, true) == CSI_STALL_RESTART, "restart");
}

static void test_link_down_while_healthy_gives_grace(void)
{
    csi_stall_state_t s;
    csi_stall_init(&s, T, 0);
    csi_stall_step(&s, 1000, 1000, true);
    /* AP reboot: 5 minutes without a link. */
    uint32_t t;
    for (t = 2000; t < 300000; t += 1000) {
        CHECK(csi_stall_step(&s, t, 1000, false) == CSI_STALL_NONE, "paused");
    }
    CHECK(csi_stall_step(&s, t, 1000, true) == CSI_STALL_NONE,
          "link just returned: stale activity must not fire immediately");
    CHECK(csi_stall_step(&s, t + T, 1000, true) == CSI_STALL_REARM,
          "no capture a full window after link returns");
}

static void test_wraparound(void)
{
    csi_stall_state_t s;
    uint32_t base = 0xFFFFFFFFu - 5000u;
    csi_stall_init(&s, T, base);
    csi_stall_step(&s, base, base, true);
    CHECK(csi_stall_step(&s, base + 10000u, base, true) == CSI_STALL_NONE,
          "no false fire across wrap");
    CHECK(csi_stall_step(&s, base + T, base, true) == CSI_STALL_REARM,
          "timeout measured correctly across wrap");
    CHECK(csi_stall_step(&s, base + T + 500u, base + T + 200u, true) == CSI_STALL_RECOVERED,
          "recovery detected across wrap");
}

int main(void)
{
    test_unarmed_never_fires();
    test_healthy_stream_never_fires();
    test_full_ladder();
    test_rearm_recovers();
    test_link_down_pauses_after_reassoc();
    test_link_down_while_healthy_gives_grace();
    test_wraparound();
    if (s_fail) {
        printf("csi_stall_policy: %d FAILED\n", s_fail);
        return EXIT_FAILURE;
    }
    printf("csi_stall_policy: all tests passed\n");
    return EXIT_SUCCESS;
}
