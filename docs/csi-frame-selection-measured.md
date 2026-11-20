# Which CSI frames a node keeps decides whether cross-node fusion works

A measured comparison of three frame-selection gates on a nine-node ESP32-C6
fleet, and a method for repeating it in your own environment.

**This is one house.** Nine boards, one floor plan, one RF environment, one
transmitter population. The numbers below are what we measured here; the part
worth reusing is the method, not our figures. `run_arms.py` is included so you
can measure yours.

---

## The problem

Cross-node fusion pairs frames by transmission identity: two nodes are fusing
usefully only when they hold measurements of *the same packet in the air*. The
metric is `paired_fraction` — of the transmissions the fleet reported, what
share were captured by two or more nodes.

Every node also rate-limits which CSI callbacks it keeps, because processing
every callback at 100–500 Hz destabilises the WiFi ISR. **How that limiting is
done turns out to matter far more than we expected**, because it decides
whether two nodes keep the *same* frames or merely the *same number* of frames.

Three mechanisms, all valid rate limits:

| gate | rule |
|---|---|
| `elapsed` | at least 20 ms since **this node's** last accepted frame |
| `mesh` | first frame heard in each shared 20 ms window of a mesh-aligned clock |
| `seq` | keep frames where the 802.11 `rx_seq` is divisible by N |

`elapsed` gives every node an independent phase. `mesh` makes nodes agree on
the *window*. `seq` makes them agree on the *frame*, using a number the
transmitter assigns — so it needs no clock, no leader and no synchronisation.

## Method

Four arms — the three gates plus `mesh`+`seq` stacked — run as a 2x2 factorial.
Three deliberate choices, each of which we got wrong first:

**Whole fleet per arm, never a subset.** An earlier attempt compared four gated
nodes against four ungated ones. Nodes differ enormously in how much traffic
they hear, so that comparison could not be read at all.

**Verify the gate is actually running.** Each switch is read back from every
node before sampling. "Config was stored" is not "gate is live", and an earlier
run could not distinguish a null result from a change that never applied.

**Frame rate is NOT a valid check that a gate is working.** A bucket gate needs
only one survivor per window, so at ~14 candidate frames per bucket even a
perfect 1-in-4 gate moves frame rate about 2%. Use instead

    R_window = (delta early_drop + delta accepted) / delta accepted

computed from **differenced** counters. Never divide lifetime totals: they
carry a large post-boot burst. One node read R = 1126 moments after a reboot,
26.5 an hour later, and 19.2 windowed.

**Establish the noise floor first.** A null run — three hours, no changes —
gave drift of sd **0.45 pp** across 20-minute windows, about 2x the ±0.2 pp
binomial counting error. Without that number none of the results below can be
called significant.

Arms are 20 minutes in Latin-square blocks (`U M S MS / S MS M U / MS U S M`),
so slow drift lands on every arm equally rather than on whichever ran first.

## Results

Three blocks, twelve arms, every arm verified live on all nine nodes.

| arm | block 0 | block 1 | block 2 | mean |
|---|---|---|---|---|
| `elapsed` | 0.2881 | 0.2409 | 0.2012 | **0.2434** |
| `mesh` | 0.2692 | 0.2580 | 0.2167 | **0.2480** |
| `seq` (period 8) | 0.5819 | 0.5573 | 0.5422 | **0.5605** |
| `mesh`+`seq` | 0.5639 | 0.5623 | 0.5093 | 0.5452 |

**`seq` roughly doubles pairing: +31.7 pp, 2.30x.** That gap is ~70x the
measured drift floor and reproduces in all three blocks; within-block contrasts
sit at +0.153 to +0.175 every time.

**Mesh-time alignment bought essentially nothing here: +0.5 pp**, about one
standard deviation of the noise floor. We built that gate and expected it to be
the answer; it is indistinguishable from doing nothing. Stacking it on top of
`seq` is marginally *worse* than `seq` alone.

### It is a trade, not a free win

At period 8 the seq arms carried ~75–95k transmissions per 20 minutes against
~120–160k ungated. You are buying pairing with raw frame rate. Absolute paired
transmissions per minute still went **up** (2629 vs 2524), because the frames
it discards are disproportionately ones only one node would have kept — but
whether that trade is right depends on what else consumes the stream.

One measurement that made the trade easier here: on-device edge processing was
**unaffected**. Frames actually processed stayed ~1900 per node per 4 minutes
in both modes, because that pipeline is separately rate-limited. The cost falls
on the uplink, not on local sensing.

### The mesh arm was partly an elapsed arm

The mesh gate uses mesh-time buckets only while `c6_sync_espnow_is_valid()`. A
node without valid sync falls back to the elapsed gate rather than gating on a
meaningless epoch -- correct behaviour, but it weakens this comparison.

Three of the nine nodes never synchronised during these runs. For those three,
the `mesh` and `mesh`+`seq` arms silently ran the elapsed gate. So **+0.5 pp is
not a clean fleet-wide measurement of window alignment**; it is "mesh where sync
held, elapsed where it did not", and the true effect of alignment on a fully
synchronised fleet is not measured here.

We are reporting the number as observed rather than reconstructing it, because
the arms were assigned before we understood the fallback, and a post-hoc
per-node reanalysis would not be the experiment we ran.

The cause is worth naming because it is environmental, not a firmware defect.
Time sync is leader-elected and hub-and-spoke: one node broadcasts and the rest
follow. Whether every node hears that leader is a property of the building. A
larger or more divided house fragments into sync domains, and any option that
depends on shared time then degrades quietly for the nodes that fell out --
which is precisely the situation where someone would most want to trust this
measurement.

### A prediction we got wrong

Our mesh had split into two leader domains with three nodes never synchronising
at all. We predicted `seq` would win *most* on node pairs spanning that split.
Per-pair yield says otherwise:

| pair group | pairs | seq / mesh |
|---|---|---|
| same sync domain | 7 | 5.82x |
| spans the two domains | 8 | 6.24x |
| involves a never-synced node | 21 | **5.16x** |

Cross-domain is marginally best, but the never-synced nodes are the *lowest* —
the opposite of the prediction — and within-group spread (3.6x–8.1x) dwarfs
every between-group difference.

So `seq` is **not** a workaround for broken time sync. It wins roughly
uniformly, including where sync is healthy. The useful consequence is negative:
repairing our mesh split would probably not have closed the gap, and we would
have spent that effort for nothing.

## What we suggest

Not "use `seq`". **Make it selectable and measure your own environment.** The
right answer depends on your transmitter population, your node placement and
what consumes your CSI stream — none of which we can see from here.

`run_arms.py` runs the factorial unattended: it applies each arm fleet-wide,
verifies every node adopted it, samples `/api/v1/fusion`, and writes one row
per arm. Run the null pass first; without your own drift floor the contrasts
cannot be judged.

## Limits

- One environment, nine nodes. Treat the numbers as an existence proof, not a
  constant.
- Only period 8 was characterised. 4 and 16 are unmeasured, so the
  yield/pairing curve has a single point on it.
- `paired_fraction` measures **frame pairing**, which is a precondition for
  cross-receiver work. Nothing here measures sensing accuracy, and none of it
  should be read as an accuracy claim.
