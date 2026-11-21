# Appendix A. Ledger of tests, theories, builds and errors

Each entry is one test, theory, build or study error, with its question, verdict, evidence tag and key numbers; IDs (L###) are cited in the main text.
Tags: MEASURED (reproducible from recorded data), CLAIMED (asserted, not reproduced), SYNTHETIC (simulated or demonstration data).
Verdict words: PASS, FAIL, NOT-PROVEN, FALSIFIED, SUPPORTED, INVALID-DESIGN, VOID, BUILT-WORKS, BUILT-FAILED, OPEN.

| ID | date | category | question | verdict | tag | key number(s) |
|---|---|---|---|---|---|---|
| L001 | 2026-09-24 | presence | Does any full-rate AP-link statistic show a person response? | FAIL | MEASURED | 5/9 links pass rate ratio but adjacency 519 vs null 561.0 (p~0.035 vs 0.0025): outcome C |
| L002 | 2026-09-30 | presence | Does RSSI standard deviation differ between occupied and empty? | NOT-PROVEN | MEASURED | morning p<0.001 but empty-halves control p=0.033; evening control p=0.002 |
| L003 | 2026-10-01 | presence | Can plate reads or car Wi-Fi give arrive/depart labels? | NOT-PROVEN | MEASURED | 33 sessions; plate arrive within 5 min on 19; 22/24 plate arrivals have Wi-Fi |
| L004 | 2026-10-01 | presence | Does any earlier method beat baselines at house level? | FAIL | MEASURED | ~27 methods, 3-7 chosen days: all FAIL, false presence 0.19-0.94 |
| L005 | 2026-10-02..03 | presence | Does sink presence tell empty from occupied; which writer makes it? | FAIL | MEASURED | EMPTY 0.9978 vs OCCUPIED 0.9989 presence=true; firmware flag 84.1% vs 84.3% |
| L006 | 2026-10-02 | pets | Live sink output in an animals-only window? | FAIL | MEASURED | presence=true 6,098/6,101 lines (99.95%); persons 2-20 |
| L007 | 2026-10-03 | presence | Do node motion energy and firmware presence respond to occupancy? | FAIL | MEASURED | presence=true 100% empty and home; edge motion energy higher when empty |
| L008 | 2026-10-04 | presence | Which feature separates empty from occupied on a clean same-day transition? | NOT-PROVEN | MEASURED | firmware motion AUC 0.45; amplitude variability median 0.77 |
| L009 | 2026-10-06 | presence | Can two amplitude features detect house presence (label-free vs supervised)? | FAIL | MEASURED | waking AUC median A .57 / B .58; animals-only false presence A 37-93%, B 67-94% |
| L010 | 2026-10-06 | presence | Does an RF-quality mask plus 0.3-3 Hz band features fix presence? | FAIL | MEASURED | mask keeps 85.6/81.9/24.5/86.1/97.5/97.9% of local day; masked A FP 4-7% not real (51% undecided) |
| L011 | 2026-10-06..13 | presence | Does keeping clean upper tones (R18 FG) beat dropping them? | FAIL | MEASURED | secondary: FG median AUC .64 vs LS .73; primary: false presence 28-44% on confirmed-empty |
| L012 | 2026-10-06..07 | presence | Does the label-free method beat the sink label actually displayed? | PASS | MEASURED | live label present 100% of minutes, AUC 0.50; candidates median AUC .71-.78 |
| L013 | 2026-10-06 | rf-physics | Do R17 lower-segment features pass the presence bar? | FAIL | MEASURED | masked A waking AUC .59/.70/.72, B median .78; A FP 15% (51% undecided), B FP 31%; fails R13 bar |
| L014 | 2026-10-06 | presence | Can presence_v2 run live in the sink and reproduce offline numbers? | PASS | MEASURED | first live gate 27/32 = 84%; after stamping fix 37/37 |
| L015 | 2026-10-07 | presence | Does a 15-min strong-node hold raise detection without false presence? | FAIL | MEASURED | S alone detection median +9.6 pts; D adds false presence +4/+25/+42; decision days R23b detection +0.207 |
| L016 | 2026-10-07 | presence | Does a witness-weighted vote add detection beyond the house median? | FAIL | MEASURED | bar days: detection gain +0.00 (bar +0.10); decision days +0.008; gate mismatched 2 of 3 days |
| L017 | 2026-10-09 | presence | Would holding presence for T minutes keep still sleepers present? | FAIL | MEASURED | T 0/30/60/120: detection .57/.91/.95/1.00; away FP .48/.87/.93/.93 |
| L018 | 2026-10-10..16 | presence | Do legacy (non-HE) frames carry occupancy better than presence_v2? | FAIL | MEASURED | test week median detection 0.601, pooled false presence 0.192 (bar 0.70 / 0.05) |
| L019 | 2026-10-15 | presence | Does presence_v2 meet the house-presence bar on a held-out week? | FAIL | MEASURED | median detection 0.62, pooled false presence 0.63 (bar 0.70 / 0.05) |
| L020 | 2026-10-18..11-03 | presence | How well does a phone Wi-Fi/BLE rule call the house empty? | PASS | MEASURED | sealed forward test H1: 1.31% false AWAY, 52.2% caught; H2 1.05%, 50.8% |
| L021 | 2026-10-22 | presence | Do per-link RF features separate empty from occupied? | FAIL | MEASURED | 0 survivors: null max 6.86 (one shift; next 4.32), best 3.17 |
| L022 | 2026-10-22 | presence | Does RSSI, noise floor, gain or count separate empty from occupied? | FAIL | MEASURED | 0 survivors: null max 3.44, best 2.41; shifted-window control gave 2 survivors |
| L023 | 2026-10-22 | presence | Does any motion-band feature on any node separate empty from occupied? | FAIL | MEASURED | 0 family-wise survivors: null max 4.37, best 3.22 |
| L024 | 2026-10-22 | presence | Does whole-house power catch the phone-rule failure? | FAIL | MEASURED | power alone: 30/33 empty hours called empty but 83/374 occupied (22%) |
| L025 | 2026-10-22 | presence | Does whole-house power separate empty from occupied hours? | SUPPORTED | MEASURED | raw AUC 0.75 [0.57,0.86], recall@5%FP 0.03; same-hour normalised AUC 0.87, 0.30 |
| L026 | 2026-09-14 | room-activity | What did the fleet report against a labelled live window? | OPEN | CLAIMED | never scored; label approximate |
| L027 | 2026-09-23 | room-activity | Can per-transmitter co-detection learn node witness zones? | OPEN | SYNTHETIC/MEASURED | premise rule: >=3 node pairs under 2+ tx and >=1 without AP; 17 of 178 links live-rate |
| L028 | 2026-09-24 | room-activity | Does co-detection pass the frozen rule? | PASS | MEASURED | 36 of 36 pairs co-detect, 2-11 tx agreement, none via AP; saturated |
| L029 | 2026-09-24 | presence | Does AP-link raw_motion respond to a walking person? | FAIL | MEASURED | own-room in-poll ratios 0.79-1.10; 24 h range 1.45x; permutation p=0.044 post hoc |
| L030 | 2026-09-24 | room-activity | Does the coverage event stream witness a walking person? | FAIL | MEASURED | balloon 0/108, balloon_live 0/46, own_room 0/11; bins 205/min quiet vs 215/min walk |
| L031 | 2026-10-13 | room-activity | Can a node give a high-certainty 'room active' event? | NOT-PROVEN | MEASURED | false events/h 0.00-0.11 on 5 nodes; events 1-6% of awake minutes; 0-14% of switch-ons |
| L032 | 2026-10-15 | room-activity | Does a per-node rule catch known-room human actions? | NOT-PROVEN | MEASURED | node 5 catch 0.86 vs base 0.39; no rule <=0.1 false/h; z-wipe 0.68 -> 0.26 false/h |
| L033 | 2026-10-16..18 | room-activity | Does the draft burst rule catch anything on the fixed sink? | NOT-PROVEN | MEASURED | 0 false events at chosen N; catches none: node 5 0/11, node 6 0/3, node 3 0/1 |
| L034 | 2026-10-17..18 | room-activity | What per-node burst rule and bars would be sealed for room activity? | NOT-PROVEN | MEASURED | node 6 caught 3/10 (all on one reference board; 0/7 on AP); node 5 excluded (3 false in 15 night h) |
| L035 | 2026-10-18..28 | room-activity | Does a sealed burst rule tell when a room is active? | PASS | MEASURED | PASS nodes 0,3 (0.0 false/h over 37.1 h, 9.2 h); catches node 5 2/107, node 6 1/51, node 8 0/22, node 3 0/3 |
| L036 | 2026-10-18 | room-activity | Seal the draft, a looser rule, or a precision-only claim? | OPEN | MEASURED | since the sink fix: zero false alarms, zero catches; option C chosen |
| L037 | 2026-10-22 | room-activity | Does any node/feature fire in own-room event minutes above chance? | FAIL | MEASURED | room 5: null max 5.80, 0 survivors; rooms 5;6: 11.29, 0; room 8 untested |
| L038 | 2026-10-29 | room-activity | Does a blind sandboxed analyst find a rule beating its own null? | SUPPORTED | MEASURED | node 5 rule: catch 0.176 vs base 0.034, 0.024 false/h dev; held-out empties 0.46 false/h |
| L039 | 2026-11-02..11-10 | method-error | Did the sealed blind confirmation week catch own-room moments? | NOT-PROVEN | MEASURED | graded 0/99 caught vs base 0.0042; 0 false in 3.03 nobody-home h (<10 h); 70.5 of 168 h covered |
| L040 | 2026-08-27 | position | Does scalar per-node energy weighting localise motion? | FAIL | MEASURED | clean negative: magnitude tracks proximity, direction does not |
| L041 | 2026-08-28 | position | Can labelled fingerprints classify position among five known spots? | SUPPORTED | MEASURED | NN 6/10, mean error 27 in; between/within L2 2.54 |
| L042 | 2026-08-28 | method-error | Does anyone upstream have position or pose working? | FAIL | CLAIMED | 81.63% random split vs ~11.6% cross-subject (target 30%) |
| L043 | 2026-09-01 | position | Is the measurement manifold smooth enough to reconstruct a field? | NOT-PROVEN | MEASURED | rho +0.273 (p 0.542); LOO IDW +6.3% (p 0.133); LOO NN -39.8% |
| L044 | 2026-09-03 | method-error | Does the Doppler/BVP centroid position tier discriminate motion? | BUILT-WORKS | SYNTHETIC/MEASURED | static as % of moving 6.2% (bug) vs 0.45% (fixed); live metric still ~2.5-2.6 of 3.0 |
| L045 | 2026-09-03 | position | Does ratio normalisation improve room/position classification from marks? | NOT-PROVEN | MEASURED | RAW 9%, SUBTRACT 27%, RATIO 18% vs null 50% (n=11 of 28) |
| L046 | 2026-10-17..18 | position | Can link tomography place a moving person (room, floor)? | FAIL | MEASURED | baseline room hit 0.34 vs 0.23; est-rti-v1 lift +0.15 (n=36) vs +0.149; floor 2 empty blob 86% of home rate |
| L047 | 2026-10-18..20 | rf-physics | Would more node-node traffic make positioning work? | OPEN | CLAIMED/SYNTHETIC/MEASURED | bars H1 pairs>=2 fps >= max(24,2x), n2n >=1.4x, H3L lift >=+0.25; sim 46% false blobs unless gate scales |
| L048 | 2026-10-18 | rf-physics | Would transmit-only basement boards improve floor separation? | NOT-PROVEN | SYNTHETIC | floor-correct share A .13/B .10/C .11 (optimistic) vs .52/.55/.56 (pessimistic); basement blob ~0 |
| L049 | 2026-10-21..22 | position | Does est-rti-v1 beat chance on days after the 10-17 tuning? | FAIL | MEASURED | 107 moments, 13 hits, 0.12, lift +0.04; empty/home blob ratio 0.98 |
| L050 | 2026-11-09..20 | position | Did tripling node-to-node frames improve est-rti-v1 positions? | FAIL | MEASURED | room-moment lift 0.197 OFF = 0.197 ON (bar +0.10); BLE nearest-node chance 0.450/0.450 |
| L051 | 2026-09-08 | rf-physics | Why does the per-link motion metric lose still people? | SUPPORTED | MEASURED | absorbed in 3-8 s; raw_motion worse (U 12/20 vs 16/20) |
| L052 | 2026-09-08 | presence | Does motion level separate occupied sleeping rooms from empty rooms overnight? | NOT-PROVEN | MEASURED | U = 16/20 (critical 19); occupied ranks 2-5; empty living room rank 1 |
| L053 | 2026-09-14 | vitals-stillness | Does the breathing window ever hold 30 s of data? | SUPPORTED | MEASURED | 6 of 9 nodes 0% full; clears 1.7-15.6/min |
| L054 | 2026-09-14 | vitals-stillness | Can gated vitals calibration run in an occupied home with pets? | OPEN | MEASURED/CLAIMED | calibration needs 600 s empty house every 12 h; baseline expiry 86400 s |
| L055 | 2026-09-14 | rf-physics | Is the breathing-buffer retune sawtooth real bursts or EMA jitter? | SUPPORTED | MEASURED | replay vs live retune rate ratio 0.87-1.48x on 7/8 nodes; node 0 max CSI gap 10,662 ms |
| L056 | 2026-09-15 | vitals-stillness | Does a slow EMA stop breathing-window clears? | BUILT-WORKS | MEASURED | clears 10.4-14.4 -> 0.00-0.30/min offline; live window full 92-99% |
| L057 | 2026-09-16 | vitals-stillness | Do people cause the vitals-buffer clears? | SUPPORTED | MEASURED | clears/h EMPTY 27.2, OCCUPIED 10.0 / 36.1, ASLEEP 9.3 |
| L058 | 2026-09-17 | vitals-stillness | Can the breathing buffer survive rate wander? | BUILT-WORKS | MEASURED | replay 584 -> 151 fires (300 s dwell); live clears ~42x lower (0.583/h) |
| L059 | 2026-09-20 | vitals-stillness | Which route gives vitals authority in a home with pets? | OPEN | CLAIMED | research path chosen; no result |
| L060 | 2026-10-09 | vitals-stillness | Do nodes show a breathing peak for a sleeper? | NOT-PROVEN | MEASURED | ratio>=4 windows: master NIGHT 8% vs EMPTY 19%; steady share ~20% in both; bpm medians 20-26 in both |
| L061 | 2026-10-09 | vitals-stillness | Can still people be seen as shape change versus empty? | NOT-PROVEN | MEASURED | rule failed: occupied R 1.3-19 vs controls up to 51 |
| L062 | 2026-10-09 | vitals-stillness | Does link shape change iff either endpoint room is occupied? | FAIL | MEASURED | AUC .62/.88/.75; room calls 0/7, 2/7, 0/7; only 6-8 links judgeable |
| L063 | 2026-10-10 | vitals-stillness | Does a node see a sleeping person (watch heart rate as truth)? | FAIL | MEASURED | node 4 motion 0.95-1.2x median all night, max 1.29; Spearman with heart rate -0.07 |
| L064 | 2026-10-13 | room-activity | Can a node see a person seated at a desk? | FAIL | MEASURED | desk: dev AUC 0.32/0.43, test 0.60/0.55 vs bar 0.70; own-circuit jumps node 4 +0.15, node 2 +0.27 (n 7) |
| L065 | 2026-11-13 | vitals-stillness | Does the stock fall detector fire spuriously? | FAIL | MEASURED | 13 'Fall detected' events overnight across 5 nodes; disabled; 0 after |
| L066 | 2026-09-23 | presence | Can a pet-occupied node be detected so it does not learn empty? | BUILT-WORKS | SYNTHETIC/MEASURED | no node over activity ratio 3.0; highest 2.32 (node 7); quiescence rule 16/51 vs 0/51 windows |
| L067 | 2026-10-09 | pets | Is false presence caused by pets or the RF environment? | FAIL | MEASURED | closed-door rooms fire as often empty (n1 .49/.44) as occupied; only n6 .14/.52 |
| L068 | 2026-10-13 | pets | What drives false presence in empty or animals-only minutes? | FALSIFIED | MEASURED | score flags EMPTY 40%, AWAKE 51%, ASLEEP 63%; AP and HVAC correlations rho <= 0.2 |
| L069 | 2026-10-18 | pets | Can any feature separate person events from pet events per node? | FAIL | MEASURED | node 5 band AUC 0.86 [0.71,0.97], node 6 0.81 [0.60,0.97] (microwave?); bar CI low > 0.75 on >=3 nodes |
| L070 | 2026-10-18 | pets | Do pets trigger the animal-room node at night? | NOT-PROVEN | MEASURED | 4 events on one fixed-sink night (~1 h); daytime only |
| L071 | 2026-08-31 | presence | How good is the trained adaptive classifier? | FAIL | MEASURED | training accuracy 0.486 (chance 0.25); trained on a vanished 3-board setup |
| L072 | 2026-09-23 | presence | Is the per-link quiet floor stable enough to learn without calibration? | SUPPORTED | MEASURED | drift floor median/p90 2.5/4.4% (10 h), 4.3/6.9% (7.2 h); rule passed 0 of 30, 0 of 21 |
| L073 | 2026-09-27 | raw-learning | Are Hampel (S5) and stable-subcarrier (S4) statistics well defined? | BUILT-WORKS | MEASURED | S5 replaces 6.9-13.5% of samples; S4 12/12 on 9 links; buffer CENTRED |
| L074 | 2026-10-06..13 | raw-learning | Does a small neural net beat label-free and supervised baselines? | FAIL | MEASURED | supervised AUC median .68 unmasked / .73 masked vs shuffled control .56 / .51; primary: loses to shuffled labels on 3 days |
| L075 | 2026-11-20..21 | raw-learning | Does the raw CSI hold presence information the per-minute features drop? | INVALID-DESIGN | MEASURED | P AUC 0.595 vs null p99 0.622; F 0.618, T 0.261, H (time of day) 0.729 |
| L076 | 2026-11-20 | raw-learning | Is any upstream pretrained model or self-learning claim usable? | FAIL | CLAIMED/MEASURED | encoder one sleeper/one room; pose cross-subject 64%, cross-environment ~10% |
| L077 | 2026-08-28 | rf-physics | Can per-link phase Doppler give radial velocity? | FALSIFIED | MEASURED | resultant length R 0.011-0.032 vs 0.000; std 1.78-1.81 vs 1.814 |
| L078 | 2026-08-29 | infrastructure | Does ESP-NOW sync work and is the fps estimator correct? | BUILT-WORKS | MEASURED | skew -4.46/-7.42 ppm; EMA 21.1% above 50 fps; node 0 +21% |
| L079 | 2026-08-31 | rf-physics | Does moving a weak node toward the AP help? | SUPPORTED | MEASURED | AP link 73% @ -81 -> 0% -> 100% @ -64; best corner 17 dB better |
| L080 | 2026-09-01 | rf-physics | Do positioned emitters surround each node? | FALSIFIED | MEASURED | blind arcs 101-286 degrees; six of nine nodes blind to one side |
| L081 | 2026-09-02 | rf-physics | What radios illuminate the east basement? | SUPPORTED | MEASURED | 7 placed + 1 unplaced emitter; three nodes depend on one printer |
| L082 | 2026-09-02 | rf-physics | Are two windows comparable after a power event? | SUPPORTED | MEASURED | audible transmitter set swapped wholesale; one device 1.1 -> 6.9 links |
| L083 | 2026-09-03 | rf-physics | What does moving a second AP to another channel do? | NOT-PROVEN | MEASURED | links 178 -> 147; transmitters 38 -> 33; single snapshot |
| L084 | 2026-09-03 | rf-physics | Why were most overheard transmitters never becoming links? | BUILT-WORKS | MEASURED | links 31 -> 137; transmitters 10 -> 32 |
| L085 | 2026-09-04 | rf-physics | Which frame-selection gate gives the highest cross-node paired fraction? | SUPPORTED | MEASURED | U 0.2434; M 0.2480; S 0.5605 (2.30x); MS 0.5452 |
| L086 | 2026-09-04 | infrastructure | Which nodes run hot and does it corrupt link diagnostics? | SUPPORTED | MEASURED | enclosed node 49 C vs 29-40 C; clear rate r = -0.94 |
| L087 | 2026-09-04..09-29 | infrastructure | Why do 5 of 9 nodes never hold mesh sync? | BUILT-WORKS | MEASURED/SYNTHETIC | v1: 6/9 valid, 2 leaders; v2 multi-hop: 9/9 valid, 1 leader, hops 1-3 (500 sim seeds clean) |
| L088 | 2026-09-07 | rf-physics | Can household IoT transmitters serve as illuminators? | FALSIFIED | MEASURED | 3 of 36 clear 2+ receivers at >=1 fps; IoT grid-64 at <=0.1 fps |
| L089 | 2026-09-07 | rf-physics | What transport do the loud energy meters use? | SUPPORTED | MEASURED | both are 2.4 GHz WiFi clients of the gateway own AP (-85 / -71) |
| L090 | 2026-09-08 | infrastructure | Does a different frame selection or timebase improve pairing? | FALSIFIED | MEASURED | 358 falsified; 359 +0.0121 but bucketing loses; 360 AP share 46.2% -> 46.8% |
| L091 | 2026-09-09 | rf-physics | Does ambiguity about the heard transmitter suppress cross-node pairing? | SUPPORTED | MEASURED | paired fraction 0.5668 / 0.7287 / 0.5836 (A1/B/A2); paired obs/s 41.4 -> 24.7 -> 47.6 |
| L092 | 2026-09-15..18 | rf-physics | Do fleet-wide fps dips coincide with transmitter-population shifts? | SUPPORTED | MEASURED | median consecutive-window cosine distance 0.0025; 5 crossings of flag 0.0101 (5x-31x) |
| L093 | 2026-09-15 | rf-physics | Does raising the SSID minimum rate stop 1-2 Mbps latching? | FALSIFIED | MEASURED | node 5 retry 23.7% -> 36.7%; node 6 newly collapsed 34.8% |
| L094 | 2026-09-15 | rf-physics | Why did a second node reach 46-48 C? | SUPPORTED | MEASURED | node dominant 2 and 1 Mb/s, retry 23.7%, airtime share 8.3%; AP ~81% |
| L095 | 2026-09-20..21 | rf-physics | Is the two-node rate collapse airtime contention or the node own link? | SUPPORTED | MEASURED | AP airtime 84% (9 nodes) -> 48% (7 blocked) -> 18% (all blocked); node 6 retry 34.8% -> 11.1%; node 5 36.7% -> 18.2% |
| L096 | 2026-09-22..23 | infrastructure | How much does delivered fleet CSI rate drift with no intervention? | SUPPORTED | MEASURED | fleet cv 14.8% (raw), share-of-fleet cv 1.0-6.0%; steps ~200 -> ~242 -> ~165 fps all nodes together |
| L097 | 2026-09-23 | rf-physics | Are links reciprocal and transmitter powers comparable? | BUILT-WORKS | MEASURED | 13 pairs agree within 2.1 dB; slope -20.4 dB/decade; reach 10.2-13.9 m |
| L098 | 2026-09-27 | rf-physics | Can C6 AGC/FFT gain be forced and read via public API? | NOT-PROVEN | MEASURED/CLAIMED | setters present, undeclared; no public read |
| L099 | 2026-09-28 | rf-physics | Did setting AP TX power to High change node RSSI or fps? | OPEN | CLAIMED | not measured; ~20 clean polls |
| L100 | 2026-09-28 | rf-physics | Is there a gain field in C6 rx_ctrl, AGC at least 40? | PASS | MEASURED | 51 of 3,574 bit-fields pass, all word 5; bits 24-31 rho -0.89..-0.98 on 9/9; hourly AP median >= 40 all nodes |
| L101 | 2026-09-29..30 | rf-physics | Are AGC gain steps enriched in quiet-hour motion spikes? | FAIL | MEASURED | first attempt RR 1.636 p 0.0125 (5 of 9 nodes); retest on 7 nodes RR 0.742 p 0.879: KILL |
| L102 | 2026-09-30 | rf-physics | Does the 11b block stop 1-5.5 Mbps frames? | SUPPORTED | MEASURED | node 6 1093 of 10263 frames/min at 11b; hidden BSS 3.0% airtime |
| L103 | 2026-09-30..10-02 | rf-physics | Did 0.9.4 firmware and a dedicated 2.4 GHz SSID free channel airtime? | SUPPORTED | MEASURED/MODEL | daily median cu_total 80-85% -> 44% (10-01); first night 54% mean / 47% median; slow node-minutes 988-1968/day -> 89 |
| L104 | 2026-09-30 | rf-physics | Why does delivered fps decay with uptime? | SUPPORTED | MEASURED | node 6 8.1 -> 1.0 Mbps; nodes 4+6 ~88% of fleet airtime; fps corr -0.84 with sum(1/rate) |
| L105 | 2026-10-02..05 | rf-physics | Do ping rate, IDF version or fixed rate stop the slide? | FAIL | MEASURED | ping 25 Hz: accepted fps -2.5/-1.7, heard-by-peers 3.8 -> 1.4/1.8; IDF 5.4.4 node slid overnight and stayed ~7.5 h |
| L106 | 2026-10-02 | rf-physics | Does node 4 spend 3-4x the upload airtime of others? | FALSIFIED | MEASURED | frames normal; 506-576 us/frame at 8.1 Mbps vs 80-221 |
| L107 | 2026-10-03 | rf-physics | Why did firmware motion_energy double on all nodes mid-morning? | SUPPORTED | MEASURED | node 6 22% airtime, node 5 11% at 1 Mbps; channel 45% -> 70% -> 82% |
| L108 | 2026-10-06 | rf-physics | What corrupts per-frame shape of AP HE-SU frames? | SUPPORTED | MEASURED | AP HE-SU Beamformed=1 on ~96%; M1 burst on 15-45% of AP frames, buffers 152-250 jump 3-5x; M2 steering R2 0.07-0.62 |
| L109 | 2026-10-09 | rf-physics | Were nodes 5/6 slow minutes an OTA regression? | SUPPORTED | MEASURED | 19 minutes at >=70% busy (max 86%); own airtime ~13-17% of 76-86% |
| L110 | 2026-10-13 | rf-physics | What triggers the thermostat rejoins preceding node slides? | FALSIFIED | MEASURED/INFERRED | within 5 min of schedule 0/117 and 5/115; 113/117 within 3 min of each other |
| L111 | 2026-11-01 | rf-physics | Does the fleet show CSI capture stalling while other packets flow? | FALSIFIED | MEASURED | 0 stalls; 46 silent gaps = AP blackouts |
| L112 | 2026-11-01 | rf-physics | Would upstream grid vote lock differ from ours? | SUPPORTED | MEASURED | all 9 nodes majority 256/HE-SU 56.4-80.0%; same accepted share under both rules |
| L113 | 2026-11-08 | rf-physics | Can flipping the fleet between two APs add resilience and a view? | FAIL | MEASURED/CLAIMED | AP-B better only for node 5; 8 of 9 lose 2-21 dB; fleet fps -28% |
| L114 | 2026-11-12 | rf-physics | Why did fleet fps rise from ~191 to ~268 on 11-12? | SUPPORTED | MEASURED | overnight 256.6 vs 186.7; coincided with firmware roll |
| L115 | 2026-11-12 | rf-physics | Should invalid-estimate HE frames be dropped? | OPEN | MEASURED/CLAIMED | he_valid 1102 / he_invalid 2425 (69% invalid); DROP vs PASS not scored |
| L116 | 2026-11-15 | rf-physics | Can a node eavesdrop on the other channel without leaving its AP? | BUILT-WORKS | MEASURED | arm 1: ch 6 0.65/s for ~4 fps (13-15%) cost; parked |
| L117 | 2026-11-16 | rf-physics | Why are node-to-node links stuck ~0.5 fps? | BUILT-WORKS | MEASURED | gate keeps rx_seq%8==0; bypass n2n 122 -> 418 fps (x3.3); pairs>=2 fps 24 -> 39 |
| L118 | 2026-08-28 | theory | Does CSI response fall with distance from the link line (RTI)? | FALSIFIED | MEASURED | response inverse to link-line distance: 11 in -> /z/ 3.8, 47 in -> 13.0; solve 58 in vs 49 in fixed point |
| L119 | 2026-09-03 | theory | Can UWB anchors or BLE tags give truth and identity? | OPEN | CLAIMED | ideas only; registering phones will not happen |
| L120 | 2026-09-07 | theory | Do interior room measurements sum to the exterior outline? | OPEN | SYNTHETIC | proposal only; example figures SYNTHETIC |
| L121 | 2026-09-16 | room-activity | Do co-receiver disturbances covary so groups mean separate movers? | NOT-PROVEN | CLAIMED | paired_fraction 0.607; pooled r +0.44 on pairs with 0/3 per-tx agreement -> centred median r +0.214 -> +0.072; 0 groups on real capture |
| L122 | 2026-09-23 | room-activity | Does a body near a receiver dent all its links at once? | FALSIFIED | SYNTHETIC/MEASURED | zero seed pairs in 3 recordings; rx events never co-denting 2 links 99.4% / 92.4% / 86.4%; synthetic 16/17 pass |
| L123 | 2026-09-27 | theory | What replaces G2 option C after outcome C? | OPEN | CLAIMED | memo; S1 (3/9) did worse than S0 (5/9) |
| L124 | 2026-09-27 | theory | What do other ESP32 CSI projects offer? | OPEN | CLAIMED | outside numbers CLAIMED; leakage example 82.7% random vs 52.3% grouped split |
| L125 | 2026-10-13 | theory | Is high-certainty per-room activity a better target than presence? | FALSIFIED | CLAIMED | reframe only; superseded 10-29 |
| L126 | 2026-10-29 | theory | After the presence tests and screens, is per-room activity worth pursuing? | NOT-PROVEN | MEASURED | 11 pre-registered presence tests + 3 screens: no RF signal above chance; expected false survivors 0.15, observed 0 |
| L127 | 2026-08-28..09-03 | method-error | Did UI or pose output present fabricated values as measurement? | INVALID-DESIGN | MEASURED/SYNTHETIC | client simulation default-off; demo confidence 88% hard-coded; skeleton noise seeded by person index |
| L128 | 2026-09-01 | method-error | Do all links in the RTI solver describe the same instant? | INVALID-DESIGN | MEASURED | window spans 1.2 s to 3494 s (median 165.8 s); only 27 of 155 links under 20 s |
| L129 | 2026-09-03 | method-error | Does firmware latch a mean+3 sigma threshold? | OPEN | CLAIMED | true floor ~14x below animals; second server baseline untested |
| L130 | 2026-09-03..10-10 | method-error | Did contribution work leak house data, and did the push guard work? | INVALID-DESIGN | MEASURED | 136 CSI diagnostic files staged once; 3 real MACs in test fixtures of 2 PRs; guard absent on branches cut from origin/main |
| L131 | 2026-09-03 | method-error | Were extracted contribution branches functionally alive? | BUILT-FAILED | MEASURED | 11 of 13 server branches passed; dead match arm; 531 tests passed yet feature inert |
| L132 | 2026-09-03 | method-error | Was the firmware build under test the fleet configuration? | VOID | MEASURED | active config 4 MB / partitions_4mb / TX buffers 64 vs fleet 16 MB / 128; 3 of 5 sdkconfig keys fail on origin/main |
| L133 | 2026-09-04 | method-error | Was the sequence-number gate inert? | INVALID-DESIGN | MEASURED | gated 18.3 fps vs control 21.8 (ratio 0.84); rx_seq%16 uniform 5.07-7.20% over 1084 transmissions |
| L134 | 2026-09-14 | infrastructure | Did the C6 edge pipeline silently drop all HE20 frames? | FALSIFIED | MEASURED | all nine nodes frames_rejected 0; counter never assigned |
| L135 | 2026-09-14..10-06 | method-error | Where did stale, zero-filled or mis-sliced values pass as live? | BUILT-FAILED | MEASURED | duplicate CSI_MAX_FRAME_SIZE 2068 vs 2076; disturbance age gate 5 s (3 of 4 tests fail with gate off); node_log constants 0 |
| L136 | 2026-09-14 | method-error | Why is vital-signs buffer_status always 0/300 and vitals abstained? | BUILT-WORKS | MEASURED | reported buffer belongs to an AppState detector nothing feeds; capacity 300/150 = untouched 10 Hz constructor |
| L137 | 2026-09-15..17 | method-error | Did the clear/retune classifier verdicts mean anything? | INVALID-DESIGN | MEASURED | classifier "464 genuine / 462 jitter-residue" (55 h) vs after dwell: 0 genuine, 546 hunting; 4-min run 0 genuine / 14 unresolved |
| L138 | 2026-09-16 | method-error | Why did three PRs compile-fail after merging upstream? | BUILT-WORKS | MEASURED | positional Vec vs keyed map; tests 4647 pass after fix |
| L139 | 2026-09-17 | method-error | Does the grouping threshold separate quiet from busy night? | INVALID-DESIGN | MEASURED/CLAIMED | quiet partition empty; 0.45 threshold retracted; Jaccard 0.26 |
| L140 | 2026-09-18 | method-error | Was a PR actually failing CI? | INVALID-DESIGN | MEASURED | cargo check exit 0; 203 passed / 0 failed |
| L141 | 2026-09-21..23 | method-error | Did the ADR-360 matrix, canary and delivery checks mean what was claimed? | INVALID-DESIGN | MEASURED | 7 OTA pushes silently no-op; mesh fields were a 20.0 default; csi_fps_samples 0 -> 0 on nodes 0/3/7 |
| L142 | 2026-09-23 | method-error | Does the fleet run a continuous adaptive floor? | BUILT-WORKS | MEASURED | earlier 'deploy it' recommendation backwards; node 6 send_fail 448 |
| L143 | 2026-09-25..27 | method-error | Can a label-free resident-device rule give home/away context? | VOID | MEASURED | rule selected 7 of 60 clients (1 phone, 6 always-on IoT); empty span marked 10,800/10,800 s resident home |
| L144 | 2026-10-03 | method-error | Why does the persons= log read a constant 5? | BUILT-WORKS | MEASURED | tracker gate 10 cm units vs pixels; 3 px/tick gives exactly 5 tracks |
| L145 | 2026-10-06 | method-error | Did a code change push sink CPU back to ~1 core? | INVALID-DESIGN | MEASURED | 37% -> 108% when own extraction jobs started; sequential runs 51-98% |
| L146 | 2026-10-07 | room-activity | Can a slow room-by-room walk be graded per room? | INVALID-DESIGN | MEASURED | NOT GRADABLE: started 0-3 min after restart; visits under 1 min |
| L147 | 2026-10-07 | method-error | Why did per-node thresholds move at every sink restart? | BUILT-WORKS | MEASURED | thresholds .84/1.12... -> .43/1.35...; fix exact=true bit-identical |
| L148 | 2026-10-09 | method-error | Are phone Wi-Fi sessions reliable home/away ground truth? | INVALID-DESIGN | MEASURED | 110-min window labelled AWAY while a person was home; relabel: confirmed animals-only minutes 654 -> 867; every set still fails |
| L149 | 2026-10-16..18 | method-error | Do archive offline features overstate node 5/6 catches? | INVALID-DESIGN | MEASURED | archive cold-start: node 5 34 switches vs sink 8; reference arms node 6 caught 0/1/1 of 7, node 5 0/20 |
| L150 | 2026-10-18 | method-error | Would the sealed test chain run unattended correctly? | BUILT-WORKS | MEASURED | 6 defects found (stale path, silent less-data scoring, ...) |
| L151 | 2026-10-18 | method-error | Which method errors recur in the last five weeks? | OPEN | MEASURED | roll-up of 8 items (post-hoc pick, T1 not pushed, blind week broken ...) |
| L152 | 2026-10-22 | method-error | Does any method beat non-RF baselines on the whole capture? | INVALID-DESIGN | CLAIMED | design only; effective sample 15 empty windows (3 holdout) |
| L153 | 2026-10-23 | presence | Can phone-rule AWAY runs add more empty windows? | VOID | MEASURED | union equals hand log: 16 windows, 2,630 min; premise false |
| L154 | 2026-10-28..11-16 | infrastructure | Did the planned 80 ms fleet treatment happen? | FAIL | MEASURED | only nodes 3,7 at 80 ms; per-pair beacon CSI x1.5-1.65 vs bar x3; fleet push never happened |
| L155 | 2026-10-29 | method-error | Can any room-level claim be confirmed outside rooms with appliance truth? | INVALID-DESIGN | MEASURED | 132 moments: 107/51/22/3 per node, overlap-counted; 0 on five nodes |
| L156 | 2026-11-20 | method-error | Was the +0.12 room-moment lift evidence of weak localisation? | INVALID-DESIGN | MEASURED | lift is a room-bias artifact; truth ~90% one room |
| L157 | 2026-09-03 | method-error | Which nodes can auto-roll-back an OTA? | BUILT-WORKS | MEASURED | initially 3 of 9 valid; six USB-flashed bootloaders had no rollback; after OTA bootloader install 9/9 |
| L158 | 2026-09-14 | infrastructure | Is the 8 Hz edge DSP rate a defect? | SUPPORTED | MEASURED | edge_fps 7.96-7.99 vs csi_fps 22-29; 12 Hz arm 10.08, 20 Hz 11.50 |
| L159 | 2026-09-21..22 | infrastructure | Does batching CSI frames with staggering cut uplink airtime? | BUILT-FAILED | MEASURED | raw airtime went UP on flashed nodes (2.5% -> 8.3%, 4.3% -> 12.3%, 3.0% -> 15.7%) while retry fell; 4 boards crashed at B=1,2 |
| L160 | 2026-09-22 | infrastructure | Could on-node logging be ported to the fleet branch without regression? | BUILT-WORKS | MEASURED | GET /log 200 on canary; rollout 9/9; branches diverged 338 local-only / 63 origin-only commits |
| L161 | 2026-09-22..10-16 | infrastructure | Which recorders and cron jobs silently stopped? | BUILT-WORKS | MEASURED | 23 h of raw capture lost (10-15 to 10-16); background job died at 40 of 210 min |
| L162 | 2026-09-22 | infrastructure | Why are five nodes dark to the feature path? | BUILT-FAILED | MEASURED | nodes 0,1,2,3,7 zero accepted frames; grid-lock fix unit tests 1127 pass but nodes stay stale 10 min |
| L163 | 2026-09-23 | infrastructure | Does any sink consume the edge feature packets? | BUILT-FAILED | MEASURED/CLAIMED | 0003 8.27/s, 0006 8.88/s; 1-7% of uplink airtime; no consumer |
| L164 | 2026-09-23 | method-error | What product rules and data bind the design? | OPEN | MEASURED | no labelling; 10.0 h, 7.9 d, 40 GB inventories |
| L165 | 2026-09-23 | infrastructure | Can the sink parse batch envelopes and expose silently dropped packets? | BUILT-WORKS | MEASURED | 9/9 delivering; fps pre/post within +-2.3; 3 unparsed packet types ~9/s |
| L166 | 2026-09-23 | infrastructure | Can the controller give coarse home/away truth and AP airtime? | BUILT-WORKS | MEASURED | AP 2.4 GHz cu_total 79-86%; 68 clients; cu updates every 35-36 s |
| L167 | 2026-09-27 | infrastructure | Can raw rx_ctrl be logged without behaviour change? | BUILT-WORKS | MEASURED | 9/9 on new version; 4,198/4,198 rssi match; fps 221 -> 246 |
| L168 | 2026-09-28 | infrastructure | Can fleet health be seen and survive sink restarts? | BUILT-WORKS | MEASURED/SYNTHETIC | night report matched hand-built report; 1169 then 576+551 tests |
| L169 | 2026-09-28 | infrastructure | Which sink state takes >1 h to rebuild after restart? | BUILT-WORKS | MEASURED | two states (24 h coverage, 24 h KPI history); SIGTERM not handled |
| L170 | 2026-09-29 | infrastructure | Why was the build disk lost? | BUILT-FAILED | MEASURED | hot-resize of iSCSI LUN; journal aborted; 745 MB replay output ABORTED |
| L171 | 2026-09-30 | infrastructure | Can occupancy be scored with full-rate S0 against reported empty windows? | BUILT-WORKS | CLAIMED | rule frozen; setup only |
| L172 | 2026-10 | infrastructure | Which sensing routes were dropped and why? | OPEN | CLAIMED | dropped list from the plan record; phone rule rejected as product |
| L173 | 2026-10-02..07 | infrastructure | Can nodes scan BLE for phone identity, and what does it cost? | BUILT-WORKS | MEASURED | scanner on: heap 120 KiB, IRK MSB-first resolves 35 of 63; peers hear +24 fps; own fps DiD +0.45 (~2%) |
| L174 | 2026-10-02 | method-error | Do engine-fuser nodes at the origin contaminate what we measure? | NOT-PROVEN | MEASURED | fuser feeds only provenance and trust; presence/position own path |
| L175 | 2026-10-02 | infrastructure | Which upstream PRs touch our paths; which fixes are missing? | BUILT-WORKS | MEASURED | 307 PRs triaged (126 touch paths); audit 46 present exactly, ~34 absent |
| L176 | 2026-10-02 | infrastructure | Why can the sink not keep up with nine-node CSI? | BUILT-WORKS | MEASURED | 44% UDP drops at 101-102% CPU -> memoised Doppler: drops 0, container CPU 25.6%; baseline 17% kernel drop in replay |
| L177 | 2026-10-03 | infrastructure | What causes ~35-min sink CPU spikes? | SUPPORTED | MEASURED | NAS daemon 260-300% CPU every 35-37 min; 0 drops in 6,977 samples |
| L178 | 2026-10-04..16 | infrastructure | Does the link-rate watchdog recover slides? | SUPPORTED | MEASURED | 19 acts, 15 reached node, 15/15 recovered within 3 min; 7 soak nights no 10+ min slide |
| L179 | 2026-10-04..08 | infrastructure | Why did node 2 stop streaming while associated? | PASS | MEASURED | CSI stopped after 30-34 fps; heap 174K -> 100K, sendfail ~2,000/min saturating u16; 0 false reboots in 3 nights (7,096 records) |
| L180 | 2026-10-04..16 | infrastructure | Why does fleet CSI drop to zero for ~2 min? | SUPPORTED | MEASURED | 5 crashes on 10-04; all retained boots KERNEL_CRASH, same fault registers; 8.8.11 EA did not fix (crashes 10-11 x3, 10-13, 10-14, 10-16) |
| L181 | 2026-10-09 | infrastructure | Can post-fix scoring run without the assistant in the loop? | BUILT-WORKS | CLAIMED | runners built; 10-08 had 0 animals-only minutes |
| L182 | 2026-10-09 | infrastructure | Why did upstream CI and our PRs go red? | BUILT-WORKS | MEASURED | led_strip 3.1.0 needs IDF >=5.5; 13 PRs red; scanner-gaming change reverted |
| L183 | 2026-10-10 | infrastructure | Can local models replace Claude for repo operations? | PASS | MEASURED | mean /10: Opus 9.6, Sonnet 9.6, qwen 6.4-6.5, gpt-oss 4.9-5.9 |
| L184 | 2026-10-15 | infrastructure | Can rc1 firmware be rolled and records tooling made durable? | BUILT-WORKS | MEASURED | node 4 35.2 h unbroken; fleet 9/9, 295 fps; search recall@1 66.7% |
| L185 | 2026-10-18 | infrastructure | Can hourly position estimates be kept current? | BUILT-WORKS | MEASURED | hourly chunks byte-identical to continuous run |
| L186 | 2026-10-18 | infrastructure | What reliability tooling kept the recorder alive? | BUILT-WORKS | MEASURED | fw_verify negative controls; sink 101-107% CPU at ~268 fps, 0 drops |
| L187 | 2026-10-27 | infrastructure | Which raw capture hours are unreadable? | BUILT-WORKS | MEASURED | hour 1 ~67,374 B in 4 holes salvaged; hour 2 ~5 MB unreadable |
| L188 | 2026-10-28 | infrastructure | Can a package let a blind analyst search for a rule? | BUILT-WORKS | MEASURED | 380 MB package; 386 moments; phone rule rejected as product |
| L189 | 2026-11-13 | infrastructure | What if the pinned BSSID changes? | BUILT-WORKS | CLAIMED | [controller] slots shifted 11-08; fallback added, not hardware-tested |
| L190 | 2026-11-13 | infrastructure | Why do nodes log ESP-NOW send-callback stalls? | BUILT-WORKS | MEASURED | stalls 133/day -> 0 on canary; race fixed |
| L191 | 2026-11-13 | infrastructure | Why 2016 hourly-job failures a day? | BUILT-WORKS | MEASURED | 84 files retried per run since 11-09; skip list added |
| L192 | 2026-11-13 | infrastructure | Why was node_log off and what did the logs show? | BUILT-WORKS | MEASURED | FAT 8,089,600 B 0 free; 183,760 records 0 CRC failures; heap low-water 2K on node 3 |
| L193 | 2026-11-19 | infrastructure | Why did one node stay dark 94 min? | BUILT-WORKS | MEASURED/SYNTHETIC | node 5 heap 137->84 KiB; zero packets 94 min; rule 3 added |
