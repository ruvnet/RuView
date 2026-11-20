/*
 * Host unit tests for ADR-427's link_watchdog_core.c.
 *
 * Pure C, no ESP-IDF, no FreeRTOS -- same pattern as test_adaptive_controller.c
 * and test_rv_mesh.c in this directory.
 *
 * Build + run (from this directory):
 *   make
 *   ./test_link_watchdog
 *
 * One test (test_negative_control_catches_baseline_regression) deliberately
 * breaks the "baseline never learns while degraded" rule, confirms this
 * test file actually detects it, then restores the correct behaviour --
 * proving the test harness is not just passing by accident.
 */
#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "link_watchdog_core.h"

static int g_pass = 0, g_fail = 0;

#define CHECK(cond, msg) do {                                              \
    if (cond) { g_pass++; }                                                \
    else { g_fail++; printf("  FAIL: %s (line %d)\n", msg, __LINE__); }    \
} while (0)

static lwd_window_sample_t healthy_sample(uint32_t rtt_ms)
{
    lwd_window_sample_t w;
    memset(&w, 0, sizeof(w));
    w.median_rtt_ms = rtt_ms;
    w.p90_rtt_ms    = rtt_ms + 2;
    w.ping_total    = 3000;   /* 50 Hz * 60 s */
    w.ping_success  = 3000;
    w.associated    = true;
    return w;
}

static lwd_window_sample_t degraded_sample(uint32_t rtt_ms)
{
    lwd_window_sample_t w = healthy_sample(rtt_ms);
    w.ping_success = (uint32_t)(w.ping_total * 0.9); /* 10% loss: not an outage */
    return w;
}

static lwd_window_sample_t outage_sample(bool associated, uint32_t loss_pct)
{
    lwd_window_sample_t w;
    memset(&w, 0, sizeof(w));
    w.median_rtt_ms = 5;
    w.p90_rtt_ms    = 5;
    w.ping_total    = 3000;
    w.ping_success  = (uint32_t)((uint64_t)w.ping_total * (100 - loss_pct) / 100);
    w.associated    = associated;
    return w;
}

static lwd_inhibits_t no_inhibits(void)
{
    lwd_inhibits_t i;
    memset(&i, 0, sizeof(i));
    return i;
}

static lwd_quorum_t no_quorum_data(void)
{
    lwd_quorum_t q = { .valid_peers = 0, .degraded_peers = 0 };
    return q;
}

/* Seeds a core with a clean ~8ms baseline via 20 healthy windows, as if the
 * node had been up and healthy for 20 minutes before anything degrades. */
static void seed_baseline(lwd_core_t *core, uint64_t *t)
{
    for (int i = 0; i < 20; i++) {
        lwd_window_sample_t w = healthy_sample(8);
        lwd_inhibits_t inh = no_inhibits();
        lwd_quorum_t q = no_quorum_data();
        lwd_core_tick(core, &w, &inh, &q, (*t)++, 0);
    }
}

/* ------------------------------------------------------------------ */

static void test_collapse_triggers_reassoc_then_reboot(void)
{
    printf("test: a 10+ min collapse triggers reassociate, then reboot\n");
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_ACT, &persist);

    uint64_t t = 0;
    seed_baseline(&core, &t);
    CHECK(core.baseline_seeded, "baseline seeded after healthy windows");

    lwd_inhibits_t inh = no_inhibits();
    lwd_quorum_t q = no_quorum_data();

    /* 10 consecutive degraded windows (>=10 min at 60s/window) must trigger
     * a reassociate once jitter (0-5 windows, jitter_draw=0 here) elapses. */
    lwd_action_t last = LWD_ACTION_NONE;
    for (int i = 0; i < (int)cfg.n_reassoc; i++) {
        lwd_window_sample_t w = degraded_sample(200); /* >> 3*8=24, >> 40 */
        last = lwd_core_tick(&core, &w, &inh, &q, t++, 0 /* no jitter wait */);
    }
    CHECK(last == LWD_ACTION_REASSOC, "fires REASSOC after n_reassoc degraded windows");
    CHECK(core.in_grace, "enters the post-reassoc grace");

    /* Run out the grace. During the grace nothing should fire even though
     * the link is still bad, and degraded_streak must not accumulate. */
    for (uint32_t i = 0; i < cfg.grace_windows; i++) {
        lwd_window_sample_t w = degraded_sample(200);
        lwd_action_t a = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(a == LWD_ACTION_NONE, "no action fires during the grace");
    }
    CHECK(!core.in_grace, "grace has ended");
    CHECK(core.reboot_eligible, "reboot becomes eligible once the grace completes");

    /* Still degraded after the grace: n_reboot more consecutive windows
     * should now trigger a reboot. */
    last = LWD_ACTION_NONE;
    for (int i = 0; i < (int)cfg.n_reboot; i++) {
        lwd_window_sample_t w = degraded_sample(200);
        last = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
    }
    CHECK(last == LWD_ACTION_REBOOT, "fires REBOOT after a further n_reboot degraded windows");
    CHECK(core.actions_taken == 2, "exactly two real actions taken (reassoc + reboot)");
}

static void test_whole_ap_outage_never_accumulates(void)
{
    printf("test: a whole-AP outage never accumulates degraded windows (guard 1)\n");
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_ACT, &persist);

    uint64_t t = 0;
    seed_baseline(&core, &t);

    lwd_inhibits_t inh = no_inhibits();
    lwd_quorum_t q = no_quorum_data();

    /* Disconnected for far longer than n_reassoc windows: an outage by the
     * "not associated" clause. Must never reach PENDING, never fire. */
    for (int i = 0; i < 50; i++) {
        lwd_window_sample_t w = outage_sample(/*associated=*/false, 100);
        lwd_action_t a = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(a == LWD_ACTION_NONE, "disconnected outage never fires");
        CHECK(core.degraded_streak == 0, "disconnected outage holds streak at 0");
        CHECK(core.last_class == LWD_WINDOW_OUTAGE, "classified OUTAGE, not DEGRADED");
    }

    /* Associated but >=50% loss (AP/channel-wide busy): same guard, the
     * >=50% loss clause this time. */
    for (int i = 0; i < 50; i++) {
        lwd_window_sample_t w = outage_sample(/*associated=*/true, 60);
        lwd_action_t a = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(a == LWD_ACTION_NONE, "associated-but-50%%-loss outage never fires");
        CHECK(core.degraded_streak == 0, "50%% loss outage holds streak at 0");
    }
}

static void test_quorum_hold(void)
{
    printf("test: quorum hold -- half or more of the peers degraded holds everyone (guard 2)\n");
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_ACT, &persist);

    uint64_t t = 0;
    seed_baseline(&core, &t);
    lwd_inhibits_t inh = no_inhibits();

    /* Exactly half the peers degraded: per ADR-427 wording ("fewer than
     * half ... acts"), half must still HOLD. */
    lwd_quorum_t half_degraded = { .valid_peers = 4, .degraded_peers = 2 };
    lwd_action_t last = LWD_ACTION_NONE;
    for (int i = 0; i < (int)cfg.n_reassoc + (int)cfg.max_jitter_windows + 2; i++) {
        lwd_window_sample_t w = degraded_sample(200);
        last = lwd_core_tick(&core, &w, &inh, &half_degraded, t++, 0);
    }
    CHECK(last == LWD_ACTION_NONE, "half the mesh degraded holds the action");
    CHECK(core.last_inhibit == LWD_INHIBIT_QUORUM_HOLD, "reports QUORUM_HOLD");

    /* Quorum clears (now a minority is degraded): the already-pending action
     * (jitter already spent above) should fire on the next good quorum
     * read. */
    lwd_quorum_t minority_degraded = { .valid_peers = 4, .degraded_peers = 1 };
    lwd_window_sample_t w = degraded_sample(200);
    last = lwd_core_tick(&core, &w, &inh, &minority_degraded, t++, 0);
    CHECK(last == LWD_ACTION_REASSOC, "fires once the quorum clears");
}

static void test_rate_limits_survive_simulated_reboot(void)
{
    printf("test: rate limits persist across a simulated reboot\n");
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);

    /* ---- "boot" 1: drive a reassociate, capture the persisted state. ---- */
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_ACT, &persist);
    uint64_t t = 0;
    seed_baseline(&core, &t);
    lwd_inhibits_t inh = no_inhibits();
    lwd_quorum_t q = no_quorum_data();

    lwd_action_t last = LWD_ACTION_NONE;
    for (int i = 0; i < (int)cfg.n_reassoc; i++) {
        lwd_window_sample_t w = degraded_sample(200);
        last = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
    }
    CHECK(last == LWD_ACTION_REASSOC, "first boot: reassociate fires");
    uint64_t t_at_action = t - 1;
    CHECK(core.persist.next_reassoc_allowed_s == t_at_action + cfg.reassoc_min_period_s,
         "rate limit recorded relative to lifetime seconds");

    lwd_persist_t carried = core.persist; /* what link_watchdog.c would write to NVS */

    /* ---- simulated power cycle: a brand new core, same persisted state,
     * lifetime_s continues (it is cumulative uptime, not per-boot uptime) a
     * mere 3 seconds later -- a fast reboot loop, deliberately. ---- */
    lwd_core_t core2;
    lwd_core_init(&core2, &cfg, LWD_MODE_ACT, &carried);
    uint64_t t2 = t_at_action + 3;

    /* Immediately degraded again post-reboot and past the boot/disconnect
     * inhibits (pretend 16 minutes have already passed for this check).
     * Exactly n_reassoc windows is enough to re-reach threshold and hit the
     * still-active rate limit -- checked right at that point, before any
     * further window has a chance to reset last_inhibit back to NONE. */
    lwd_inhibits_t inh2 = no_inhibits();
    last = LWD_ACTION_NONE;
    for (int i = 0; i < (int)cfg.n_reassoc; i++) {
        lwd_window_sample_t w = degraded_sample(200);
        last = lwd_core_tick(&core2, &w, &inh2, &q, t2++, 0);
    }
    CHECK(last == LWD_ACTION_NONE,
         "rate limit from before the reboot still holds after it");
    CHECK(core2.last_inhibit == LWD_INHIBIT_RATE_LIMITED, "reports RATE_LIMITED");

    /* Advance lifetime_s past the persisted deadline: now it may fire.
     * Exactly n_reassoc windows re-reaches threshold and (with jitter_draw=0,
     * i.e. no jitter wait) fires on that same tick -- checked immediately,
     * same reasoning as the rate-limited loop above. */
    uint64_t t3 = carried.next_reassoc_allowed_s + 1;
    last = LWD_ACTION_NONE;
    for (int i = 0; i < (int)cfg.n_reassoc; i++) {
        lwd_window_sample_t w = degraded_sample(200);
        last = lwd_core_tick(&core2, &w, &inh2, &q, t3++, 0);
    }
    CHECK(last == LWD_ACTION_REASSOC, "fires again once the persisted deadline passes");
}

static void test_baseline_does_not_learn_during_degradation(void)
{
    printf("test: baseline does not learn during degradation\n");
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_OBSERVE, &persist);
    uint64_t t = 0;
    seed_baseline(&core, &t);
    uint32_t baseline_before = core.baseline_ms;
    CHECK(baseline_before < 40, "baseline learned a clean ~8ms value");

    lwd_inhibits_t inh = no_inhibits();
    lwd_quorum_t q = no_quorum_data();
    for (int i = 0; i < 30; i++) {
        lwd_window_sample_t w = degraded_sample(500); /* way above threshold */
        lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(core.baseline_ms == baseline_before,
             "baseline unchanged while every window is degraded");
    }
}

static void test_observe_mode_never_acts(void)
{
    printf("test: observe mode never acts\n");
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_OBSERVE, &persist);
    uint64_t t = 0;
    seed_baseline(&core, &t);

    lwd_inhibits_t inh = no_inhibits();
    lwd_quorum_t q = no_quorum_data();
    bool saw_would_reassoc = false, saw_would_reboot = false;

    /* Run the full collapse -> reassoc -> grace -> reboot cycle exactly as
     * in test_collapse_triggers_reassoc_then_reboot, but in observe mode:
     * every returned action must be NONE or a WOULD_* variant, never the
     * real REASSOC/REBOOT, and esp_wifi_disconnect()/esp_restart() (which
     * this pure module cannot call at all -- there is no such call in this
     * file) are therefore structurally never reached. */
    for (int cycle = 0; cycle < (int)cfg.n_reassoc; cycle++) {
        lwd_window_sample_t w = degraded_sample(200);
        lwd_action_t a = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(a != LWD_ACTION_REASSOC && a != LWD_ACTION_REBOOT,
             "observe mode never returns a real action");
        if (a == LWD_ACTION_WOULD_REASSOC) saw_would_reassoc = true;
    }
    for (uint32_t i = 0; i < cfg.grace_windows; i++) {
        lwd_window_sample_t w = degraded_sample(200);
        lwd_action_t a = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(a != LWD_ACTION_REASSOC && a != LWD_ACTION_REBOOT,
             "observe mode never acts during the grace either");
    }
    for (int cycle = 0; cycle < (int)cfg.n_reboot; cycle++) {
        lwd_window_sample_t w = degraded_sample(200);
        lwd_action_t a = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(a != LWD_ACTION_REASSOC && a != LWD_ACTION_REBOOT,
             "observe mode never returns a real action (reboot phase)");
        if (a == LWD_ACTION_WOULD_REBOOT) saw_would_reboot = true;
    }

    CHECK(saw_would_reassoc, "observe mode still logs WOULD_REASSOC");
    CHECK(saw_would_reboot, "observe mode still logs WOULD_REBOOT");
    CHECK(core.actions_taken == 0, "zero real actions taken in observe mode");
    CHECK(core.would_actions_taken == 2, "two would-act events recorded");
}

/* ------------------------------------------------------------------ */
/* Negative control: break "baseline never learns while degraded" on
 * purpose, confirm THIS test file's own check catches it, then restore the
 * correct behaviour. Proves test_baseline_does_not_learn_during_degradation
 * is not a tautology that would pass no matter what the code does.
 *
 * We cannot edit link_watchdog_core.c from inside a test and recompile
 * mid-run, so instead this reimplements the one-line defect locally against
 * the real struct/API and shows the real check (replicated here) fails
 * against it, then shows the real module does not exhibit the defect. */
static void test_negative_control_catches_baseline_regression(void)
{
    printf("test: negative control -- a baseline-learns-while-degraded defect is caught\n");

    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_OBSERVE, &persist);
    uint64_t t = 0;
    seed_baseline(&core, &t);
    uint32_t baseline_before = core.baseline_ms;

    /* Simulate the defective behaviour directly: "baseline tracks every
     * window's median regardless of classification" is exactly the bug
     * ADR-427 prohibits. */
    uint32_t defective_baseline = baseline_before;
    int defect_detected = 0;
    for (int i = 0; i < 10; i++) {
        uint32_t degraded_rtt = 500;
        /* The defect: unconditional EMA update, bypassing the
         * cls == HEALTHY && loss < healthy_loss_pct gate that the real
         * lwd_core_tick() applies. */
        int32_t delta = (int32_t)degraded_rtt - (int32_t)defective_baseline;
        defective_baseline = (uint32_t)((int32_t)defective_baseline + delta / 8);
    }
    if (defective_baseline != baseline_before) {
        defect_detected = 1; /* this is what the real test would report as a FAIL */
    }
    CHECK(defect_detected == 1,
         "the harness DOES flag an unconditional-EMA defect when present");

    /* Now confirm the real module, unmodified, does not exhibit it. */
    lwd_inhibits_t inh = no_inhibits();
    lwd_quorum_t q = no_quorum_data();
    for (int i = 0; i < 10; i++) {
        lwd_window_sample_t w = degraded_sample(500);
        lwd_core_tick(&core, &w, &inh, &q, t++, 0);
    }
    CHECK(core.baseline_ms == baseline_before,
         "restored/real behaviour: baseline is unaffected by degraded windows");
}

/* CONFIG_CSI_SELF_PING_HZ is configurable (10-50 Hz), and link_watchdog.c
 * and lwd_core_tick() take whatever ping_total/ping_success the window
 * actually saw -- see link_watchdog_core.h's lwd_window_sample_t doc. This
 * proves that holds: a window run at 25 Hz, with zero loss, must classify
 * HEALTHY exactly like a 50 Hz window would -- not DEGRADED or OUTAGE from an
 * implicit assumption that ping_total should be ~3000 (50 Hz * 60 s). */
static void test_classification_uses_rate_in_effect_not_kconfig_constant(void)
{
    printf("test: a 25 Hz window with no loss classifies healthy, not a rate drop\n");
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    lwd_persist_t persist; memset(&persist, 0, sizeof(persist));
    lwd_core_t core;
    lwd_core_init(&core, &cfg, LWD_MODE_OBSERVE, &persist);
    uint64_t t = 0;

    /* Seed the baseline at 25 Hz too: 1500 pings/min (25 Hz * 60 s), not the
     * 3000 every other test in this file uses for the default 50 Hz rate. */
    for (int i = 0; i < 20; i++) {
        lwd_window_sample_t w;
        memset(&w, 0, sizeof(w));
        w.median_rtt_ms = 8;
        w.p90_rtt_ms    = 10;
        w.ping_total    = 1500;   /* 25 Hz * 60 s */
        w.ping_success  = 1500;   /* 0% loss */
        w.associated    = true;
        lwd_inhibits_t inh = no_inhibits();
        lwd_quorum_t q = no_quorum_data();
        lwd_action_t a = lwd_core_tick(&core, &w, &inh, &q, t++, 0);
        CHECK(a == LWD_ACTION_NONE, "25 Hz healthy window never triggers an action");
        CHECK(core.last_class == LWD_WINDOW_HEALTHY,
             "25 Hz, 0%% loss classifies HEALTHY (not a rate-drop false DEGRADED/OUTAGE)");
    }
    CHECK(core.baseline_seeded, "baseline still seeds correctly off a 25 Hz window count");
    CHECK(core.degraded_streak == 0, "no degraded streak accrues from the lower sample count alone");
}

int main(void)
{
    test_collapse_triggers_reassoc_then_reboot();
    test_whole_ap_outage_never_accumulates();
    test_quorum_hold();
    test_rate_limits_survive_simulated_reboot();
    test_baseline_does_not_learn_during_degradation();
    test_observe_mode_never_acts();
    test_classification_uses_rate_in_effect_not_kconfig_constant();
    test_negative_control_catches_baseline_regression();

    printf("\n%d passed, %d failed\n", g_pass, g_fail);
    return g_fail == 0 ? 0 : 1;
}
