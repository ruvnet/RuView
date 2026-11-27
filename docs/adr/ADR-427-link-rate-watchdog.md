# ADR-427: Node-side link-rate watchdog driven by the self-ping gateway RTT

Status: **Proposed** (upstream). Running in observe mode on a 9-node ESP32-C6 deployment in the contributor's fork.

Date: 2026-11-20

## Context

Evidence comes from one 9-node ESP32-C6 deployment on a single 2.4 GHz channel. It was MEASURED in that deployment, but the raw data is not public, so it carries no reproducer here:

- **Uplink rate collapses are real, sticky and expensive.** Over 8 days, 27 sustained onsets were seen: >= 5 Mbps for 20 min, then <= 2 Mbps for 20 min. They hit 5 of the 9 nodes. Once low, a node stayed low for 40-50 min, and once for a full day, until it reassociated or rebooted. A node held at 1 Mbps occupies roughly a tenth of the channel by itself. Fleet frame rate correlated -0.84 with sum(1/rate) across nodes.
- **There is no known mechanism.** Most onsets coincided with other IoT clients on the same AP dropping and rejoining. Blocking 802.11b data rates removed the 1 Mbps data frames but not 2 Mbps RTS or 5.5 Mbps retries. `esp_wifi_set_protocol` on the C6 (IDF v5.4) rejects every 2.4 GHz bitmap without 11b. A remedy that works without knowing the mechanism is needed.
- **The node's existing health signals cannot see it.** Across 10,867 node-minutes of on-node log aligned to the AP's per-client rate, `send_fail > 0` showed up in 0.9 % of collapsed minutes and heap <= 150 KiB in 4.8 %.
- **Gateway RTT can see it.** A small sample of 20 pings per node gave 38-40 ms average on the two collapsed nodes and 8-18 ms on the others; a node at 1 Mbps gave 67-83 ms.
- **The node already pings the gateway up to 50 times a second.** `csi_collector.c` runs an infinite `esp_ping` session to the gateway as a CSI source (#954, `CONFIG_CSI_SELF_PING_HZ`). Its callbacks were no-ops, so lwIP measured the RTT and threw it away.
- **A whole-AP outage hits every node at once.** One AP restart left 7 nodes silent for ~130 s and 2 nodes for ~3.5 min, and all recovered with no reboot. A watchdog that read that as "my link is bad" would reboot every node, re-run ESP-NOW leader election and discard mesh time sync, all for nothing.
- `CONFIG_UPLINK_WATCHDOG` (main.c) covers "cannot put a packet on the wire". It deliberately does not cover "putting packets on the wire slowly", which is this failure.

## Decision

Add an optional link-rate watchdog to the node firmware. It reads the RTT of the existing self-ping, detects a sustained slow uplink relative to the node's own baseline, and remedies it with a rate-limited ladder: reassociate first, and reboot only if that did not help. It is compiled in behind Kconfig (`CONFIG_LINK_WATCHDOG`, default **n**), and every decision is logged.

### Signal

- Callbacks on the self-ping session record, per window (default 60 s): success count, timeout count, and the median and p90 of `ESP_PING_PROF_TIMEGAP`. They use a fixed-size histogram with no heap allocation per ping.
- Replies slower than `CONFIG_CSI_SELF_PING_TIMEOUT_MS` (default 200) count as losses. The timeout must stay well above `abs_ms`, or a slow link reads as an outage instead of being measured.
- **Baseline:** an exponentially weighted median of the node's own *healthy* windows (loss < 5 %). It is updated only while not degraded, with a floor (default 5 ms) so that ordinary jitter on a very clean link does not look like a collapse.
- **Degraded window:** median RTT > max(`k` x baseline, `abs_ms`) (defaults `k` = 3, `abs_ms` = 40) with loss < 50 %.
- **Outage window:** loss >= 50 % or no association. An outage is never counted as degraded; it belongs to the reconnect logic and `UPLINK_WATCHDOG`.

### Actions (ladder)

1. **Reassociate** after `n_reassoc` consecutive degraded windows (default 10). This calls `esp_wifi_disconnect()` and lets the existing backoff reconnect run, which resets the driver's rate control while keeping mesh time sync. A re-learn grace (default 10 windows) follows.
2. **Reboot** (`esp_restart()`) if the link is still degraded for `n_reboot` consecutive windows after the grace (default 10).

- **Rate limits:** at most 1 reassociation per 30 min, and 1 reboot per 6 h. The reboot period doubles after each reboot that did not cure the collapse, capped at 8x. The limits are persisted in NVS so that a reboot loop cannot reset them. After the limit, the watchdog only logs.
- **Inhibits:** no action while an OTA image is `PENDING_VERIFY` (`ota_health_is_pending_verify()`; a reboot there rolls the image back), while a config trial is pending (`config_trial_is_pending()`), or within 15 min of boot or of any disconnect.

### No fleet-wide action on a whole-AP or channel-wide event

1. **Outage is not degradation (guard 1).** Disconnected, or >= 50 % loss, resets the streak. An AP restart therefore never accumulates degraded windows. It ends in a disconnect, which starts the post-disconnect inhibit.
2. **Peer quorum hold (guard 2).** Each node advertises a "link degraded" bit to its mesh peers. A node acts only if fewer than half of the valid peers heard in the last 5 min are degraded.
   **Not wired in this implementation:** the upstream ESP-NOW sync beacon (`c6_sync_espnow.c`, proto v1) has no field for the bit, and adding one is a beacon-format change left to a separate decision. `link_watchdog_note_peer()` exists but has no caller, so the quorum is always empty and never holds. **Act mode therefore runs without guard 2.**
3. **Jitter (guard 3).** Each node delays an action by a random 0-5 min and re-checks before acting, so nodes that collapse in the same event do not act in the same second.

### Configuration and reporting

- Kconfig: `CONFIG_LINK_WATCHDOG` (default n), window, `k`, `abs_ms`, baseline floor, `n_reassoc`, `n_reboot`, grace, rate limits, boot grace.
- NVS key `linkwd` (u8): 0 off, 1 **observe** (detect, log "would act", never act; the default), 2 act. It is writable live through `POST /config` (no trial, no reboot).
- `node_log` event `NODE_LOG_EV_LINK_WD` (id **8**; ids 6 and 7 are reserved by a downstream fleet's on-flash format) on every state change and every action or would-act. It records the action, inhibit, window median RTT, loss, baseline, peers degraded and peers valid; the packing is documented in `node_log.h`.
- `GET /config` gains `active.link_wd`: mode, baseline, last window median/p90/loss, degraded streak, grace, actions and would-actions, last action and its time, current inhibit, and peers.

## Validation

- **Host (SYNTHETIC):** `tests/host/test_link_watchdog.c` covers:
  - the ladder
  - an AP outage never accumulating
  - quorum hold
  - rate limits surviving a simulated reboot
  - the baseline not learning while degraded
  - observe mode never acting
  - windows at a lower ping rate
  - a negative control

  Breaking the baseline rule on purpose was confirmed to fail the suite.
- **Observe mode on hardware:** running with `linkwd` = 1 on one node of a 9-node deployment since 2026-10-02, and on all 9 since 2026-10-09. Would-act events are compared against the AP's own per-client rate. The pass criterion for guard 1 is zero would-act events across an AP restart.
- **Act mode** is a per-node decision after a clean observe period. In this implementation it also lacks guard 2 (above).

## Alternatives considered

- **Kick the client from the AP controller:** this needs controller write access from automation, and it is not portable.
- **Scheduled reboots:** these cure the symptom blindly, cost mesh re-election every time, and record nothing.
- **send_fail / heap thresholds:** measured to be unusable (see Context).
- **Read the STA's own TX rate:** IDF v5.4 has no public API for it. `esp_wifi_get_tx_statistics()` is private and test-oriented.
- **Fix the mechanism:** preferred if it is ever found. The watchdog is the safety net either way.

## Consequences

- An indefinite collapse becomes at most ~20-30 min in act mode, at the cost of an occasional reassociation or reboot on the affected node only.
- The self-ping cadence is unchanged; this ADR only reads it.
- Until a peer feed exists, act mode has no defence against a channel-wide slowdown beyond guard 1, guard 3 and the rate limits. Observe mode is unaffected.
