# ADR-361: Point the whole fleet at one transmitter, to remove selection ambiguity

**Status:** Proposed — built, deployed fleet-wide, A-B-A measured. **Hypothesis CONFIRMED, adoption NOT justified.** The pairing *fraction* rises sharply; paired observations per second fall.
**Date:** 2026-09-09
**Follows:** ADR-358 (falsified), ADR-359 (mechanism confirmed, not adopted), ADR-360 (falsified).

## The hypothesis, recorded long before it could be tested

From `docs/BACKLOG.md`, "Next: the actual pairing constraint":

> Two nodes can therefore fire in the same aligned window and still pick frames
> from *different* transmitters, which can never pair under the
> `(tx_mac, rx_seq)` key. Cheapest test and likely fix: **point the whole fleet
> at one strong transmitter with the existing ADR-060 MAC filter.**

It had never been run, and the reason was mechanical rather than scientific:
**`filter_mac` was writable only by `provision.py` over USB.** It is a 6-byte
blob, absent from the `cfg_key_t` scalar table the config API validates
against, so the test would have required nine physical visits. Exposing it over
the network (see the firmware change accompanying this ADR) reduced the whole
experiment to two config pushes.

## What was measured

The associated AP was chosen on measurement, not assumption. Five transmitters
reach all nine receivers; only one carries a usable rate:

```
transmitter            n_rx  total_fps  mean_rssi
8c:30:66:86:a4:21        9      109.81     -66.6   <- the AP, 26x the next
96:30:66:86:a4:21        9        4.24     -65.9
08:36:c9:71:ff:52        9        0.18     -75.9
64:16:66:cb:17:07        9        0.08     -81.7
10:a5:62:24:bb:30        9        0.06     -78.2
```

A-B-A, ten 30 s windows per arm. `/api/v1/fusion` counters are cumulative since
sink start, so they are **differenced over each window** and never divided as
lifetime totals — the trap already recorded for `early_drop`/`csi_fps_samples`.
The filter applied live on all nine nodes with **no restart**, so no reboot
confounds any arm.

```
arm            n   paired_fraction        tx/s    paired/s
A1 control    10   0.5668 +/- 0.0107      73.0     41.4
B  filtered   10   0.7287 +/- 0.0352      33.9     24.7
A2 control    10   0.5836 +/- 0.0120      81.5     47.6
```

**Pairing fraction +0.1535 against the pooled control (+26.7% relative),
Welch t = 13.26, df = 10.5, p < 0.0001.** Reversible: A2 returned to control,
+0.0168 against A1.

Per-node confirmation that the filter did what it claims, taken on node 3 while
eight nodes were still unfiltered:

```
node 3 (filtered)   AP fps 13.56   non-AP fps 0.28   non-AP share  2.0%
node 8 (control)    AP fps 12.18   non-AP fps 5.66   non-AP share 31.7%
```

The AP rate is untouched; peer traffic falls to residue.

## The hypothesis is confirmed. The fix is not.

Transmitter ambiguity **was** suppressing pairing, and removing it lifts the
fraction by a quarter. That settles a question that had been open since
2026-09-07 and had resisted three other experiments.

But the backlog called this the "likely fix", and that half does not survive.
It predicted yield would fall to ~13 fps and that "the frames that remain are
pairable". Both are true, and neither is sufficient:

- **tx/s fell 56%** (77.3 -> 33.9)
- **paired observations per second fell 44%** (44.4 -> 24.7)

A higher fraction of a much smaller pie. The quantity downstream sensing
actually consumes is paired observations per second, and that went **down**.
Fraction was the wrong figure of merit, and choosing it is what made this look
like a fix rather than a diagnosis.

## Decision

**Do not adopt. Keep the branch unmerged.** Production stays unfiltered,
`gate_mode=2`, `gate_seq_period=8`.

`filter_mac` **is** kept in the config API. It costs nothing when unset, it is
now the cheapest available probe for "is this node's pairing transmitter-
limited?", and the ADR-060 filter had been effectively unreachable for the
fleet's whole life.

## What this opens

A **partial** filter is the obvious next question and is NOT what was tested
here. Restricting to a handful of widely-heard transmitters rather than exactly
one would trade less yield for some of the gain, and there is no reason to think
the optimum is at either extreme. The five 9-receiver transmitters above are the
natural candidate set, though four of them carry almost no rate.

That is a new experiment with a different independent variable, not a variant
of this one. It should be sized on **paired observations per second**, which is
the lesson this ADR paid for.

## Honest scope

Software-only. One house, one AP, nine nodes, ~15 minutes per arm, at 05:00
local with the house quiet. The effect is far larger than the within-arm
scatter and it reversed cleanly, but it has not been repeated across times of
day, and pairing is known to move with the transmitter population (see
"Transmitter population instability", 2026-09-02).
