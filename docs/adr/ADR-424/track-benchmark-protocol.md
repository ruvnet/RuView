# Track-aware benchmark: per-person labels, held-out rooms, ambiguity class

Protocol for ADR-424 D7. It measures how many people RuView reports and
whether it keeps each person's track, against a reference that tracks people
individually. It is **separate** from the presence benchmark
(`docs/adr/ADR-423/presence-benchmark-protocol.md`): a presence result must
not be quoted as a counting or tracking result, or the reverse.

**Scope.** Count, per-person detection, track continuity and position within
a room. Track ids are session-local and pseudonymous. This protocol supports
no claim about identity (who a person is), pose or activity.

No run under this protocol has been done. It makes no performance claim.

## Reference

The reference must track people individually and give positions. Suitable:

| Reference | Gives | Limits to record |
|---|---|---|
| Tracking radar (LD2450-class) | Up to 3 moving targets, x/y | Loses people who are still; fixed target limit |
| Two or more such radars with overlapping coverage | Cross-checked count | Needs a shared coordinate frame |
| Camera with a person tracker, used only as a labelling reference | Positions and tracks | Person data: keep out of the repository (ADR-299) |

Record the reference's stated resolution (the distance below which it merges
two people). That number is `--min-separation-m`. Using a smaller value than
the reference supports turns reference errors into "clear" labels.

When more than one reference source covers a room, write every count they
reported into `count_candidates`. Do not pick one beforehand.

## Window classes

Every reference window gets exactly one class:

- **unusable**: the reference is stale, unverified, simulated or malformed.
  Dropped, and counted by reason.
- **ambiguous**: the reference is healthy but cannot resolve who or how many:
  - its sources disagree on the count (`count_disagreement`);
  - two reference tracks are closer than the reference's resolution
    (`tracks_too_close`);
  - a track appeared, disappeared or changed id in this window or within
    `--settle-windows` of it (`track_change`);
  - a reference position is missing (`missing_position`).
- **clear**: everything else. Only clear windows are scored.

Ambiguous windows are a result, not noise to remove. The report gives their
count, share and reasons per room. They are never forced into a count or a
track and never silently dropped. People entering, leaving, crossing and
standing together are exactly where a system fails, so a run whose ambiguous
share is high has only been tested on easy windows and must say so.

## Rooms and sessions

- At least **3 rooms**; each is scored as a held-out fold.
- Per room, sessions with 1, 2 and 3 people, at least 10 minutes each,
  including: people still, people crossing paths, people standing close
  together, entries and exits. Record the planned script per session.
- Mark the reference's coordinate origin and axes in the room and record them,
  so predictions and reference share a frame.
- Measure the clock offset as in the presence protocol and pass it explicitly.

## Scoring

```bash
python scripts/track_benchmark.py \
  --reference sessions/*-reference.jsonl --predictions sessions/*-tracks.jsonl \
  --clock-offset-ms <measured> --min-separation-m <reference resolution> \
  --out sessions/track-report.json
```

For each held-out room, the match radius and the majority-count baseline are
fitted on the other rooms only. On the room's clear windows the scorer reports:

- **count**: accuracy and mean absolute error, next to the majority-count
  baseline's accuracy;
- **detection**: precision, recall and F1 from one-to-one matching of
  predicted to reference persons within the match radius;
- **tracking**: identity switches (a reference person's matched predicted id
  changes) and fragmentations (a person is lost and found again);
- **position**: mean error of matched pairs, metres;
- **classes**: clear, ambiguous (share and reasons), and clear windows that had
  no prediction.

## Reporting a result

A result may be stated as MEASURED only when all of these hold:

- the report's `evidence` is `MEASURED` (no synthetic predictions);
- at least 3 rooms, every room a held-out fold, per-fold numbers published;
- count accuracy is published next to the majority-count baseline;
- the ambiguous share and its reasons are published per room, together with
  the `--min-separation-m` used and where that number comes from;
- clear windows without a prediction are published (a system that goes silent
  on hard windows must not score better for it);
- the reference's own limits (target cap, loss of still people) are stated.

If count accuracy does not beat the majority-count baseline in a held-out
room, say so for that room. State the largest person count actually tested;
the result says nothing about larger groups.
