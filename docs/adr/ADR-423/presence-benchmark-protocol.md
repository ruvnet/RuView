# Radar-gated, per-zone presence benchmark with a held-out room

Protocol for ADR-423's presence claim. It produces one number per held-out
room: how well CSI presence matches a 60 GHz radar's presence, compared with a
baseline that always predicts the most common class. The tooling is in
`scripts/`; the hardware runs are done by whoever has the rooms.

**Scope.** Presence per zone only. A result from this protocol supports no
claim about pose, person count, identity or activity.

## Equipment per room

| Item | Role |
|---|---|
| CSI node(s), one per zone | Prediction source (ESP32-S3/C6/C5 or RTL8721Dx) |
| MR60BHA2 60 GHz radar on an ESPHome host board, one per zone | Reference label |
| Host running `sensing-server` and both recorders | One clock for everything |

Mount each radar so its field of view covers its zone and **only** its zone,
because overlapping coverage mixes the zones' labels. Run the radar recorder
and the CSI recorder on the same host so both use the same wall clock.

## Rooms and sessions

- At least **3 rooms** (the scorer refuses fewer than 2). Rooms should
  differ in layout and in AP placement. Two zones of one room are not two rooms.
- Per room: at least **2 sessions of 10 minutes or more**, on different days if possible.
- Each session mixes empty periods (everyone out of the zone, door shut where
  possible) with occupied periods. Occupied periods must include someone
  **still**: seated or lying, not only walking. Aim for 30–70 % occupied time.
- Note anything that changes the RF scene (furniture moved, doors, other
  people in neighbouring zones) in a per-session text note.

## Recording

Start the sensing server with an explicit hardware source. On a host with its
own WiFi adapter, `--source auto` never binds the CSI receiver:

```bash
cd v2 && cargo run -p wifi-densepose-sensing-server -- \
  --udp-port 5005 --udp-bind 0.0.0.0 --source esp32
```

Per zone, in parallel:

```bash
# Radar reference: windowed labels plus the raw log for later re-windowing
python scripts/radar_window_logger.py --port COM4 --room office --zone desk \
  --duration-s 900 --out sessions/office-s1-radar-desk.jsonl \
  --raw-log sessions/office-s1-radar-desk.log

# CSI predictions (one recorder per room; map every node to its zone)
python scripts/csi_presence_recorder.py --room office \
  --node-zone 1=desk --node-zone 2=sofa \
  --duration-s 900 --out sessions/office-s1-csi.jsonl
```

### Clock offset

`--clock-offset-ms` (radar clock minus CSI clock) is a required argument.
With both recorders on one host, it reflects serial and server processing
latency, not clock skew. Measure it once per setup:

1. With the zone empty, walk in at a noted moment, stand still for 30 s, and walk out.
   Repeat 5 times.
2. For each entry, find the empty→present edge in both the radar and the CSI
   rows. The offset is the median of radar time minus CSI time.
3. Record it in the session note and use that value, rounded to 100 ms, for
   every session of that setup.

## Scoring

```bash
python scripts/presence_benchmark.py \
  --radar sessions/*-radar-*.jsonl --predictions sessions/*-csi.jsonl \
  --clock-offset-ms <measured> --out sessions/report.json
```

For each room, the decision threshold is chosen on the other rooms only, then
applied to the held-out room. Radar windows that are stale, unverified or
missing a presence value are dropped and counted in `dropped`. They are never
treated as "empty". Windows with fewer than three CSI predictions are also
dropped.

## Reporting a result

A result may be stated as MEASURED only when **all** of these hold:

- the report's `evidence` field is `MEASURED`, with no synthetic predictions;
- at least 3 rooms, with every room present as a held-out fold;
- for every fold, the model's F1 and accuracy are published next to the
  majority baseline. The baseline's accuracy, not 50 %, is the bar;
- the dropped-window counts are published, and each room keeps at least
  70 % of its windows;
- the session notes, the measured clock offset and the commands are published
  so the run can be reproduced. Raw CSI and radar logs are person data: keep
  them out of the repository (ADR-299) and share them only by agreement.

Report per-fold numbers, not only the pooled ones: a pooled score can hide a
room where the model is worse than the baseline. If the model does not beat
the majority baseline in a held-out room, say so plainly for that room.

The radar is a reference, not ground truth. Its presence accuracy is a
datasheet figure (CLAIMED). Spot-check at least one session per room against
a written log of entries and exits. If more than 5 % of spot-checked windows
disagree, report that next to the result.
