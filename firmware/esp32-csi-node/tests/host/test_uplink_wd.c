/*
 * Host unit tests for uplink_wd_core.c (uplink watchdog decision).
 *
 * Replays tick sequences shaped on measured fleet behaviour: a TX stall in
 * which every CSI send failed for ~10 h while the node stayed associated, the
 * self-healing silences that set the 1200 s timeout (median 210 s, max 225 s),
 * ordinary healthy / ENOMEM-pressure running, and (rule 3) a dead IP path
 * while associated. The traces are SYNTHETIC
 * reconstructions of those shapes, not recorded counters.
 *
 * Pure C, no ESP-IDF -- same pattern as the rest of tests/host/.
 *
 * Build + run (from this directory):
 *   make test_uplink_wd
 *   ./test_uplink_wd
 */
#include <stdio.h>
#include <string.h>

#include "uplink_wd_core.h"

static int g_pass = 0, g_fail = 0;

#define CHECK(cond, msg) do {                                           \
    if (cond) { g_pass++; }                                             \
    else { g_fail++; printf("  FAIL: %s (line %d)\n", msg, __LINE__); } \
} while (0)

#define TIMEOUT_S 1200
#define TICK_US   (10LL * 1000000LL)

/* A simulated node: main.c's 10 s loop around uwd_step(). */
typedef struct {
    uwd_state_t st;
    int64_t  now_us;
    uint32_t ok, fail;
    int64_t  last_ok_us;
    uint8_t  fired_rule;
    int64_t  fired_at_us;
    int64_t  stalled_s;
} sim_t;

static void sim_init(sim_t *s, uint32_t ok0, uint32_t fail0)
{
    memset(s, 0, sizeof(*s));
    s->now_us = 30LL * 1000000LL;   /* some uptime already */
    s->ok = ok0;
    s->fail = fail0;
}

/* One 10 s tick in which `ok` sends were accepted and `fail` failed. Returns
 * the rule; latches the first firing. */
static uint8_t sim_tick(sim_t *s, uint32_t ok, uint32_t fail)
{
    s->now_us += TICK_US;
    s->ok += ok;
    s->fail += fail;
    if (ok > 0) s->last_ok_us = s->now_us - 1000;   /* within this tick */
    int64_t st = 0;
    uint8_t r = uwd_step(&s->st, s->now_us, s->ok, s->fail, s->last_ok_us,
                         TIMEOUT_S, &st);
    if (r != UWD_RULE_NONE && s->fired_rule == UWD_RULE_NONE) {
        s->fired_rule = r;
        s->fired_at_us = s->now_us;
        s->stalled_s = st;
    }
    return r;
}

static void run_ticks(sim_t *s, int n, uint32_t ok, uint32_t fail)
{
    for (int i = 0; i < n; i++) sim_tick(s, ok, fail);
}

/* 1. The measured TX stall, worst case for rule 1: ~33 attempts/s all failing except
 *    one accepted every 2 s (the ENOMEM backoff's max retry interval). Rule 1
 *    never fires -- that is the bug -- and rule 2 must, within timeout + 2
 *    ticks. */
static void test_tx_stall_occasional_ok(void)
{
    printf("TX stall, one accepted send per 2 s:\n");
    sim_t s;
    sim_init(&s, 100000, 5318);
    run_ticks(&s, 60, 330, 0);                 /* 10 min healthy */
    int64_t stall_start = s.now_us;
    for (int i = 0; i < 6 * 60 * 10 && s.fired_rule == UWD_RULE_NONE; i++) {
        sim_tick(&s, 5, 325);                  /* 1.5 % accepted */
    }
    CHECK(s.fired_rule == UWD_RULE_LOW_OK, "rule 2 fires");
    int64_t after_s = (s.fired_at_us - stall_start) / 1000000LL;
    CHECK(after_s >= TIMEOUT_S, "not before the timeout");
    CHECK(after_s <= TIMEOUT_S + 20, "within timeout + 2 ticks");
    CHECK(s.stalled_s >= TIMEOUT_S, "stalled_s reports >= timeout");
    printf("  fired rule %u after %lld s\n", s.fired_rule, (long long)after_s);
}

/* 2. Total stall, nothing accepted: rule 1 fires at the timeout (it is checked
 *    before rule 2, so the original behaviour is unchanged). */
static void test_total_stall(void)
{
    printf("total stall, nothing accepted:\n");
    sim_t s;
    sim_init(&s, 0, 0);
    run_ticks(&s, 6, 330, 0);
    int64_t stall_start = s.now_us;
    for (int i = 0; i < 400 && s.fired_rule == UWD_RULE_NONE; i++) {
        sim_tick(&s, 0, 330);
    }
    CHECK(s.fired_rule == UWD_RULE_NO_SEND, "rule 1 fires");
    int64_t after_s = (s.fired_at_us - stall_start) / 1000000LL;
    CHECK(after_s >= TIMEOUT_S - 10 && after_s <= TIMEOUT_S + 10, "at the timeout");
}

/* 3. Healthy for 24 h at ~33 sends/s: never fires. */
static void test_healthy_day(void)
{
    printf("healthy 24 h:\n");
    sim_t s;
    sim_init(&s, 0, 0);
    run_ticks(&s, 6 * 60 * 24, 330, 0);
    CHECK(s.fired_rule == UWD_RULE_NONE, "never fires");
}

/* 4. The measured self-healing silences: 225 s with every send failing,
 *    then recovery, every 10 min for 24 h. Neither rule may fire. */
static void test_self_healing_silences(void)
{
    printf("225 s self-healing silences every 10 min, 24 h:\n");
    sim_t s;
    sim_init(&s, 0, 0);
    run_ticks(&s, 6, 330, 0);
    for (int cycle = 0; cycle < 6 * 24; cycle++) {
        run_ticks(&s, 23, 0, 330);             /* ~230 s silent */
        run_ticks(&s, 37, 330, 0);             /* rest of the 10 min */
    }
    CHECK(s.fired_rule == UWD_RULE_NONE, "never fires");
}

/* 5. Heavy ENOMEM pressure for 6 h: half of sends fail. Never fires. */
static void test_enomem_pressure(void)
{
    printf("50 %% failures for 6 h:\n");
    sim_t s;
    sim_init(&s, 0, 0);
    run_ticks(&s, 6, 330, 0);
    run_ticks(&s, 6 * 60 * 6, 165, 165);
    CHECK(s.fired_rule == UWD_RULE_NONE, "never fires");
}

/* 6. One healthy tick inside a long low run resets rule 2's streak. */
static void test_one_good_tick_resets(void)
{
    printf("one good tick per 15 min inside a 2 h low run:\n");
    sim_t s;
    sim_init(&s, 0, 0);
    run_ticks(&s, 6, 330, 0);
    for (int i = 0; i < 8; i++) {
        run_ticks(&s, 89, 5, 325);
        run_ticks(&s, 1, 330, 0);
    }
    CHECK(s.fired_rule == UWD_RULE_NONE, "never fires");
}

/* 7. Not armed: no bulk send ever accepted since boot. Never fires, however
 *    long (the unbounded reconnect owns that case). */
static void test_not_armed(void)
{
    printf("never delivered since boot:\n");
    sim_t s;
    sim_init(&s, 0, 0);
    run_ticks(&s, 6 * 60 * 3, 0, 330);
    CHECK(s.fired_rule == UWD_RULE_NONE, "never fires");
}

/* 8. Too few attempts to judge (node not capturing) never builds a rule 2
 *    streak, even at 0 % accepted; rule 1 still covers a silent node. */
static void test_few_attempts(void)
{
    printf("< 10 attempts per tick:\n");
    sim_t s;
    sim_init(&s, 0, 0);
    run_ticks(&s, 6, 330, 0);
    for (int i = 0; i < 6 * 30; i++) {
        sim_tick(&s, (i % 6 == 0) ? 1 : 0, 8); /* one ok per minute keeps rule 1 armed-off */
    }
    CHECK(s.fired_rule == UWD_RULE_NONE, "rule 2 never fires on thin ticks");
}

/* 9. The 5 % boundary, exactly. */
static void test_boundary(void)
{
    printf("5 %% boundary:\n");
    sim_t a, b;
    sim_init(&a, 0, 0);
    sim_init(&b, 0, 0);
    run_ticks(&a, 6, 330, 0);
    run_ticks(&b, 6, 330, 0);
    run_ticks(&a, 140, 5, 95);                 /* exactly 5 %: not low */
    run_ticks(&b, 140, 4, 96);                 /* 4 %: low */
    CHECK(a.fired_rule == UWD_RULE_NONE, "5 % does not fire");
    CHECK(b.fired_rule == UWD_RULE_LOW_OK, "4 % fires");
}

/* 10. u32 counter wrap mid-stall: deltas stay right. */
static void test_counter_wrap(void)
{
    printf("u32 wrap during the stall:\n");
    sim_t s;
    sim_init(&s, 0xFFFFFF00u, 0xFFFFF000u);
    run_ticks(&s, 6, 330, 0);
    for (int i = 0; i < 200 && s.fired_rule == UWD_RULE_NONE; i++) {
        sim_tick(&s, 5, 325);
    }
    CHECK(s.fired_rule == UWD_RULE_LOW_OK, "rule 2 fires across the wrap");
}

/* 11. First tick after boot must not judge the counters since boot as one
 *     window (no prior sample): primed only after it. */
static void test_first_tick_not_judged(void)
{
    printf("first tick only primes:\n");
    uwd_state_t st;
    memset(&st, 0, sizeof(st));
    int64_t stalled = 0;
    uint8_t r = uwd_step(&st, 40LL * 1000000LL, 10, 100000, 39LL * 1000000LL,
                         TIMEOUT_S, &stalled);
    CHECK(r == UWD_RULE_NONE, "no rule");
    CHECK(st.low_since_us == 0, "no streak started from since-boot totals");
}


/* ---------------------------------------------------------------- rule 3 --
 * One call per completed 60 s link-watchdog window. Shape of the measured
 * dead-path wedge: healthy, then 100 % gateway-ping loss while associated, one
 * unassociated window (an external reassociate), then 94 min associated with
 * no ping answered and every sendto accepted. SYNTHETIC reconstruction. */
#define WIN_US (60LL * 1000000LL)

typedef struct {
    uwd_path_state_t st;
    int64_t now_us;
    int     fired;
    int64_t fired_at_us;
    int64_t stalled_s;
} psim_t;

static void psim_init(psim_t *p) { memset(p, 0, sizeof(*p)); p->now_us = WIN_US; }

static void pwin(psim_t *p, int n, uint32_t ok, bool assoc)
{
    for (int i = 0; i < n; i++) {
        p->now_us += WIN_US;
        int64_t st = 0;
        uint8_t r = uwd_path_step(&p->st, p->now_us, ok, assoc, TIMEOUT_S, &st);
        if (r != UWD_RULE_NONE && !p->fired) {
            CHECK(r == UWD_RULE_DEAD_PATH, "rule 3 returns UWD_RULE_DEAD_PATH");
            p->fired = 1;
            p->fired_at_us = p->now_us;
            p->stalled_s = st;
        }
    }
}

static void test_dead_path_wedge(void)
{
    printf("dead IP path, associated, 94 min:\n");
    psim_t p; psim_init(&p);
    pwin(&p, 60, 2000, true);           /* healthy hour */
    pwin(&p, 5, 0, true);               /* 100 % loss, associated */
    pwin(&p, 1, 0, false);              /* external reassociate */
    int64_t rejoin_us = p.now_us;
    pwin(&p, 94, 0, true);              /* the 94 silent minutes */
    CHECK(p.fired, "dead-path shape fires");
    int64_t after_s = (p.fired_at_us - rejoin_us) / 1000000LL;
    CHECK(after_s >= TIMEOUT_S && after_s <= TIMEOUT_S + 120,
          "fires within 2 windows of the timeout after the rejoin");
    CHECK(p.stalled_s >= TIMEOUT_S, "stalled_s reports >= timeout");
}

static void test_ap_outage_never_fires(void)
{
    /* A short AP restart (~3 min unassociated), and a 10 h one: reconnecting
     * is main.c's job, never rule 3's. */
    printf("AP outages (unassociated):\n");
    psim_t p; psim_init(&p);
    pwin(&p, 30, 2000, true);
    pwin(&p, 3, 0, false);
    pwin(&p, 30, 2000, true);
    pwin(&p, 600, 0, false);
    pwin(&p, 30, 2000, true);
    CHECK(!p.fired, "AP outages (unassociated) never fire");
}

static void test_not_armed_never_fires(void)
{
    printf("gateway never answered this boot:\n");
    psim_t p; psim_init(&p);
    pwin(&p, 300, 0, true);
    CHECK(!p.fired, "no successful ping since boot -> not armed");
}

static void test_partial_loss_never_fires(void)
{
    printf("1 answered ping per window for 500 windows:\n");
    psim_t p; psim_init(&p);
    pwin(&p, 10, 2000, true);
    pwin(&p, 500, 1, true);             /* lossy, not dead */
    CHECK(!p.fired, "any successful ping per window resets the streak");
}

static void test_dead_path_boundary(void)
{
    /* Streak counted from the END of the first dead window, so with 60 s
     * windows it fires on dead window 21 (20 windows = 1200 s later). */
    printf("rule 3 boundary:\n");
    psim_t p; psim_init(&p);
    pwin(&p, 5, 2000, true);
    pwin(&p, 20, 0, true);
    CHECK(!p.fired, "20 dead windows: not yet");
    pwin(&p, 1, 0, true);
    CHECK(p.fired, "21st dead window fires");
}

static void test_unassoc_resets_streak(void)
{
    printf("a disassociation mid-streak:\n");
    psim_t p; psim_init(&p);
    pwin(&p, 5, 2000, true);
    pwin(&p, 15, 0, true);
    pwin(&p, 1, 0, false);
    pwin(&p, 15, 0, true);
    CHECK(!p.fired, "a disassociation restarts the streak");
}

static void test_gateway_outage_documented(void)
{
    /* Known exposure: gateway down (or not answering ICMP) > timeout with the
     * AP up reboots each armed node once. Asserted so a change is deliberate. */
    printf("gateway down > timeout, AP up (accepted exposure):\n");
    psim_t p; psim_init(&p);
    pwin(&p, 5, 2000, true);
    pwin(&p, 40, 0, true);
    CHECK(p.fired, "gateway down > timeout while associated fires (accepted)");
}

int main(void)
{
    test_tx_stall_occasional_ok();
    test_total_stall();
    test_healthy_day();
    test_self_healing_silences();
    test_enomem_pressure();
    test_one_good_tick_resets();
    test_not_armed();
    test_few_attempts();
    test_boundary();
    test_counter_wrap();
    test_first_tick_not_judged();
    test_dead_path_wedge();
    test_ap_outage_never_fires();
    test_not_armed_never_fires();
    test_partial_loss_never_fires();
    test_dead_path_boundary();
    test_unassoc_resets_streak();
    test_gateway_outage_documented();
    printf("\n%d passed, %d failed\n", g_pass, g_fail);
    return g_fail ? 1 : 0;
}
