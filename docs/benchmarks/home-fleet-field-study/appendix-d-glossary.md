# Appendix D. Metric glossary, truth supply and detectable effect

Conventions: "min" = minutes of a 1-minute grid; "h" = hours; "per node" = one node's own series; "house" = fleet-level (median or vote over nodes); "window" = one owner-confirmed animals-only or empty interval. "(derived)" = computed from recorded numbers.

## 1. Metric glossary

### 1.1 Detection (rate), false presence, false AWAY

- **Detection** (R13 family: L009, L011, L012, L015, L016, L018, L019, L074). Share of waking (07:00-23:00 local) minutes labelled HOME that the method called present, per local day; bar term "median detection >= 0.70" is the median over days that have >= 60 HOME minutes; R9 runner   applies the same >= 60 rule). Units: fraction of minutes, per day, then median across days (not pooled). House level unless a node is named. Minutes with no decision are counted absent (L010: 51% of animals-only minutes undecided "counted absent" made masked false presence look 4-7%, "NOT real").
- **False presence** (same family). Share of confirmed animals-only minutes the method called present: per day, and pooled over the scored days. Bar: pooled <= 0.05 (R13/R9/R24). Denominator is minutes inside owner-confirmed windows, so a handful of long windows dominate (section 2). The first ~30 min after a departure are included (L018: excluding them moves R24 pooled false presence 0.192 -> 0.147).
- **Where other tests used "false presence" differently.**
  - L006 and L005: share of sink log lines / 30-min slices reporting presence (99.95% of 6,101 lines; 0.9978 of 897), not minutes of a bar-scored method.
  - L007, L067: per-node fire rate in AWAY vs HOME minutes (AWAY includes unconfirmed AWAY labels; L067 notes AWAY is "all daytime").
  - L068: share of minutes where the score exceeded its threshold, by EMPTY / AWAKE / ASLEEP class (40% / 51% / 63%).
  - L015, L016, L019: "FP gain" = FP(variant) minus FP(C3 baseline), bar <= +0.05, per day (L015/L016 decision days) or pooled (L019: +0.057).
  - L024: share of occupied HOURS called empty by whole-house power (83 of 374, 22%), hour units not minutes.
  - L012: false presence at the live label's own detection rate (100%) is ~95-100%, i.e. conditional on matched detection.
  - R16/L074 "FP animals-only S.53-.91" are per day; the primary 47% is pooled over 425 confirmed-empty minutes.
- **False AWAY** (phone rule, L020, L148, L153). Share of known-HOME minutes the rule called empty (the mirror of false presence). Sealed bars: <= 2% known-home minutes, longest false stretch <= 10 min (L020). Denominator was 764 HOME minutes in the forward test and 1,775 in the 16-window study. Not comparable with false presence: different denominator class (HOME minutes, not empty minutes), different truth.
- **Catch / caught, phone rule** (L020). Share of empty minutes the rule called empty (52.2% H1, 50.8% H2 forward). This is detection of EMPTINESS: it is a recall on empty truth, the opposite sign convention to R13 "detection" (recall on HOME).

### 1.2 AUC: waking, pooled, per-day, and the other AUCs

- **Waking AUC** (L008-L012, L074, L148). Per held-out local day, P(a HOME minute scores above an AWAY minute) over labelled minutes 07:00-23:00 local; 0.5 = chance. Labels are HOME vs AWAY (AWAY is the phone-derived or confirmed label, so it contains more than confirmed animals-only minutes). Reported per day, then median over days. "Masked" = after the R14 RF-quality mask removed fleet-excluded node-minutes (L010); unmasked and masked AUCs are different quantities and the ledger quotes both.
- **Per-day AUC.** One AUC per held-out day (L009 four days; L074 four dev days, three primary days; L075 per-day P 0.37-0.83). Sensitive to day offsets: L008 shows cross-day AUCs of 0.00 and 0.96 in opposite directions.
- **Pooled AUC.** One AUC over all out-of-fold scores of all days together, L075: P 0.595, F 0.618, T 0.261, H 0.729; L025: raw power AUC 0.75 pooled over 18 days with a day-bootstrap CI). Pooling lets between-day (time-of-day) differences count as signal, which is exactly why L075's H (clock only) reached 0.729 and voided the design. Do not compare a pooled AUC with a per-day median.
- **Direction.** L025/L024 `auc` returns P(score_pos < score_neg) (lower usage = empty; L075 AUC is for the E-vs-O labels (E = animals only, O = person present, so >0.5 = separates O); R13 AUC is HOME above AWAY. L075's T = 0.261 is therefore a strong INVERSE separation, not weakness.
- **S, the screen statistic** (L021-L023, L037). Not an AUC: S = mean(d)/sd(d) x sqrt(k) over k windows (or hour cells for rooms), d = window mean minus the median of the same clock span on other days,. It is a one-sample t-statistic on the window differences.

### 1.3 Nulls: family-wise, shuffled-label, block shuffle, circular shift

- **Circular day-shift null** (L001, L021-L023, L037). The empty-window set is moved by j = 1..N-1 days within the dev-day list; windows landing on a real window are dropped; shift skipped if < 8 windows remain. For each shift the maximum |S| over the whole family is kept.
- **Family-wise null / null max** (L021-L023, L037). The maximum over shifts of the per-shift maximum |S| over all (feature, node) tests (L023: 45 tests; L022: 38 testable of 40; L021: 72 tests). A test "survives" only if its observed |S| exceeds that maximum. Recorded nulls: L021 6.86 (next 4.32; one shift), L022 3.44 (next 2.72), L023 4.37, L037 room 5: 5.80, rooms 5;6: 11.29. Because the threshold is the max of ~14 shifts, the achievable p-value is floored at 1/(1+shifts) = 0.0625 (`rank_info`,; ledger "rank_p floor 0.0625"). A single test could never reach p < 0.05.
- **Circular-shift null, row-174 test** (L001). 1000 circular shifts of event times; pass threshold "null p99.75" (Bonferroni over 4 statistics); L001 reports 519 vs 561.0.
- **Shuffled-label null (supervised nets)** (L074). Same training pipeline with labels permuted, 5 seeds; a "shuffled" AUC is the control model's AUC. L074: shuffled median waking AUC.56 unmasked /.51 masked; per-seed-day shuffled AUC spans 0.10-0.94.
- **Block shuffle null** (L075). Within each day, labels are permuted among 10-minute blocks (blocks mapped onto each other by position, cycled when lengths differ); 100 nulls; `null_p99` is the 99th percentile of the 100 null AUCs. With 100 draws p99 is the interpolation between the two largest values, so it is a coarse bound. The block shuffle keeps the within-day base rate and the 10-minute autocorrelation but destroys hour-of-day ordering of labels across days only partly; H (clock only) beating it at 0.729 is what the INVALID-DESIGN rule caught.
- **Same-hour random control (room tests, see 1.5)** is a control for time of day, not a null distribution.

### 1.4 Catch, catch rate, false events per nobody-there hour (H3 and blind test)

- **Catch** (L035, L038, L039). A truth moment is caught if the node's rule has a qualifying event within +-2 min. Catch rate = caught / own-room truth moments. A run of qualifying minutes is ONE event; a new event needs a 5-minute gap. Different from L020 "caught" (share of empty minutes called empty) and from L003 "plate caught 1 of 4 transitions".
- **Base / same-hour control** (H3): the same +-2 min test at every other minute of the same hours of day on the scored days, excluding +-10 min around any truth moment. Blind test: base = every minute of the local (date, hour) slots that hold a graded moment, same exclusion. Different slot sets, so a H3 base (0.013 for node 5) and a blind base (0.0042; dev 0.034) are not the same construct.
- **Lift** has two meanings in the ledger:
  1. Difference: lift = catch - base (H3, node 5 lift 0.006; positions: lift = rate - control).
  2. Ratio: L038 "lift 5.2" = 0.176 / 0.034.
  Two meanings, so never quote a lift without saying which.
- **False events per nobody-there hour** (H3). Events per hour counted in minutes when nobody can be in that room. Hours come from two sources, which is why one test week shows two denominators: confirmed animals-only windows (every node; 9.2 h = the five test-week windows, derived from the label file: 552 min) plus, for common-room nodes only, night-time hours while the house is HOME (27.9 h; 9.2 + 27.9 = 37.1 h). The night hours rest on an ASSUMPTION that sleepers stay in bedrooms, not on confirmation. Bar: <= 0.1 per nobody-there hour, with >= 4 h (H3 seal) or >= 10 h (blind test; under 10 h = NOT PROVEN,. Node 4 0.217/h over 9.2 h and node 7 0.162/h over 37.1 h failed.
- **Firing floor** (L035). Node must fire >= 0.05 events/h in the test week to be graded at all; added by Option C so that a rule that never fires cannot pass. Node 0 passed at 0.065/h vs floor, node 3 at 0.059/h.

### 1.5 Lift and same-hour control, positions

- **Room-moment hit rate** (L046, L049, L050). Share of truth minutes whose STRONGEST blob within +-2 min lies inside the room's rectangle, with missing data and "no blob" counted as failures in the headline rate (answered-only printed beside it).
- **Control.** The same test at random minutes of the same local hour on the estimator's days, away from every truth moment and known-empty windows (`--control-k 50`, seed 1). **Room-moment lift** = hit rate - control rate (difference form). Recorded: L049 +0.12 (n=31) / +0.02 (n=76) / +0.04 (n=107); L050 0.197 OFF = 0.197 ON.
- Same-hour control does not remove room bias: truth is ~90% one room (L050: 90/99, 49/60), so an estimator that parks its strongest blob in that room scores high hit rate and the control can still sit lower; GAPS/memory "est-rti lift = room-bias artifact".

### 1.6 Paired fraction, n2n fps

- **Paired fraction** (L085, L090, L091). paired_fraction = paired transmissions / finalised transmissions over a window, where paired = transmissions captured by two or more nodes (`v2/.../fusion.rs:147-161`; windowed form dPaired/dTransmissions,, 20-min windows). It is a share of transmissions, not of frames and not per second. L091: later judged the wrong figure of merit; adoption judged on paired observations per second (fraction rose 0.5752 -> 0.7287 while paired/s fell 44%).
- **n2n fps** (L154,. Frames per covered second for node-to-node ordered (receiver, transmitter) pairs; covered = union of [first, last] frame time inside the window, capture holes not counted as silence. The headline "n2n fps 68-100 (median 81)" is read here as the fleet-wide total over ordered pairs (inferred from magnitude: ~1.1 fps over 72 ordered pairs); it is NOT a per-link rate. Companion figure "pairs >= 2 fps" counts ordered pairs. Window: night hours, healthy nights only.

### 1.7 Precision-only claim

(L035, L036,. Option C: a node may claim only "quiet when nobody is in the room": PASS = false events/h <= bar AND false-active share <= bar AND nobody-there hours >= minimum AND events/h >= firing floor 0.05; catch metrics computed and printed REPORT-ONLY, never graded; nodes 1, 5, 6 excluded at seal. It was chosen because the data could not show catches (SEAL-OPTIONS: "7 of 9 rooms have no truth"). Consequence recorded in L035/L126: nodes 0 and 3 passed "by never firing; it caught nothing". The claim contains no sensitivity statement.

### 1.8 Verdict words as used

| Word | Used as | Where it is NOT consistent |
|---|---|---|
| PASS | Every sealed bar term met by the sealed scorer. L020 phone rule PASS (non-RF); L012 PASS "vs the live label" only; L028 PASS "on the letter" with 36/36 saturation; L035 PASS precision only. | L012 and L028 are PASS against a weak or saturated bar; read with the qualifier. |
| FAIL | At least one bar term missed on scored data. | L033 H3 preview recorded as NOT-PROVEN in one source and FAIL in another for the same numbers; L060 NOT-PROVEN vs FAIL; L048 NOT-PROVEN vs FAIL (GAPS section 2). |
| VOID | The result is discarded because a pre-condition broke: L015/L016 decision days (reproduction gate mismatched on 2 of 3 days); L153 (premise false: phone windows = the hand list); L143 rule VOID as a signal; L005 upstream PRs VOID. | Not a failure of the method; also not a pass. |
| INVALID-DESIGN | The test could not have answered its question: L075 (clock-only readout H = 0.729 >= 0.65,; L152 (effective n = windows); L155 (truth supply); L127; L148 (phone labels alone). | L075 also failed its own null, so it is INVALID-DESIGN and FAIL; the script prints only INVALID-DESIGN when H >= 0.65. |
| NOT-PROVEN | Evidence points no way, or the test was under the sealed evidence floor (blind week: 3.03 h of nobody-home < 10 h; 70.5 of 168 h covered). | Used both for "too little data" (L039, L043, L045) and "ambiguous/post hoc" (L060). |
| NOT TESTABLE | Blind scorer only: < 20 graded moments. | -- |

## 2. Truth-supply table

All counts are of the CURRENT state of the label tree unless a ledger ID is cited; where I recomputed from the label file I say so. Windows and day counts only; no times.

### 2.1 Sources and what they label

| Source | Labels | Count available | Used by | Known defects (ledger) |
|---|---|---|---|---|
| Owner-confirmed animals-only (empty-house) windows | Nobody home, [pet] home. Negative truth for house presence and for every node's room-activity "nobody-there". | Through the dev cut: 16 windows, 2,630 min (43.8 h), on 13 days (L126, L153; recomputed from the file). 15 of them are dev windows (the 16th falls on a capture-gap day dropped from dev: dev = 10-01..10-20 minus two gap days,; 15 windows = 2,569 min; 3 are holdout (L152). Median window 87.5 min; 12 of 16 >= 60 min; the two longest are 632 and 650 min = 49% of all minutes (recomputed, derived). Whole file today: 34 windows, 4,383 min, 26 days. | R9, R13-R24 (FP), L021-L023, L035, L038, L074, L075 | Only one house; pets always home so "empty" means people-empty. Windows are owner-logged outings, so they are not random draws from the day (L075: E and O fall at different hours). L153: phone-rule AWAY runs add no new windows (premise false). |
| Phone Wi-Fi/BLE rule | HOME/AWAY per minute from resident phones. | Study n=16 windows; forward test 9 of 14 labelled days, 13 empty windows (1,434 empty min), 764 HOME min (L020). | L020; labels for R13-R24 AWAY and HOME minutes | L148: one 110-min stretch labelled AWAY while a person was home (phone off Wi-Fi) taught "quiet = empty"; supervised edge largely this (R16 S.68 ->.47). Days with an additional occupant: 8 of 14. Rule rejected as a product (L188). |
| Plate reads (camera system) | Car arrive/depart events. | 79 events on 22 days; 33 Wi-Fi car sessions >= 20 min; plate arrival within 5 min on 19; plate missed 14; departures 8 (L003). Second car: 2 usable pairs. | Arrive/depart context only | Dusk/dark misses, false arrive/depart pair, one car under two labels; departures weak. |
| home-automation database appliance / switch moments | Own-room events: appliance power steps, switch presses. | H3 test week (7 local days): 132 moments (per node 107/51/22/3, overlapping, see 2.2). Dev: 386 moments (node 5: 330, node 6: 80, node 8: 41, node 3: 9, node 4: 6; also overlapping; L038). Screen A: room 5 38 events, rooms 5;6 14, room 8 14 (L037). Positions: 31 (10-17 span, other sources 32/35/36), 76, 107, 101 / 146 (L049), OFF 102 (99 with data), ON 60 (L050). Blind week graded 99 (+ 40 shared-appliance moments excluded, 139 total, L039). | H3 catch, L038/L039, positions | Truth exists only where the house has instrumented loads: 7 of 9 rooms have none in the test week (L155). Node-6 moments are all one appliance shared with node 5 (51 of 183, derived overlap). That appliance is a 2.4 GHz emitter, so it perturbs CSI by itself (L038/L039: excluded from grading, reported). One node carried 56% of analyst-rule events on one link. ~90% of position truth is one room (L050). |
| Wall-keypad presses | A person is present and active (pets cannot press). | O = 495 min in the raw probe (2 min before, 5 after, 30-min exclusion; at most ~70 distinct press windows, derived 495/7) (L075). Also folded into home-automation moments for positions. | L075 | Presses cluster at different hours from the animals-only windows (H = 0.73 from clock alone). |
| Smartwatch night | Per-minute heart rate of a sleeper: asleep/awake. | One subject, one night (L063). | L063 | N = 1; link crosses only the foot half of the bed (plausible, not proven). |
| Walk marks (prompted walks) | Known spots at known times. | Survey 5 known spots (L041); 10 spots x 2 passes = 16 spot-passes scored (L030); L045: n=11 of 28 marks; L043 survey re-analysis. | L030, L041, L043, L045, L046 walk | Owner rule: no prompted labelling shipped (L164), so any design needing walks is outside the constraint frame. Few passes per link (L030: ~4 bins per link per 40 s spot). One room, three sensors in the original survey (memory: doesn't transfer). |
| Labelled live windows | Approximate "who is where" snapshots (e.g. 3 humans one asleep + animal). | A handful, never scored (L026). | -- | OPEN: never checked at the time. |
| Whole-house power | Not truth: a competing signal; hour kWh vs same hour other days. | 31-33 empty / 374 occupied hours over 18 days (L025). | L024, L025 | Quiet babysitting evening looks empty (L024). Uses other days' labels for the ratio (mild hint). |

### 2.2 The per-test truth counts the tests actually got

| Test (ledger) | Truth units | Counts | Held-out vs dev |
|---|---|---|---|
| R13 A/B, L009 | animals-only minutes, HOME/AWAY minutes | 113 archived hours; 6,459 feature minutes: HOME 4,628, AWAY 1,592, animals-only 1,307; false-presence denominators for the three scored days n = 60 (day 2) / 27 (day 3) / 603 (day 4) (L012, L015) | Leave-one-day-out over 4 days (10-01..04); all autocorrelated pre-link-fix days |
| R9 week, L019 | same, 7 days | S pooled false presence over 685 min; recomputed current file: 8 windows, 770 min on 6 of the 7 days for that span | Held-out week (sealed) |
| R17/R18/R22/R23 primary, L011, L015, L016 | confirmed-empty minutes | 425 minutes (three decision days; ~6 windows, ~469 min in the current file, derived) | Decision days 10-08, 09, 11 post-fix |
| R24, L018 | same | 605 pooled minutes; 120 pcaps; 10,080 minutes in the week, 552 undecided; asleep-tagged HOME 2,404 | Held-out week 10-08..10-14 (same days as R9) |
| Phone rule, L020 | HOME minutes, empty minutes | 764 HOME / 1,434 empty min forward; 13 windows | Forward test (sealed 10-19 for 10-20..11-02) |
| Screens, L021-L023 | empty windows | 15 windows (dev), 15 day shifts; 17 eligible links (>= 10 windows each), 554 pcaps, 458 hourly feature files | Holdout (3 windows) excluded by filename |
| Screen A rooms, L037 | event minutes | 38 / 14 / 14, 4 shifts | dev only |
| H3 sealed test, L035 | nobody-there hours; truth moments | 37.1 h (nodes 0, 5, 6, 7), 9.2 h (nodes 1, 3, 4, 8), 2.9 h (node 2); moments 132 total; per node 107/51/22/3 (node 5/6/8/3), others 0. 107 + 51 + 22 + 3 = 183 > 132 because a moment is counted for each of its nodes; 183 - 132 = 51 = the node-6 count, so every node-6 moment is also a node-5 moment (derived; inferred from   and arithmetic, GAPS 1b) | Sealed test week 10-21..10-27; 132 moments are the held-out supply |
| Blind analyst, L038 | moments; nobody-home hours | 386 dev moments (overlapping per-node counts 330/80/41/9/6); 41.0 nobody-home h on the analyst's labels, 10.9 h in held-out windows after its label cut; node 5 graded 330 (58 catches from 30 events) | Dev (analyst), held-out windows, then sealed week |
| Blind week, L039 | graded moments, nobody-home hours | 99 graded + 40 shared-appliance; 3.03 nobody-home h; only 70.5 of 168 h covered | Sealed week 11-03..11-09 |
| Positions after tuning, L049 | room moments | 107 (n 31 / 76), re-score 101 new days / 146 all | Days after the 10-17 tuning |
| Positions bypass, L050 | room moments | OFF 102 (99 with data), ON 60 | A/B days 11-09..14 vs 11-17..19 |
| Sleep night, L063 | per-minute watch HR | 1 night, 1 subject | none |
| Desk study, L064 | breaker classes | test: DESK 744 / AWAY 347 / OFF 602 min | dev 10-01..07, test 10-08..13 (scored once) |
| Raw probe, L075 | E and O minutes | 2,177 min = E 1,682 + O 495, on 13 of 16 days; label set 21 windows, 16 days, 2,060 E min before decode losses (one capture hour unreadable, skipped) | Leave-one-day-out over 13 days |
| Net, L074 | HOME/AWAY | same as R13 plus primary days | LODO 4 days; primary 3 days |

Count mismatch to remember: 16 empty windows on 13 days (dev cut, 2,630 min) vs 21 windows on 16 days (2,060 min) for the raw-probe label set (a later, shorter-window cut up to 11-08, trimmed 5 min each end). They are different cuts of the same label file, not conflicting numbers.

## 3. Detectable effect per major test

Method notes. (a) Where the record gives a null or a bar, I use it. (b) Binomial thresholds are one-sided exact: smallest k with P(X >= k | n, p0) <= alpha, computed with the exact sum (derived). (c) Minute counts overstate independent information: minutes inside a window are strongly autocorrelated (L152: "effective sample is 15 empty windows, not hours"). Where I use windows as units I say so.

### L009 R13 two amplitude features, LODO 4 days
- Bar: detection >= 0.70 at false presence <= 0.05 pooled. Truth: false-presence denominators 60 / 27 / 603 minutes in 3-5 windows (current file: 5 windows on the four days).
- Zero-failure bound at 5% (derived): to show FP <= 0.05 with 95% confidence from zero false minutes you need n >= ln(0.05)/ln(0.95) = 59 independent minutes. The 60-min and 27-min days cannot, even if minutes were independent; with windows as units, 5 windows allow at best 1 - 0.05^(1/5) = 45% as an upper bound on window-level FP (derived).
- AUC resolution (recorded): per-day waking AUC ranged.14 to.78 for the SAME features across four days (L009), and shuffled controls reach 0.10-0.94 per seed-day (L074). Any true AUC edge below ~0.3 is not distinguishable from day-to-day swing on this supply. The test could in principle only detect a feature that held >= 0.7 on every day, which is the bar.
- Underpowered by its own record: L009 stated cause (features unstable across days); L074 "S edge over shuffled (~0.05-0.10) is inside the spread of four autocorrelated days"; L010 "bar days pre-link-fix and autocorrelated, so further tuning is forking paths".

### L011 R18 clean upper tones; L018 R24 legacy frames; L019 R9 presence_v2
- Bars: detection median >= 0.70, pooled FP <= 0.05 (R9 definitions) over the held-out week. Supply: 685 pooled FP minutes (L019), 605 (L018), 425 on the three primary days (L011); about 6-8 windows in each.
- Naive minute-level 95% half-width on a pooled false-presence rate p over n minutes = 1.96 x sqrt(p(1-p)/n) (derived). At p = 0.63, n = 685: +-0.036. At p = 0.192, n = 605: +-0.032. These are far below the 0.05-vs-0.19 gap, so the verdicts FAIL decisively even after inflating the variance by the design effect (with ~8 windows the effective n is ~8, giving +-0.33 at p = 0.63 (derived)); the failures are real, but a PASS at 5% could not have been certified: zero false minutes in 8 windows bounds window-level FP at 1 - 0.05^(1/8) = 31% (derived).
- Detection median over 7 days with 7 values: the bar 0.70 is detectable (R24 0.601, R9 S 0.62, C3 0.36) because the failures are 0.08-0.34 below bar.
- Underpowered by record: L018 failure mode only in the first ~30 min after departures; undecided 552/10,080 minutes; L019 P(night): node 3 alone above threshold in 0 of 2,478 asleep minutes (a null result on a descriptive check).

### L021-L023 family-wise feature screens (house level)
- Supply: k <= 15 empty windows (dev), 15 shifts. Statistic S = mean(d)/sd(d) x sqrt(k). Recorded null maxima: L021 6.86, L022 3.44, L023 4.37.
- Smallest detectable effect size (derived): d' = mean/sd of the window-level difference needed to exceed the null max M is d' = M / sqrt(k). For k = 15: L023 4.37/3.873 = 1.13 SD; L022 0.89 SD; L021 1.77 SD. So a feature had to shift its mean by about one full standard deviation of its own window-to-window spread to survive, in at least ~11 of 15 windows (best observed 11-12 of 15 windows up; S 3.22 / 3.17 / 2.41 fell short).
- Resolution of the null itself: with 15 shifts the lowest attainable p is 1/16 = 0.0625 (L023 "best rank 1/16 (floor)"). A perfect signal could not reach p < 0.05 on any single test (derived from the record). Observed 9 of 72 and 7 of ~38 tests sit at that floor vs ~4.5 and ~2.4 expected by chance.
- Calibration weakened: L022 shifted-window control gave 2 survivors where 0 expected (node 2 rssi 3.62, gain -3.35), exceeding the observed best of 2.41; L021 controls clean.
- Underpowered by own record: L023 "15 windows is why a real effect cannot be told from luck"; L022 thin 15-shift null "probably weak for autocorrelated series"; L152 effective sample = windows. The three screens together ran 155 tests on 15 units.

### L035 H3 precision-only test (sealed week)
- False-event bar: <= 0.1 events per nobody-there hour. Probability of seeing ZERO false events when the true rate is exactly at the bar, P0 = exp(-0.1 x h) (Poisson, derived): 9.2 h -> 0.40; 37.1 h -> 0.024; 2.9 h -> 0.75 (node 2 was labelled UNTESTED, consistent). So a 9.2-h pass (node 3) is reached 40% of the time by a rule sitting exactly at the bar; a rule has to be tested over at least 3/0.1 = 30 h to have <= 5% chance of passing at the bar with zero events (derived). Nodes 0 (37.1 h, mostly assumed-nobody night hours) is the only adequate precision test, and its 28 h of nights are assumed, not confirmed.
- Firing floor: 0.05 events/h means a node expecting ~8 events in 168 h; node 0 fired 0.065/h and node 3 0.059/h, which is barely above the floor (a node can sit near the floor and be NO-CLAIM or graded on small count noise alone). The precision claim is therefore nearly vacuous: false events 0 with fires 0.065/h.
- Catch bars were report-only, but if they had been graded: smallest catch count to beat the H3 base at alpha = 0.01 (derived, exact binomial): node 5 n=107, base 0.013 -> k >= 6 (5.6%); node 6 n=51, base 0.011 -> k >= 4 (7.8%); node 8 n=22, base ~0.01 -> k >= 3 (13.6%); node 3 n=3 -> only 3/3. Observed: node 5 2/107, node 6 1/51, node 8 0/22, node 3 0/3. The shortfall is 4 catches at node 5 (and node-6 moments overlap node 5, so they are not independent). With "catches report-only" the sealed bar could not fail on sensitivity at all.
- Underpowered by own record: the sealing-options document (A/B/C) ("7 of 9 rooms have no truth... a catch bar would fail for lack of evidence, not because the rule is wrong"); STRATEGY 10-29 "passed by never firing".

### L038 / L039 blind analyst and sealed confirmation week
- Dev (L038): node 5 rule catch 0.176 (58/330) vs base 0.034; family-wise null max 0.067 vs unshifted 0.142 (beats all 14 shifts over 105 node-5 looks, p = 1/15 = 0.067 floor). Smallest catch rate that beats the family-wise null = 0.067 (recorded). 521 looks over 16 families; the null is over the 105 node-5 looks only. Binomial detectable catch at the dev base 0.034, n = 330, alpha = 0.01: k >= 21 (6.4%) (derived); effective n is ~30 events not 330 (58 catches from 30 events; 56% of events on one link), so 330 overstates information by ~11x (derived 330/30).
- Sealed week (L039): alpha = 0.01, graded n = 99, base 0.0042: k >= 3 catches (3.0%) would have passed (derived); observed 0/99. The week could not fail on lack of sensitivity: 0 catches against a 3-catch threshold means the rule had effectively zero response, consistent with the record "zero catches in 70 h is not a sample-size miss". The false-alarm guard needed >= 10 nobody-home hours and got 3.03 (P0 at the bar = exp(-0.1 x 3.03) = 0.74, derived), so the guard was untestable; coverage was only 70.5 of 168 h.
- Held-out empty windows after the analyst's label cut: 5 false events in 10.9 h = 0.46/h (four on one link) vs the analyst's 1 in 41 h (0.024/h): the dev false-alarm claim was an artifact of the labels the analyst saw.
- Underpowered by record: L038 caveats (58 catches from 30 events; only 12.4 nobody-home h post-fix); L039 3.03 h < 10 h; L155.

### L049 / L050 position, est-rti-v1
- Metric: room-moment hit rate vs same-hour control; lift = difference. Supply: n = 31 (+0.12), 76 (+0.02), 107 (+0.04), 146 (+0.05); OFF 99 with data, ON 60.
- L049: minimum hits to beat control at alpha = 0.05, exact binomial (derived). n = 31, control 0.07: k >= 6 (19.4%): observed 6 = exactly the threshold (the +0.12 "lift" at n = 31 was borderline, then vanished at n = 76). n = 107, control 0.07: k >= 13 (12.1%): observed 13 hits sits at the threshold. So the smallest detectable lift at n = 107 is about +0.05 above control, and the estimator earned +0.04: it was not distinguishable from chance but the test would have caught a lift of ~0.05 if it had been real. BUT every hit was one room (13/83; others 0/24): the effective information is the single-room rate, not 107 independent localisations.
- L050 bar written beforehand: ON lift >= OFF lift + 0.10. Standard error of the difference of two proportions: sqrt(0.253 x 0.747/99 + 0.333 x 0.667/60) = 0.075 (derived); one-sided 95% detectable difference = 1.645 x 0.075 = 0.123, and 80% power needs (1.645 + 0.84) x 0.075 = 0.19 (derived). So the +0.10 bar demands an effect the experiment would detect less than half the time (power at +0.10: P(Z > (0.123 - 0.10)/0.075) = 0.38 (derived)). The observed difference was 0.000 (lift 0.197 OFF = 0.197 ON), so the null result stands on its own, but a real +0.10 improvement would likely have been missed.
- Truth is ~90% one room (90/99, 49/60), so lift is dominated by one room's bias; empty-house false blobs came from one short window each ("not interpretable").
- Underpowered by record: L049 "10-17 lift (+0.12, n=31) was small-sample"; L050 empty-house comparison "one short window each".

### L063 sleeping person with watch HR
- Supply: 1 night, 1 subject. Spearman(motion, HR) = -0.07; respiration windows 9% asleep vs 15% awake.
- Effective sample (derived, assumption): consecutive minutes are autocorrelated; a night holds ~4-6 sleep cycles, so n_eff is on the order of 5-10. For n = 10, |rho| must exceed 0.64 for p < 0.05 two-sided (derived, Spearman critical value). So only a very strong, monotone effect could have been detected; -0.07 is nowhere near that. The test could also not separate "node blind" from "geometry blind" (link crosses only the foot half of the bed).
- Underpowered by record: N = 1; no replication (L063 stated cause "plausible, not proven").

### L074 R16 neural net vs baselines
- Supply: LODO over 4 days; primary 3 days, 425 confirmed-empty minutes; 5 seeds. Shuffled per-seed-day AUC 0.10-0.94 (recorded). With a null spread that wide, the minimum AUC edge detectable at the per-day level is >= 0.4 above 0.5, i.e. a net with true AUC >= ~0.9 on every day (derived from the recorded spread). The observed edge (~0.05-0.10 supervised, 0.67/0.69/0.58 primary) is inside it; after label correction S fell to.47-.53 (L148).
- Bar (R13) needs 0.70 detection at 0.05 FP; the primary result 0.61 detection at 0.47 FP failed it by a wide margin, so a FAIL is secure; a modest true improvement would not have been detected.
- Underpowered by record: L074 "S edge over shuffled is inside the spread of four autocorrelated days"; R15 best-of-18 configs = forking paths.

### L075 raw CSI random-convolution probe
- Supply: 2,177 min (E 1,682 / O 495) on 13 days. Bar terms: P >= 0.75, P above block-shuffle null p99, P >= F + 0.05, P >= T + 0.05, H < 0.65. Recorded: P 0.595, F 0.618, T 0.261, H 0.729, P null p99 0.622.
- If minutes were independent, null AUC SE = sqrt((n1 + n0 + 1)/(12 n1 n0)) = sqrt(2178/(12 x 495 x 1682)) = 0.0148, null p99 would be 0.5 + 2.326 x 0.0148 = 0.534 (derived). The recorded block-shuffle p99 of 0.622 implies SE 0.052, a variance inflation of (0.052/0.0148)^2 = 12.6 (derived), i.e. an effective sample of about 175 minutes, or on the order of 1/12 of nominal. So the AUC that beat the null had to exceed 0.622, and the AUC the bar required was 0.75 (0.128 above null p99).
- INVALID-DESIGN trigger: H (time of day alone) = 0.729 >= 0.65, so any P up to 0.73 could be a clock artefact; the design could only count P as evidence above 0.73, which is above the 0.75 bar by only 0.02, and P (0.595) did not reach the null p99 anyway. Because the null p99 is a percentile of 100 nulls, its estimate is itself noisy (the 99th of 100 draws).
- Underpowered by own record: the plan and backlog records: "empty windows and presses happen at different hours".

### Summary of the weakest tests
Tests where the stated truth supply was too thin to separate a modest effect from luck, on the record: L021-L023 screens (15 windows, null floor 0.0625), L035 precision claim (9.2 h at node 3: 40% chance of zero events at the bar), L038/L039 (30 events behind 330 moments; 3.03 nobody-home h vs 10 h needed), L049 (n = 31 hit exactly the 5% threshold), L050 (bar needed 0.10 above a 0.123 detection margin: ~38% power), L063 (N = 1 night), L075 (effective n ~175 minutes of 2,177; clock-only readout 0.729). The decisive FAILs (L019 R9 week, L018 R24, L011 R18, L009 R13 at day level) are not underpowered in the failing direction (gap to bar 0.1-0.6) but none of them could have certified a PASS at <= 5% false presence with <= 8 windows.

