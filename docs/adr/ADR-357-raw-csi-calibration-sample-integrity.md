# ADR 357: Raw CSI calibration sample integrity

Status: Accepted

Date: 2026-09-06

## Context

The empty room field model requires 1,000 accepted observations over at least
600 seconds. The sensing server previously invoked calibration from both the
raw CSI path and the one second edge vital path. Edge vital packets did not add
a CSI observation, so that path repeatedly fed the existing history tail. A
counter could therefore satisfy the frame gate without representing 1,000
independent CSI packets.

Node liveness also combined edge vital and CSI traffic. A source emitting only
edge vital summaries could appear live enough to start calibration even though
it had no fresh raw CSI. Finally, cloning the complete per node history before
each feed copied as much as 204,800 bytes for a 100 frame, 256 subcarrier
window even though calibration consumes only the current observation.

The earlier single installation result recorded in ADR 355 predates this
admission boundary. Its frame count is not accepted as evidence of independent
CSI coverage. It remains useful only as diagnosis of the failure mode and must
not be promoted as a production bootstrap prior.

## Decision

1. Only a packet on the frozen, model specific CSI grid may advance field
   calibration. Edge vital packets update their own health and presentation
   state but never feed the field model. The independent live inference grid
   remains unchanged.
2. Each calibration session stores the last admitted wrapping sequence for
   every contributing node. Duplicate sequences are ignored. Normal forward
   progress and the `u32::MAX` to zero wrap are accepted.
3. A packet one through sixteen sequence values behind the current high water mark
   is bounded UDP reordering. It is recorded and dropped without feeding the
   model or moving the high water mark. A backward depth above sixteen, or an
   ambiguous half range sequence, latches that node as faulted for the remainder
   of the session. Status exposes the policy, reorder window, per node reorder
   count, maximum observed depth, and `sequence_fault_node_ids`. Finalization
   returns `calibration_sequence_discontinuity` after a fault. The operator must
   cancel and restart the empty room capture.
4. A requested source may start calibration only after a header only, bounded
   observation window identifies one stable grid. The selected grid needs at
   least ten seconds of evidence, a measured rate of at least 2 Hz, a latest
   sample newer than five seconds, and no observed gap of five seconds or more.
   Calibration status reports the immutable grid, selection evidence, current
   grid age, and `last_sequence_by_node` without exposing CSI values.
5. Start, cancel, reset, and process initialization clear sequence admission
   state. A failed feed does not advance the sequence cursor, so the same
   packet may be retried after a transient model input failure.
6. The field bridge receives the current amplitude slice rather than a cloned
   history. A separate bounded runtime history retains only that same frozen
   grid, so the model is never trained on one symbol layout and evaluated on a
   different layout. Canonical grid normalization remains unchanged.
7. The exact source grid is stored inside the digest protected bootstrap v2
   payload. A restored model rebuilds only its matching runtime history. An
   unbound legacy image cannot become an active startup prior.
8. Successful finalization clears the selected grid runtime history. Bootstrap
   promotion waits up to 35 seconds for a complete fresh runtime window before
   scoring twelve additional samples. This covers a 50 frame window at the 2 Hz
   admission floor plus ten seconds of bounded jitter. Calibration observations
   therefore cannot enter the held out decision window.
9. Stop and promotion require the frozen grid to remain fresh and reject any
   latched sequence fault immediately before persistence. Bootstrap v2 accepts
   exactly one source node because the current runtime supports one grid bound
   field model.
10. The legacy `--calibrate` startup switch fails closed before opening ports.
    Grid evidence does not exist at process start, so operators start the server,
    wait ten seconds, then call the explicit single source calibration API.

## Consequences

Calibration frame count now means unique, forward raw CSI observations within
one uninterrupted sequence epoch. A vitals only node cannot produce a
bootstrap model. Late packets never become calibration evidence.

One physical Node 5 capture observed sequence 4,119,566 followed by 4,119,563,
4,119,564, and 4,119,565 in one receive batch. A second independent 90 second
capture observed bounded late packets at depths 9, 10, and 11. The window of
sixteen covers the measured maximum with five positions of margin. At the
firmware acceptance ceiling of 50 Hz, the window represents no more than 320
milliseconds. Late packets are always dropped. Larger regressions remain
failed closed.

The same 90 second capture measured three interleaved HT grids on Node 5. The
64 subcarrier stream delivered 436 frames at 4.84 Hz with a 1.56 second maximum
gap. The 128 stream delivered 100 frames at 1.11 Hz with a 5.05 second maximum
gap. The 192 stream delivered 49 frames at 0.54 Hz with an 11.53 second maximum
gap. Only the 64 grid exceeded the 2 Hz admission floor and stayed inside the
five second freshness boundary, so the final field model binds to that grid.

The current wire format still has no authenticated boot epoch or stable device
identity. A restart very early in a session or a same endpoint adversary cannot
be distinguished cryptographically. A future ADR 018 revision must carry an
authenticated device identifier and a random per boot identifier before the
server claims exact restart or identity collision detection.

No new raw CSI is persisted. Sequence cursors and fault markers remain bounded
in memory to active calibration sources. The bootstrap schema advances to v2
to protect the grid identity while its negative only authority remains
unchanged.

## Implementation

1. `NodeState` records header only recent grid evidence and a separate bounded
   field model history.
2. `AppStateInner::maybe_feed_calibration_frame` owns source binding, sequence
   ordering, fault latching, model feed, and cursor commit.
3. `field_bridge::maybe_feed_calibration` accepts exactly one current amplitude
   slice and rejects an empty observation.
4. Calibration start, status, stop, cancel, reset, promotion, refinement, and
   restart enforce the frozen source and grid contract.
5. `bootstrap_baseline` persists the source grid inside the verified v2 image.
6. Finalization clears training history, and promotion blocks until the model
   has a complete fresh scoring window plus twelve separately spaced samples.
7. Startup calibration without measured grid evidence exits with a typed
   operator instruction instead of creating a model that can never receive a
   frame.

## Acceptance test

Unit tests must prove stable grid selection, rate and gap rejection, first
packet acceptance, forward progress, duplicate rejection, source and grid
isolation, maximum counter wrap, empty input retry, measured depth eleven
reordering without a model feed, depth sixteen acceptance, depth seventeen
latching, half range rejection, session clear, receipt visibility, protected
grid persistence, and typed stop refusal.
Tests must also prove startup flag refusal, missing and stale grid refusal,
complete holdout isolation, and sequence fault rejection during holdout.

On physical hardware, begin a Node 5 empty room capture and observe a fresh
sixty second interval. Calibration frame growth must match unique 64 subcarrier
Node 5 sequences while the independent global inference grid remains unchanged.
Edge vital packets and other grids must produce zero growth. The bound grid
must remain active, sequence faults must remain empty, and numeric vital output
must remain absent. Any bounded late packet must appear in the reorder receipts
while leaving the model count unchanged. Complete both the 1,000 frame and 600
second gates, then run the twelve sample server controlled bootstrap holdout.
Restart and prove that only the persisted grid rebuilds the runtime history.
Do not claim human detection improvement until a separate consented occupied
holdout demonstrates retained recall.
