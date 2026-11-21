# Nine Radios, No People: a pre-registered field study of single-antenna WiFi sensing on ESP32-C6 in an occupied home

Field study contributed by @clonea1 (GitHub). Analysis performed with LLM coding agents under the owner's direction; see section 5.6.

**Appendices:** [A. Ledger](home-fleet-field-study/appendix-a-ledger.md) | [B. Methods](home-fleet-field-study/appendix-b-methods.md) | [C. Bars](home-fleet-field-study/appendix-c-bars.md) | [D. Glossary and truth supply](home-fleet-field-study/appendix-d-glossary.md) | [E. Timeline](home-fleet-field-study/appendix-e-timeline.md)

## Abstract

For about twelve weeks (2026-08-27 to 2026-11-21) nine single-antenna ESP32-C6 nodes recorded subcarrier-resolved WiFi channel state information (242 tones per frame, used as per-subcarrier amplitude and adjacent-subcarrier phase differences; one antenna gives no usable absolute or cross-antenna phase) in an occupied house where animals were always home. The fleet ran a fork of RuView, the system was required to learn without user labelling, and every result was scored on held-out days against shuffled-label nulls and non-radio baselines, with pass bars fixed before scoring (18 of 24 provably so). The record holds 193 items: tests, theories, builds and the study's own errors.

No people-sensing question was answered. For house presence, 30 scored methods (more than 60 variants) all failed. The best false presence on confirmed animals-only time was 0.192 against a bar of 0.05 [L018], and the production detector scored 0.62 detection with 0.63 false presence [L019] (MEASURED). A sealed room-activity rule "passed" only by almost never firing, and the blind confirmation week caught 0 of 99 moments [L035, L039]. An apparent position lift was a room-bias artefact [L050, L156]. Stillness and vital signs showed nothing on the one night with watch truth [L063]. A random-feature model on 2,177 minutes of raw CSI stayed inside its shuffled null (AUC 0.595 against 0.622) [L075] (MEASURED). A phone rule and whole-house power, signals the house already had, beat the radio [L020, L025].

The person's effect is buried under the channel's own variance (per-client beamforming at the access point, rate collapse and airtime contention, a shifting transmitter population, access-point crashes), under an instrument that absorbs a still body in seconds, and under scarce truth. Several tests could not have certified a pass. The result is therefore mostly *no evidence of a signal*, bounded by the effects each test could see, rather than proof of absence. A synthetic end-to-end demonstration shows the software chain is correct when it fires but wrong under the physics the fleet measured (SYNTHETIC).

The engineering worked. Rollback bootloaders reached 9 of 9 nodes (from 3), mesh sync 9 of 9 (from 6), a link watchdog recovered 15 of 15 slow links, and sink packet loss fell from 44 % to none [L157, L087, L178, L176] (MEASURED); 22 of 66 pull requests were merged upstream. The study closes with recommendations for how WiFi-sensing claims should be evaluated and labelled.


## 1. Introduction

### 1.1 Motivation

A camera answers "who is where" directly and at a privacy cost that many households will not pay. WiFi channel state information (CSI) offers a camera-free alternative: every packet a radio receives carries a measurement of how the room modified it, and in principle a body that moves, breathes or stands in the path changes that measurement. A receiver costs a few dollars, and the access point and the other radios in a home already generate the traffic. The proposition is attractive because it needs no cameras, no wearables and no cooperation from the occupants.

Most public evidence for it comes from laboratories: a few people, one room, labelled sessions, often a random split of one recording into training and test sets. This study asks the other question. It asks what a fleet of cheap radios can do in an ordinary occupied home, over months, with animals, with the occupants' routine undisturbed, and with no one asked to label, calibrate or leave. The fleet was nine single-antenna ESP32-C6 nodes in a house of two above-ground floors and a basement, running the upstream RuView firmware and sink software, observed from 2026-08-27 to 2026-11-21, about 12 weeks.

### 1.2 The claims under test

The upstream RuView README (origin/main, read-only) describes the project as detecting "people, measure breathing and heart rate, track movement" "through walls, in the dark, with no cameras or wearables" [UPSTREAM README]. Its capability list names presence and occupancy, vital signs, "camera-free pose" of 17 body keypoints, and a pretrained model with label-free self-supervised pre-training. The README is more careful than its headline. It labels device-reported vitals "unvalidated", calls the on-device pose model "first-cut" with PCK@20 of 3.0 % (below its own 35 % target), reports a pretrained-encoder figure of 82.3 % as "temporal-triplet accuracy", and records that an earlier "100% presence" figure was "retracted" [UPSTREAM README] (CLAIMED). The study therefore tests each capability at the strength its own status supports, and tests the unqualified headline where the README does not qualify it.

### 1.3 Research questions

Six questions follow from those claims, one chapter each, plus one engineering question.

- **RQ1 (chapter 6).** Is anyone home? House-level presence, against a false-presence bar fixed before scoring.
- **RQ2 (chapter 7).** Is this room active? Room-level activity, including a sealed rule and a blind confirmation week.
- **RQ3 (chapter 8).** Where is the person? Position, by weighting, fingerprinting, tomography and added node-to-node traffic.
- **RQ4 (chapter 9).** Can still people, breathing and vital signs be sensed?
- **RQ5 (chapter 10).** Do pets, always home, confound the result?
- **RQ6 (chapter 11).** Would a model learn from raw CSI what hand-built features miss?
- **Engineering question (Part II).** Can the software chain, from firmware to a person on a floor plan, be made reliable and correct, and is it the limit?

Chapter 12 asks why the answers came out as they did, by examining what the radio channel itself was doing.

### 1.4 Contributions

1. **A pre-registered, leakage-free field evaluation.** Bars were fixed or sealed before scoring; data were held out by day or by week; shuffled-label and family-wise nulls and four non-RF baselines (a phone rule, whole-house power and others) were used (chapter 5). Of 24 bars, 18 are proven to have been fixed first and six are named as weak [chapter 5].
2. **A channel-level explanation.** The nine-node channel carried large variance unrelated to people: beamformed access-point frames, rate collapses, shifting transmitter populations, access-point crashes and a receive gate that discarded seven of eight frames (chapter 12). This is an inference from the records, not a measured variance decomposition.
3. **A ledger of 193 items.** Every test, result and verdict, each with its tag, bar and reproducer where one exists (LEDGER, L001-L193). Most results are "no evidence of a signal", not "proof of absence", and the ledger says which.
4. **An engineering platform measurably better than upstream.** Rollback bootloaders on 9 of 9 nodes against 3 of 9 [L157], mesh synchronisation on 9 of 9 with one leader [L087], a sink fix that took 44 % UDP drops to none [L176], and an unattended recorder (Part II) (MEASURED).
5. **Scripts.** The scoring and analysis scripts that reproduce MEASURED results, with a note of which are unrecoverable (Appendix B and chapter 14).
6. **A set of method rules.** Nine rules drawn from this study's own errors, so that a later study need not repeat them (chapter 5.5).

The finding the study defends is bounded. Across every question, no method beat a bar fixed before scoring or a non-RF baseline [L020; L025] (MEASURED). The large-gap failures are secure for the detectors tested; the others are absent evidence, often because truth was scarce. Whether other hardware would behave differently is a hypothesis, tagged CLAIMED and untested here.

### 1.5 Reading guide

Scoring scripts will follow in a separate pull request once household-specific time windows are moved into an input file.

Chapter 2 places the study among public results. Chapters 3 and 4 describe the house, fleet and timeline, and chapter 5 the method, including the tag and verdict vocabulary (FAIL, NOT-PROVEN, INVALID-DESIGN, PASS, FALSIFIED, SUPPORTED, VOID), which is used identically throughout. Part I, chapters 6 to 11, reports results for RQ1 to RQ6. Chapter 12 explains the channel. Part II reports what worked. Chapter 13 synthesises, chapter 14 sets out threats to validity, chapter 15 the constraints that would change the answer, and chapter 16 recommendations for upstream. Appendices A to E are planned: the ledger, the scripts, the sealed bars, the method errors and a glossary. A reader short of time should read chapters 5, 13 and 14.

**Chapter summary.** This study tests, in one occupied home with nine single-antenna ESP32-C6 nodes over about 12 weeks, the capabilities upstream RuView states: presence, activity, position, vitals and learning from raw CSI. It poses six research questions and one engineering question. Its contributions are a pre-registered, leakage-free evaluation, a channel-level explanation, a 193-item ledger, a measurably more reliable platform, scripts and a set of method rules. The conclusions are bounded to this house and hardware.


## 2. Related work and the claims landscape

This chapter places the study among public WiFi-CSI results. It draws only on the ledger entries named and on upstream's own documents. Every outside number is CLAIMED: none was reproduced here.

### 2.1 What public results show, and under which splits

Public WiFi-CSI sensing results, including upstream's, are mostly measured in laboratory datasets with a small number of subjects, labelled activities and several antennas. The headline numbers are high. Under a random split of the same recording they are also inflated, because adjacent windows from one subject and one session fall on both sides of the split, and a model can match the session rather than the person.

Reports gathered from upstream's issue tracker show the effect. The headline pose result on the MM-Fi benchmark was 81.63 % torso-PCK@20 on a random split, against about 11.6 % cross-subject when leakage was removed, with a target of 30 % [L042] (CLAIMED). Upstream's later study reports 82.69 % on the random split, a zero-shot cross-subject figure of about 64 % and a cross-environment figure of about 10 % for pose, and for 27-class action recognition 88.08 % on a random split against 10.0 % cross-subject, near chance [UPSTREAM MMFI 2-3; L076] (CLAIMED). The two sets of cross-subject figures (about 11.6 % and about 64 %) come from different protocols and dates, and this study does not reconcile them. It reports both with their sources.

### 2.2 Why random splits inflate

A random split puts neighbouring windows in both halves. Where the signal is dominated by the session (room state, access-point behaviour, time of day), memorising the session scores well and says nothing about generalisation. The prior-art survey in this study gives an external example: the same data scored 82.7 % on a random split and 52.3 % under a grouped split by session [L124] (CLAIMED). This study met the same hazard in its own data. Time of day alone scored an AUC of 0.729 on a raw-CSI probe, above the probe itself at 0.595 [L075], which is why that probe was declared INVALID-DESIGN (MEASURED). Chapter 5 states the splits used here: held-out days, sealed weeks and shuffled-label nulls.

### 2.3 Upstream's MM-Fi study as the sibling of this document

Upstream's MM-Fi study is the closest neighbour of this document. It is candid in the same direction: its own takeaway is that zero-shot generalisation to new people and rooms collapses and that a short in-room calibration recovers most of the loss, with about 100 to 200 labelled samples as the stated budget [UPSTREAM MMFI 3.2-3.3] (CLAIMED). Its section 7 reports that a random frozen encoder with a trained readout comes within about 2 to 4 points of a fully trained encoder (73.8 % against 78.2 % in-domain, 62.1 % against 63.9 % cross-subject), so that "the encoder barely matters" [UPSTREAM MMFI 7; L076] (CLAIMED). This study's own raw-learning result points the same way: a random-convolution probe scored inside its null and below summary features [L075] (MEASURED).

The two documents differ in nearly every condition.

| Condition | Upstream MM-Fi study | This study |
|---|---|---|
| Setting | Public laboratory dataset, 40 subjects, 4 environments | One occupied home, about 12 weeks |
| Antennas, subcarriers | 3 antennas, 114 subcarriers | One antenna per node, 242 tones; amplitude and adjacent-subcarrier phase differences, no cross-antenna phase |
| Labels | Supervised, paired pose and action labels | None asked of occupants; sparse truth from phone, power and appliance events |
| Animals | Not applicable | Always home |
| Calibration | A labelled in-room calibration is the answer | Ruled out by design: nothing labelled, nothing emptied |
| Evaluation | Random split for comparison; cross-subject for the unbiased figure | Bars fixed before scoring, held-out days, nulls |

The calibration route is the difference that matters most. It is not available to a product that must work with no labelling, and the field setting here is that product's setting.

### 2.4 Upstream pretrained models

The upstream pretrained encoder was trained on one sleeper in one room, from 8 unnamed features at 1 Hz over 6,063 frames. Its earlier "100% presence" figure was retracted, and the 82.3 % figure is triplet ordering, not presence [L076] (CLAIMED). Upstream's own ADR reports that the incumbent per-node classifier gave a 99.8 % false-positive rate on an empty room [L076] (CLAIMED, upstream). The pretrained models were therefore not a usable baseline for this fleet.

### 2.5 Other ESP32 CSI projects

A survey of 9 and then 9 further public ESP32 CSI repositories offered five candidates: a Hampel filter, asymmetric debounce, a per-subcarrier quality gate, a vitals ensemble with abstention, and band choice by a subcarrier selection statistic [L124]. All outside numbers were CLAIMED, from single links and with no held-out split. No surveyed project solved the still-person absorption problem (chapter 9) without an empty-room calibration [L124] (CLAIMED). One project corroborated the finding that single-antenna phase is unusable [L124]. These projects are useful as a catalogue of techniques, and their numbers cannot be compared with this study's.

### 2.6 The gap

The published literature on multi-antenna, phase-coherent localisation is the reason to expect that other hardware might place a person; this study does not test it, and the claim stays a hypothesis (chapter 15). What the public record lacks is a negative result under pre-registration in an occupied home: the same questions, bars set before scoring, truth sparse, animals present, and no help from labels. That is the gap this study fills, with the limits chapter 14 states.

**Chapter summary.** Public WiFi-CSI results are mostly laboratory numbers, and under random splits they are inflated: a pose headline reported upstream fell from 81.63 % to about 11.6 % cross-subject in reports on its issue tracker, and its own MM-Fi study shows zero-shot collapse and an encoder that barely matters (all CLAIMED). That study is the sibling of this one but differs in setting, antennas, labels and the presence of animals. Other ESP32 projects offer techniques, not comparable numbers. The gap filled is a pre-registered negative result in an occupied home.


## 3. The deployment

### 3.1 The house

The deployment was a single occupied house with a basement and two above-ground floors. Nine nodes were installed on the above-ground floors, at most one per room, in rooms of ordinary kinds (bedrooms, an office, a kitchen and living spaces); the basement had no node [L046]. One main access point (AP-A) on the upper floor served every node and the household's clients; a second access point (AP-B) on a lower floor carried no node traffic and served only as a failover and as a one-evening test target [App. E: F05]. From 2026-09-23 the nodes sat on one fixed 2.4 GHz channel [App. E: F13]. The house held dozens of other radios, mostly household IoT clients (68 clients at one count [L166]), plus unassociated emitters, several of them in the basement [L081]. People were present on most days; the animals were always home [L164].

The occupancy is one household and one house. Nothing in the record can say how the results transfer to another building.

### 3.2 Hardware and firmware

Each node was an ESP32-C6 with one antenna. It reported channel-state information (CSI) as I/Q per subcarrier in HE20 format: a 256-entry buffer centred on the carrier, of which 242 in-band tones carry usable values and the rest are excluded [L100; L134]. The analyses were subcarrier-resolved: per-tone amplitude, band and segment selections (lower and upper segments, stable-subcarrier selection, outlier filtering) [L011; L013; L073], and adjacent-subcarrier phase differences, which cancel the per-packet offset [App. B: M05]. Absolute phase re-randomises every packet and was unusable [L077], and with one antenna there is no cross-antenna phase, so angle-of-arrival methods were never available. Not every frame is HE20. In 300,000 packets from one capture hour (2026-11-21), 40 % were 256-entry HE20 records (512 bytes of I/Q), 49 % were 64-entry legacy 802.11a/g records and 0.3 % were 128-entry HT records (MEASURED, one hour); the receiver is limited to 20 MHz channels, so no frame carries more than 256 subcarrier entries. Most features and the raw-CSI probe used the HE20 records only [L075]. The illuminators were frames from the associated access point and beacons sent between the nodes themselves over ESP-NOW. The nodes shared a mesh time sync, which in its first version held on only 6 of 9 nodes and in the multi-hop version on 9 of 9 with one leader [L087]. A rate estimator in the first firmware read 21.1 % of samples above the 50 fps physical bound and was corrected [L078] (MEASURED).

The firmware was one production line, a branch of upstream's C6 firmware, in which every feature that had not earned its place stayed behind a default-off switch rather than a separate branch [FLAVORS]. Its history falls into four stages. (1) Plain upstream behaviour with time sync and beacon fixes (late August). (2) A rate-and-reliability stage from 2026-09-30: no 802.11b uplink rates, a link watchdog and an uplink watchdog, and a protocol restriction, with the fleet on one version from 2026-10-09 [App. E: F19]. (3) On-node logging, a rollback bootloader installed over the air (9 of 9 valid, from 3 of 9) and a send-health confirm for updates, rolled 2026-11-12 to 2026-11-14 [L157; App. E: F44]. (4) A peer-beacon bypass of the 1-in-8 receive gate on 2026-11-16 and an uplink watchdog for a dead IP path on 2026-11-20 [App. E: F48]. Several single-node trials of other firmware versions and an ESP-IDF version change were negative and archived [App. E: F21; FLAVORS].

### 3.3 The sink and the recorder

Nodes sent UDP datagrams to a sink on a home server, which parsed CSI, kept per-link statistics, and ran presence_v2, a per-node detector whose baselines and thresholds are learned from the node's own history with no labels. presence_v2 became the displayed house state on 2026-10-07; its first restart re-learned all nine thresholds, so earlier windows are not comparable [App. E: F27]. The sink was rebuilt on 2026-10-02, after which dropped packets fell from up to 44 % to zero [App. E: F22], and was merged with upstream on 2026-11-12 [App. E: F43].

A separate capture service wrote hourly raw packet files from 2026-09-30, at roughly 400 MB per hour with nine nodes [App. E: F18]. By 2026-11-20 the store held 1,178 hourly files, about 507 GB [L075 data]. Derived per-minute feature archives were written beside them, and a night report (a morning summary of per-node delivery, access-point crash boots and fleet frame rate) was produced automatically [L168]. The recorder had gaps: a 23 h loss after a server reboot, a roughly 3 h loss when a rack power failure took both access points and the server down [App. E: F32], and two hourly files damaged on disk, one of which was salvaged except for four holes totalling 67,374 bytes, while about 5 MB of the other was unreadable [L187]. Every raw reader skips the damaged hours.

### 3.4 The fleet's history in five phases

**Phase 1, 2026-08-27 to 09-03: build-up.** A three-node fleet became nine nodes by 2026-09-01. The sink's link-table limit was raised from 64 to 256, which made every earlier recording a different instrument [App. E: F01]. A mains interruption on 2026-09-02 rebooted both access points and swapped the audible transmitter set [L082; App. E: F04]. After 2026-09-03 the nodes were pinned to AP-A.

**Phase 2, 2026-09-04 to 09-30: instability.** Access-point minimum rates were raised on 2026-09-08. On 2026-09-23 one channel was fixed on AP-A and the 2.4 GHz radio on AP-B was turned off. Two nodes slid to 1-2 Mbps uplinks and held a large share of airtime [App. E: F06]; a batched-uplink trial left nodes crash-looping for up to a day [App. E: F10]. The 0.9.4 roll on 2026-09-30 cut channel utilisation from 80-85 % to 44-55 % and raised fleet frame rate by 26 % [App. E: F19] (MEASURED).

**Phase 3, 2026-10-01 to 10-17: stabilisation.** Watchdogs went in node by node, with no slide of 10 minutes or more per night after 2026-10-05 [App. E: F25]. presence_v2 began on 2026-10-07. The whole fleet ran 0.9.7-rc1 on 2026-10-17 [App. E: F35]. AP-A crashed repeatedly from 2026-09-23 (five times on 2026-10-04), each costing about 2 minutes of CSI on every node [L180; App. E: F14].

**Phase 4, 2026-10-18 to 11-12: disturbances.** Crashes continued. One node lost power from 2026-10-30 until it was reseated on 2026-11-08. After an AP-A crash on 2026-11-05 per-node CSI fell from 25-30 to 6-12 fps, fleet rate stayed near 80 fps for four days, and presence_v2 logged no closed minute 2026-11-05 to 11-09 [App. E: F39] (MEASURED).

**Phase 5, 2026-11-12 to 11-20: consolidation.** Version 0.9.8 rolled to all nine nodes, which were pinned to AP-A. The peer-beacon bypass raised node-to-node frames from 122 to 418 per second and fleet accepted rate from 271-274 to 471-514 [App. E: F44] (MEASURED). On 2026-11-19 a rack power loss rebooted both access points. On 2026-11-20 the fleet stood at nine nodes on 0.9.8-rc12, one access point and channel, every node on its rollback-capable bootloader, and the recorder running [App. E: F49].

**Chapter summary.** The deployment was one detached house with nine single-antenna ESP32-C6 nodes recording amplitude on one access point and channel, with an unattended sink and hourly raw recorder. The fleet changed materially in five phases, and its data were not one instrument: access-point crashes, rate collapses, a power-lost node and firmware rolls each moved the frame rate and the statistics scored on it. Later chapters therefore state which phase each test spans. The record has gaps, all counted above.


## 4. Ground truth and how it fails

Every claim in Part I is scored against a truth source, and the sources were thin. This section lists each one, what it labels, how much of it exists, and how it failed. Counts and durations are given; the times of the occupants' absences are not. Definitions are in GLOSSARY 1 and the full table in GLOSSARY 2.1.

### 4.1 The owner's rule

The owner set the product rules on 2026-09-23: no end-user labelling, no logging, no explicit calibration, and a design that needs any of these has failed. Dynamic witnessing, several movers at once and always-home pets were also fixed [L164]. The rule has a direct consequence for method. Truth sources were collected for validation only. A detector could not be trained on labels that a real household would never supply, so supervised results in Part I are upper bounds, not products. Designs that depended on prompted walks, logs or an empty house were counted as failed under the rule, whatever their score.

### 4.2 Owner-confirmed animals-only windows

These label nobody-home-but-animals-home minutes and are the only negative truth for house presence. The owner confirmed 16 windows totalling 2,630 minutes (43.8 h) on 13 days in the development cut [App. D; L153] (MEASURED). The two longest windows make up 49 % of all the minutes. The windows were outings the owner chose to log, so they are not random draws of the day, and the animals were always home, so "empty" means people-empty. A later cut of the same label file gives 21 windows on 16 days (2,060 minutes) with each window trimmed at both ends [App. D]; these are different cuts, not conflicting counts.

Two checks failed to add truth. A label-free resident-device rule selected 7 of 60 clients, of which one was a phone and six were always-on IoT devices, and marked an empty span as resident-home for 10,800 of 10,800 seconds [L143] (VOID). Phone-rule runs of AWAY were then tried as extra empty windows, and their union equalled the hand log of 16 windows, so the premise that they would add truth was false [L153] (VOID).

### 4.3 The phone Wi-Fi and BLE rule

A rule over residents' phones labels each minute HOME or AWAY. A sealed forward test over 2026-10-18 to 11-03 gave 1.31 % false AWAY and 52.2 % caught in its first half, and 1.05 % and 50.8 % in its second [L020] (MEASURED, PASS as a house-presence rule). The test had 13 empty windows (1,434 empty minutes) and 764 HOME minutes on 9 of 14 labelled days. As training truth it failed: one 110-minute stretch was labelled AWAY while a person was home, because the phone was off Wi-Fi, and a model trained on it learned that quiet means empty. Relabelling raised confirmed animals-only minutes from 654 to 867, and every candidate set still failed [L148] (INVALID-DESIGN). The rule was also rejected as a product because other occupants will not pair phones, and it was kept as validation truth only [L188]; 8 of 14 labelled days had additional occupants [App. D].

### 4.4 Plate and vehicle labels

A camera system's plate reads labelled car arrivals and departures: 79 events on 22 days, with 33 vehicle Wi-Fi sessions of 20 minutes or more. A plate read arrived within 5 minutes of the Wi-Fi session on 19 of them, and 14 sessions had no plate read [L003] (MEASURED, NOT-PROVEN). Misses clustered at dusk and in the dark, one car appeared under two labels, and departures were weak (8). These labels served as arrival and departure context only.

### 4.5 Appliance and switch moments

Moments come from the home-automation data: appliance power steps and switch presses in the room that owns them. The sealed room-activity week had 132 moments, counted per node as 107, 51, 22 and 3, with no moment on five nodes [L155] (INVALID-DESIGN). The per-node counts sum to 183 because a moment is counted for every node that owns it; all 51 moments on one node were also moments on another, because they came from one shared appliance. That appliance is itself a 2.4 GHz emitter, so it perturbs the CSI directly. Seven of nine rooms had no instrumented load in the test week. Room-level claims therefore rest on two rooms and about one appliance.

### 4.6 Wall-keypad presses

A person must press a wall keypad, and animals cannot, so a press labels a present and active person. The raw-CSI probe used 495 press minutes against 1,682 animals-only minutes [L075] (MEASURED). The two sets fall at different hours of the day: time of day alone separated them with AUC 0.729 against a design limit of 0.65, which made the probe INVALID-DESIGN regardless of its score.

### 4.7 Smartwatch night and walk marks

One night of one sleeper's per-minute heart rate labelled sleep. The node nearest the bed showed motion at 0.95 to 1.2 times its median all night, with Spearman correlation -0.07 against heart rate [L063] (MEASURED, FAIL). N = 1 and the link may cross only the foot of the bed, so this is no evidence about other nodes or beds. Walk marks gave known spots at known times: 16 scored spot-passes in one test [App. D], with about four frequency bins per link per 40-second spot. Prompted walks are outside the owner's rule, so no design could ship on them.

### 4.8 How the sources failed together

Four failure modes recur. Label defects: a wrong AWAY stretch that taught the wrong lesson [L148]. Overlap: per-node counts that exceed distinct events [L155]. Hour confounding: truth classes that fall at different hours, so a clock separates them [L075]. Scarcity: two windows holding half of the empty minutes and one room holding about 90 % of position truth [App. D]. The effective sample for most tests was the number of empty windows, 15 for the development cut and 3 held out [L152], not the number of minutes. Verdicts in Part I that depend on these sources are marked NOT-PROVEN where power was the limit.

**Chapter summary.** Truth came from seven sources, all small and all with a defect: 16 empty windows (2,630 minutes), a phone rule that passed forward but mislabelled one stretch, plate reads for 19 of 33 car sessions, 132 appliance moments concentrated in two rooms, 495 keypad minutes at different hours from the empty ones, one watch night, and a few walk marks. Failures were label defects, overlap, hour confounding and scarcity. The owner's no-labelling rule limits these sources to validation. Results against them are mostly "no evidence of a signal", not proof of absence.


## 5. Evaluation method

### 5.1 Principles

Every quantitative statement in this study carries one of three tags. MEASURED means a script and a recorded input reproduce the number. CLAIMED means the record states it but no reproducer was checked, or it is a hypothesis. SYNTHETIC means it comes from generated data, such as the end-to-end position demonstration in Part II. The tags matter because a displayed pose or vital sign can be procedural, not sensed [L127].

A test result counts only against a bar fixed before scoring. A bar was proven to predate scoring in one of two ways: a git commit containing the bar text (or its sha256) earlier than the first result, or a sealed file with a sha256 and a modification time earlier than the first score. Commit proof is independent of the file store; file-time proof is weaker, because modification times can be changed, though nothing in the record shows they were [App. C]. Of 24 bars examined, 18 are proven by commit or sealed file, 4 are claimed only (record or file time), and for 2 the original text is gone [App. C].

The weak ones are named here. The positions re-score bar was a file written 12 s before the score [App. C]. Two early bars (the witness rule and the hold rule on the first bar days) were rewritten with results appended, so the original text is gone; the redesigned versions were openly made after reading those results, then sealed before scoring on new days [App. C; L015, L016]. In one test (R13) the day-counting changed after the first result: days with under half their minutes kept by the RF-quality mask were reported but not counted, while the 0.80, 5 % and 70 % thresholds never changed [App. C]. A five-minute home/away variant was also chosen after scoring [L151].

### 5.2 Splits and baselines

- **Held-out days.** Leave-one-day-out (LODO): each local day is scored by a model trained on the other days. Later tests used sealed weeks or decision days.
- **Shuffled-label null.** The pipeline is re-run with labels permuted; the control model's score is the null (five seeds for the neural net [L074]).
- **Block shuffle.** Within each day, labels are permuted among 10-minute blocks, keeping the base rate and short-range autocorrelation (100 nulls; the 99th percentile is the bound) [L075; App. D].
- **Family-wise null.** The empty-window set is circularly shifted by whole days; each shift keeps the maximum |S| over every (feature, node) test, and a test survives only if it beats the largest of these maxima [L021-L023].
- **Non-RF baselines.** Time of day alone, traffic counts, the phone Wi-Fi rule, and whole-house power [L152; L024; L025]. A radio method is credited only if it beats these.
- **Same-hour controls.** The same hit test applied at other minutes of the same hours of day, away from truth moments, so that a time-of-day pattern is not scored as sensing [App. D].

### 5.3 Metrics

| Metric | Unit | Where definitions differed |
|---|---|---|
| Detection | share of waking HOME minutes called present, per day, then median over days with at least 60 HOME minutes | Pooled in some early tests; undecided minutes counted absent [App. D; L010] |
| False presence | share of owner-confirmed animals-only minutes called present, pooled | Bar 0.05; first 30 minutes after a departure included (excluding them moves 0.192 to 0.147) [L018] |
| Waking AUC | per held-out day, HOME above AWAY, 07:00-23:00, then median | Masked and unmasked differ; pooled AUC lets time of day count as signal and is not comparable [App. D; L075] |
| Catch rate | truth moments with a qualifying event within +-2 min | Differs from the phone test's "caught" (share of empty minutes called empty) [App. D] |
| Lift | catch minus same-hour base (difference), or catch divided by base (ratio) | Two meanings; state which [App. D] |
| False events per nobody-there hour | events per hour in minutes when nobody can be in the room | Hours came from confirmed windows plus assumed-empty nights [App. D] |
| Paired fraction | paired transmissions over finalised transmissions | Later judged the wrong figure of merit; paired observations per second fell 44 % while the fraction rose [L091] |

### 5.4 Truth supply and power

| Truth source | Labels | Amount | Defects |
|---|---|---|---|
| Owner-confirmed animals-only windows | nobody home, animals home | 16 windows, 2,630 min, 13 days (dev cut); two windows are 49 % of minutes | one house; windows are owner-chosen outings; animals always home [App. D] |
| Phone Wi-Fi/BLE rule | HOME/AWAY per minute | 9 of 14 forward-test days labelled | one 110-min stretch labelled AWAY while someone was home [L148] |
| Appliance and switch moments | own-room activity | 132 test-week moments; 7 of 9 rooms have none | per-node counts overlap; one shared appliance emits at 2.4 GHz [L155] |
| Wall-switch presses | person present and active | 495 min | clustered at different hours from the empty windows [L075] |
| Smartwatch night | sleeper heart rate | 1 night, 1 subject | no replication [L063] |

Minutes inside a window are strongly autocorrelated, so the effective sample is the number of windows, not hours [L152]. Five tests could not have certified a pass (MEASURED or derived from recorded counts, as marked) [App. D]:

- The family-wise screens: 15 shifts floor the attainable p at 0.0625, so no single test could reach 0.05 [L021-L023].
- The node-3 precision pass: with 9.2 nobody-there hours, a rule exactly at the bar shows zero false events about 40 % of the time (derived).
- The blind week: 3.03 nobody-home hours against the 10 required [L039].
- The positions bypass re-score: about 38 % power to detect the +0.10 improvement the bar demanded (derived) [L050].
- The one-night watch test: n = 1, effective sample perhaps 5 to 10 (derived) [L063].

Large-gap failures (0.1 to 0.6 below bar) are secure, but none of those presence tests could have certified a pass at 5 % false presence with eight or fewer windows [App. D]. The distinction used throughout: **evidence of no signal** requires a test with enough power to detect a stated effect that then saw none; **no evidence of a signal** is what remains when a test saw nothing but could not reliably have seen the effect, or when no effect size was stated. Most results here are the second kind. Later chapters refer back to this paragraph.

### 5.5 Rules learned from our own mistakes

| Rule | Produced by |
|---|---|
| Never cite a lift without a room-shuffle or confusion check; truth was mostly one room | L156, L050 |
| Match labels to the time of day of the comparison class; empty windows and presses fell at different hours (clock alone scored 0.729) | L075 |
| Client-simulated or demonstration values are default-off and labelled "not measured" | L127 |
| Check window comparability after a power event; the transmitter set swapped wholesale | L082 |
| Use only links describing the same instant; window spans ran 1.2 to 3,494 s | L128 |
| The build under test must be the fleet configuration (silent 4 MB image) | L132 |
| Phone-derived labels need confirmation before scoring; one mislabel produced the supervised edge | L148 |
| Offline archive features must be replayed through the live path; the "2x catch" was a cold-start artefact | L149 |
| Run `cargo test`, not `cargo check`, on each branch in isolation | L131 |

### 5.6 Who did the analysis

The analysis, the scoring code and much of the record were produced by LLM coding agents (Claude Code sessions) directed and reviewed by the owner. The guard against analyst degrees of freedom is the one described above: bars committed or sealed before scoring, deviations listed with each result, and held-out data. That guard is imperfect, as section 5.1 records.

**Chapter summary.** Every number is tagged MEASURED, CLAIMED or SYNTHETIC, and a result counts only against a bar fixed first; 18 of 24 bars are proven so, and the six weak ones are named. Splits are held-out days, with shuffled-label, block-shuffle and family-wise nulls and four non-RF baselines. Metric definitions differed between tests and are tabulated. Truth was scarce enough that five tests could not have certified a pass, so most results are "no evidence of a signal", not "evidence of no signal". Nine rules came from the study's own errors.


## 6. RQ1: Is anyone home?

House presence was the first question and the one with the most truth. Sixteen owner-confirmed animals-only windows give nobody-home minutes; a phone rule gives home and away labels (section 5.4). The bar was fixed at median detection of at least 0.70 on waking home minutes with pooled false presence of at most 0.05 on those animals-only minutes [App. C; L019]. No radio method met it.

### 6.1 What the shipped system reported

The sink's house presence flag read true in 99.78 % of minutes in an empty house and 99.89 % in a time-matched occupied one; the firmware per-node flag read 84.1 % empty and 84.3 % occupied [L005] (MEASURED). In an animals-only window the live sink output said presence=true on 6,098 of 6,101 lines, with 2 to 20 persons counted [L006]. Across a full home/empty comparison the node presence flag was true 100 % of the time in both states, and edge motion energy was higher when the house was empty [L007] (MEASURED). Scored against home/away truth, the label actually displayed to the user was "present" in 100 % of minutes, an AUC of 0.50; three label-free candidates reached median AUC 0.71 to 0.78 and so beat it [L012] (MEASURED). That PASS is against the live label only; the same candidates still failed the 0.05 bar (section 6.2).

### 6.2 The method sequence

The R-numbers are the pre-registered tests of October 2026. "Bar fixed" follows section 5.1 and [App. C]. FP is false presence on confirmed animals-only minutes; "n/a" means the test did not report that quantity.

| Test | Features / model | Data | Bar fixed? | Detection | FP | Verdict | Ledger |
|---|---|---|---|---|---|---|---|
| Full-rate capture | four statistics on every AP link, day 1 learn, day 2 score | 49 h pcap | yes (commit) | n/a | n/a | FAIL: adjacency 519 vs null 561.0 | L001 |
| RSSI sd | per-link RSSI spread | 2 windows | no (exploratory) | n/a | n/a | NOT-PROVEN: empty-halves control p=0.033 and 0.002 | L002 |
| Clean transition | firmware motion; amplitude variability | one same-day change | no | AUC 0.45 / median 0.77 | n/a | NOT-PROVEN | L008 |
| R13 A and B | two amplitude features; label-free vs supervised | 4 dev days, LODO | yes (commit) | A 48-54 %, B 71 % pooled | 85 % / 84 % pooled | FAIL: waking AUC median .57 / .58 | L009 |
| R14/R15 mask, band | RF-quality mask; 0.3-3 Hz band energy | same days | yes (R13 bar) | A masked 16-25 % | A masked 4-7 %, "not real" (51 % undecided) | FAIL | L010 |
| R18 | clean upper tones kept vs dropped | 3 primary held-out days | yes (commit) | FG A 42-44 % | 28-44 % | FAIL | L011 |
| R17 | lower-segment frames, steering states | 4 dev days | claimed only | A, B masked | A 15 %, B 31 % (masked) | FAIL: waking AUC median B .78 | L013 |
| R23 strong-node hold | 15-min hold | bar days, then decision days | bar days: no; decision days: sealed | +9.6 pts (S alone) | D adds +4/+25/+42 pts | FAIL (decision days VOID) | L015 |
| R22 witness vote | weighted house vote | bar days, then decision days | bar days: no; decision days: sealed | gain +0.00 (bar +0.10) | unchanged | FAIL (decision days VOID) | L016 |
| Holdover | hold T = 0/30/60/120 min | one look | not in App. C | .57/.91/.95/1.00 | .48/.87/.93/.93 | FAIL | L017 |
| R24 legacy frames | non-HE frames, per-stream dispersion | test week of 7 days | yes (commit) | median 0.601 | 0.192 | FAIL | L018 |
| R9 presence_v2 | the production algorithm | held-out week | yes (commit) | median 0.62 | 0.63 | FAIL | L019 |
| Family-wise screens | RF features per link; RSSI/noise/gain; motion band | up to 15 empty windows | same-evening commits | n/a | n/a | FAIL: 0 survivors in 155 tests | L021-L023 |
| Adaptive classifier | trained model, earlier 3-board fleet | its own training set | n/a | accuracy 0.486 (chance 0.25) | n/a | FAIL | L071 |
| R16 neural net | 1D-CNN on 8 min x 9 nodes | 3 held-out days | yes (commit + sealed) | 0.612 | 0.466 | FAIL (chapter 11) [App. B: M19] | L074 |

The pattern is consistent. First, false presence on confirmed animals-only minutes never came near 0.05. The best unmasked pooled result was R24's 0.192 [L018], which falls to 0.147 when the first 30 minutes after a departure are excluded; the production algorithm scored 0.63 [L019]. The masked label-free arms that showed 4 to 7 % did so because 51 % of the animals-only minutes were undecided and counted absent, which the record itself calls not real [L010]. The failures are secure (section 6.5).

Second, the direction of the signal changed from day to day. The same two features gave waking AUC .50, .76, .14 and .65 on four days [L009] (MEASURED). The firmware motion feature scored 0.45 on a clean same-day transition [L008], and an RSSI spread that differed between occupied and empty also differed between two halves of an empty period [L002]. Chapter 12 explains the likely mechanism: level features follow channel state and household traffic, which follow occupancy hours, so a feature can correlate with occupancy for reasons unrelated to a person (12.2, 12.7). That is an inference, not a measured decomposition.

Third, repairs did not close the gap: masks, frame-segment choices, a hold, a witness vote and legacy frames each left the result 0.1 to 0.6 from the bar.

### 6.3 The wider method set

METHODS-27 rebuilt the "about 27 earlier presence methods" from the record. It found 30 distinct scored presence-detector configurations (M01-M30), which become at least 60 arms when A/B and masked/unmasked variants and the 18 neural-net development configurations are counted [App. B]. The unmasked, pooled or primary-day false presence ran from 0.19 (R24, M28) to 0.94 (R13 supervised on one day, M12); the range is a summary of those rows, not a computed minimum and maximum, and some single-day values reached 1.00 [App. B]. None met the bar.

A second inventory, compiled on 22 October by a records-search agent and never written into the record, was recovered from the session transcripts: 39 approaches, of which 27 were "run on a subset of 3-7 chosen days", 7 paper-only, 5 built with no scored run, and none run on the whole capture [App. B]. The two lists do not match. That inventory's 27 include room-activity, position, desk and home/away studies and omit the ten early probes (M01-M10) that open METHODS-27.

The earlier "27 scored" count in this study's own record was therefore a grouping of rows. It does not reproduce as 27 detectors, and no source states which 27 were meant [App. B; App. B]. What both lists support is the same: many configurations were scored, none was run on the full capture, and none passed a bar with a detection and a false-presence term.

### 6.4 Non-radio baselines

Signals the house already had did better. A phone Wi-Fi/BLE rule, sealed before a 14-day forward test, called 1.31 % of known-home minutes away (10 of 764) and caught 52.2 % of empty minutes (749 of 1,434) in its one-minute form; the five-minute variant gave 1.05 % and 50.8 % [L020] (MEASURED). That is PASS as a forward test. Its limits are real: 9 of 14 days were labelled, the five-minute variant was chosen after scoring, and the owner rejected it as a product because other occupants would not pair phones.

Whole-house energy use separated empty from occupied hours with AUC 0.75 raw (interval 0.57 to 0.86) and 0.87 when divided by the same hour on other days; recall at 5 % false-empty was 0.03 and 0.30 [L025] (MEASURED). As a rule to catch the phone rule's failure it did not work: it called 30 of 33 empty hours empty but also 83 of 374 occupied hours (22 %, bar 5 %) [L024]. Car arrival and departure labels from plate reads and from vehicle Wi-Fi were NOT-PROVEN: plate arrival matched within five minutes on 19 of 33 sessions [L003].

A radio method is credited only if it beats these baselines (section 5.2); none did.

### 6.5 Power and confounds

Which failures are secure. The gaps for R9, R24, R18 and R13 are 0.1 to 0.6 below the bar, and the 95 % half-widths on the pooled false-presence rates (about 0.03) are far smaller than the gap to 0.05, so those verdicts are not sampling accidents [App. D]. Which tests could not certify a pass: none of them could have shown at most 5 % false presence with eight or fewer windows, since zero false minutes in eight windows bounds window-level false presence only at 31 % [App. D]. The three family-wise screens could not reach p below 0.0625 on any single test, because 15 shifts set that floor, and a feature had to move by about one standard deviation of its own window-to-window spread to survive [L021-L023; App. D] (section 5.4). Their result is no evidence of a signal, not evidence of none.

Fleet changes. The study's per-test fleet-state record flags every presence test in this chapter as spanning at least one fleet or AP change. The R9 week (10-08 to 10-14) spans the link-fix rollout and AP radio crashes [App. E: F14], and the R24 test week spans four further major rows [App. E: F29]. This does not rescue any result: the failures are 0.1 to 0.6 from the bar, and no recorded change would turn 0.63 false presence into 0.05. It does mean the tests are not repeats under one fixed condition, and that is stated here.

**Chapter summary.** The shipped presence flag read true in empty and occupied houses alike, and the displayed label scored AUC 0.50. Every radio method scored against the bar failed it, with unmasked false presence on animals-only minutes between 0.19 and 0.94 against a bar of 0.05; per-day AUC changed direction across days. The record's "27 scored methods" was a row grouping: 30 scored configurations and a separate 39-approach inventory exist, and none ran on the whole capture. A phone rule and whole-house power outperformed the radio. The large-gap failures are secure, but the screens and any pass at 0.05 false presence were beyond the supply of truth, and all tests spanned fleet changes.


## 7. RQ2: Is this room active?

### 7.1 Why the target moved from presence to room activity

Chapter 6 found no usable house-level presence signal. In mid-October the working target was therefore reframed from "is anyone home" to "is this particular room active", with the claim limited to high-certainty events from one node at a time. The reasoning was that a rule allowed to stay silent most of the time could be precise even if it was insensitive [L125] (CLAIMED; a reframe, not a result). The reframe was falsified in practice by the tests below and was superseded on 2026-10-29 [L125]. Conventions are in Chapter 5; channel confounds are in Chapter 12.

### 7.2 Truth for rooms

A room-level test needs moments when a person was known to be acting in a known room. The only such labels the house supplies are appliance power steps and wall-switch presses, which mark own-room activity but say nothing about a person who is present and still [App. D]. Everything in this chapter is bounded by that supply.

The sealed test week (2026-10-21..10-27) held 132 moments. Per-node counts were 107 (node 5), 51 (node 6), 22 (node 8) and 3 (node 3), and the other five nodes had none [L035, L155] (MEASURED). These counts overlap. They sum to 183, and 183 minus 132 is 51, the node-6 count, so every node-6 moment is also a node-5 moment: one appliance shared by two rooms (derived, App. D). That appliance also emits at 2.4 GHz and so perturbs the channel by itself [App. D]. Seven of nine rooms therefore had no truth at all [L155]. A claim about "the house" cannot be confirmed from this supply, and a catch rate on one appliance is not evidence about people. Section 5.4 lists the other truth sources.

### 7.3 The sealed H3 test

**Path to the seal.** The draft burst rule fired when a node's amplitude rose above a margin for N consecutive minutes. In development it gave 0.00-0.11 false events per hour on five nodes, but events covered only 1-6 % of awake minutes and 0-14 % of switch-ons [L031] (MEASURED, NOT-PROVEN). A looser per-node rule reached a node-5 catch of 0.86 against a base of 0.39, yet no rule held false events at or below 0.1 per hour [L032]. On the fixed sink the draft rule produced no false events and no catches: node 5 caught 0 of 11, node 6 0 of 3, node 3 0 of 1 [L033]. The sealed candidate excluded node 5 (3 false events in 15 night hours), and node 6 had caught 3 of 10, all on one reference board and 0 of 7 on the AP link [L034]. Because the archive's offline features had started cold, node 5 showed 34 switches against 8 on the sink, and reference arms caught 0, 1 and 1 of 7 on node 6 and 0 of 20 on node 5. The early catches were an artefact of that cold start [L149] (INVALID-DESIGN, MEASURED). On 2026-10-18 the record fixed a pick rule before looking at catches and chose option C, a precision-only claim: catch metrics were reported, not graded, and a firing floor of 0.05 events per hour was added [L036] (MEASURED).

**Rule and seal.** One burst rule per node, with the false-event bar at no more than 0.1 per nobody-there hour, was sealed on 2026-10-20 and committed before the first test day [App. C].

**Result.** Nodes 0 and 3 PASS: zero false events over 37.1 h and 9.2 h of nobody-there time respectively. Nodes 4 and 7 FAIL, with false rates of 0.217 per hour over 9.2 h and 0.162 per hour over 37.1 h. The rest were NO-CLAIM or untested [L035; App. C] (MEASURED). Catches were near nil: node 5 2 of 107, node 6 1 of 51, node 8 0 of 22, node 3 0 of 3 [L035].

**Why the pass is not a detection.** The record's own reading is that the rule "passed by never firing; it caught nothing" [App. C]. A rule that never fires has no false events by construction. The precision test is also weakly selective. If a rule's true false-event rate sat exactly at the 0.1 bar, the chance of seeing zero events is exp(-0.1 x hours): 0.40 for node 3's 9.2 h and 0.024 for node 0's 37.1 h (derived, Poisson; App. D). Node 3's pass would therefore occur about 40 % of the time by chance at the bar, and a zero-event pass needs about 30 h to carry a 5 % chance. Only node 0 had an adequate window, and 28 of its hours were assumed-empty nights, not confirmed [App. D]. Verdict: PASS as written, with no detection value. The week also saw AP-A radio crashes, about 2 minutes of zero CSI each [App. E: F36].

### 7.4 The blind analyst and the confirmation week

To remove the human from rule search, a sandboxed analyst saw 28 days of node features and the truth moments, and was asked for a rule that beat its own null. It found one on node 5 with a catch rate of 0.176 against a base of 0.034 and 0.024 false events per hour on development data [L038] (SUPPORTED, MEASURED). The seal rehearsal on held-out empty windows gave 0.46 false events per hour, far above the 0.1 bar, so the sealer predicted the guard was at risk [App. C; L038]. Its development family-wise p was 0.0667 across 521 looks [App. C].

The sealed week (2026-11-03..11-09) used that rule with a one-sided binomial bar at p of 0.01 and a guard of 0.1 false events per nobody-home hour over at least 10 hours [App. C]. The verdict was NOT-PROVEN. The rule caught 0 of 99 graded moments against a base of 0.0042; three catches would have passed, so zero shows no response, not a small sample [L039; App. D] (MEASURED, derived). The 40 microwave moments were reported and not graded: 0 of 40 [App. C]. The guard could not be tested: 3.03 nobody-home hours of the 10 needed, with zero false events (P0 at the bar 0.74, derived) [L039; App. D]. Only 70.5 of 168 hours were covered. Node 3 was unpowered from 2026-10-30 to 11-08, and an AP-A crash on 11-05 cut per-node CSI from 25-30 to 6-12 frames per second [App. E: F39]. Detection is a clean negative for this rule; the false-alarm half is unproven.

### 7.5 Other room-level probes

An own-room event screen compared each node and feature against a family-wise null. Room 5 had null maximum 5.80 and 0 survivors; rooms 5 and 6 together had 11.29 and 0; room 8 was not tested [L037] (FAIL, MEASURED). A person seated at a desk was not seen: development AUC 0.32 and 0.43, test 0.60 and 0.55 against a 0.70 bar [L064] (FAIL, MEASURED). Co-receiver grouping, meant to show separate movers by covarying disturbances, found 0 groups on a real capture after centring cut the pooled correlation from +0.214 to +0.072 [L121] (NOT-PROVEN, CLAIMED). The related co-denting premise was falsified; see 12.6 [L122]. A room-by-room walk could not be graded, because it began 0-3 minutes after a restart and visits were under a minute [L146] (INVALID-DESIGN, MEASURED).

**Chapter summary.** Room activity was tested against appliance and switch truth covering two of nine rooms and effectively one shared appliance. The sealed rule passed its precision bar on two nodes only by almost never firing, with node 3's pass about 40 % likely by chance, and caught 3 of 183 attributed moments across nodes 5, 6, 8 and 3 (derived from [L035]). The blind confirmation week caught 0 of 99 and could not test false alarms (3.03 of 10 h). This is no evidence of a signal for these nodes and this truth, not proof that rooms are unobservable; the other screens failed, were not proven or were not gradable.


## 8. RQ3: Where is the person?

Position is the hardest of the questions, and the evidence is the most one-sided. No test in this chapter showed the fleet placing a person, in a room or on a floor, better than a stated chance baseline. Several tests were also too small or badly designed to say more than that (the power definitions of section 5.4 apply).

### 8.1 Early position work on the first boards

The first attempts used the earliest boards and a handful of access-point links. Scalar per-node energy weighting, with three AP-to-node links inside a 36.5 degree fan, gave a clean negative: magnitude tracked proximity, direction did not, and the AP supplied only 34 % of frames on one node [L040] (FAIL, MEASURED).

The one positive result was supervised. Nearest-neighbour fingerprints among five known spots classified 6 of 10 positions, with a mean error of 27 in and a between-to-within L2 ratio of 2.54 [L041] (SUPPORTED, MEASURED; one room, labelled data, so it says nothing about unlabelled operation). A walk survey then asked whether the measurement field is smooth enough to reconstruct: Spearman rho +0.273 (p 0.542), leave-one-out inverse-distance weighting +6.3 % (p 0.133) and leave-one-out nearest neighbour -39.8 % [L043] (NOT-PROVEN, MEASURED; undersampled). Ratio normalisation of marked walks scored 18 % against 27 % for subtraction and 9 % raw, with a null of 50 % (n = 11 of 28) [L045] (NOT-PROVEN, MEASURED). The Doppler centroid position tier had a deadband narrower than its bin spacing, so static energy read as motion (static as a share of moving 6.2 % before the fix, 0.45 % after, SYNTHETIC); the live metric nevertheless stayed near 2.5 to 2.6 of 3.0, saturated [L044] (BUILT-WORKS as a regression test; position metric BUILT-FAILED).

### 8.2 The link-line model and why it failed

Radio tomography assumes that response falls with distance from the line between a transmitter and a receiver. In one room with three co-located sensors the opposite held: response was inverse to link-line distance (|z| 3.8 at 11 in, 13.0 at 47 in), the link-line solve gave 58 in mean error against 49 in for always answering a fixed point, and AP links carried no position information [L118] (FALSIFIED, MEASURED; scope one room). Section 12.6 gives the physics.

A second defect hid in the solver's input. The solver pooled links as if they described one instant, but window spans ran from 1.2 s to 3,494 s (median 165.8 s) and only 27 of 155 links were under 20 s [L128] (INVALID-DESIGN, MEASURED).

### 8.3 Radio tomography on the fleet

The fleet estimator, est-rti-v1, used about 49 links (40 node-to-node, 9 AP) on a 0.3 m grid, with up to three blobs. Its constants were tuned on 14 October empty windows plus one prompted walk, not held out. Scored on 7 to 17 October, the baseline room hit rate was 0.34 against a control of 0.23, and the estimator's room-moment lift was +0.15 (n = 36) against +0.149 for a node-only baseline. Floor separation also failed: the empty second floor drew 86 % of its home blob rate, mostly from one link, and the basement, with no node, was never placed [L046] (FAIL, MEASURED).

On days after the tuning the lift disappeared. Across 107 moments there were 13 hits (0.12) against a control near 0.07, a lift of +0.04, and the empty-house to home blob ratio was 0.98 [L049] (FAIL, MEASURED). Thirteen hits is exactly the 5 % threshold at that n and control, and the smallest detectable lift was about +0.05 [App. D], so this is chance-level, not a proven zero.

### 8.4 More node-to-node traffic

The estimator's links are starved, so a pre-registered experiment proposed cutting the node beacon interval from 320 to 80 ms [L047]. It never ran as designed. Only two nodes were pushed, about 11 h after the sealed time; the fleet push never happened. Per-pair beacon CSI rose by x1.49 to x1.65 against a bar of x3 [L154] (FAIL, MEASURED); the receive gate was full of AP frames, so the extra beacons were mostly dropped. A simulation also showed 46 % false-blob steps with 80 ms links unless the motion gate scaled with link count [L047] (SYNTHETIC; the proportional law came from the simulation's own noise model).

The real cause was in firmware: the receive gate kept only one frame in eight. A bypass for peer beacons raised node-to-node frames from 122 to 418 per second (x3.3) and pairs at 2 fps or more from 24 to 39 [L117] (BUILT-WORKS, MEASURED). The bar, hashed before scoring, was that the room-moment lift rise by 0.10 and exceed 0.12. It did neither: lift was 0.197 with the bypass off (102 moments) and 0.197 on (60 moments), and the nearest-node baseline from phone Bluetooth was 0.450 in both [L050] (FAIL, MEASURED). The 0.197 was no weak localisation either. Truth was about 90 % one room (90 of 99; 49 of 60), the estimator parked its strongest blob in one room regardless, and outside that room 1 of 9 moments (off) and 0 of 11 (on) were correct [L050; L156] (INVALID-DESIGN for the lift reading, MEASURED).

The test had about 38 % power to detect the +0.10 improvement it demanded [App. D]. The finding is therefore no evidence that tripled traffic helped, not evidence that it cannot.

### 8.5 Is the software the limit?

A synthetic end-to-end check asked whether the chain itself, from frame extraction through estimation to the floor-plan output, works when its input does carry a position signal [Part II, synthetic demonstration] (SYNTHETIC; Part II gives the detail). A made-up two-floor, six-room house with nine nodes and one scripted walker went through the real, unchanged scripts. Thresholds were fixed before any arm was generated.

Arm A used the estimator's own physics (response falling with link-line distance, a 30 % dip) and was circular by design. It failed its pre-registered bar: room hit rate 0.444 against 0.60 required, chance 0.178 [Part II, synthetic demonstration] (FAIL). It was correct wherever it drew a blob (median error 0.76 m) and the blob gate stayed silent when nobody was present; it simply drew too rarely, on 0.438 of steps. Arm B used the measured inverse response from [L118] and had no bar: room hit rate 0.056, below chance, with a median error of 5.27 m [Part II, synthetic demonstration]. A post-hoc arm A2 with a 60 % dip scored 0.889, and is reported as post-hoc [Part II, synthetic demonstration].

The reading is limited to three statements. The chain is correct when it fires. Its sensitivity is bounded by a gate tuned to real noise. Under the physics the fleet measured, it is wrong. Arm B's extrapolation beyond 1.19 m is an assumption of the demo [Part II, synthetic demonstration].

### 8.6 Placement with other hardware

Whether other hardware would place a person here is a HYPOTHESIS (CLAIMED), not a finding. Its support is the supervised fingerprint result [L041] and published multi-antenna, phase-coherent work in lab conditions. Neither was tested on this fleet, whose nodes are single-antenna, so they have no cross-antenna phase. The owner's budget excluded new hardware (chapter 15), so the hypothesis stays open.

**Chapter summary.** Across early weighting and fingerprinting, tomography, and a tripling of node-to-node traffic, no test showed the nine-node fleet placing a person above chance, and the apparent lifts were room-bias artefacts. The one positive result was a supervised fingerprint in one room [L041]. The traffic experiment never ran as pre-registered, and its bypass re-score had about 38 % power, so it is no evidence of a signal, not proof of absence. The synthetic demonstration shows the software is correct when it fires but that real physics defeats it. Other hardware is an untested hypothesis.


## 9. RQ4: Still people, breathing and vital signs

The fourth research question asked whether the nine nodes could see a person who is not moving: a sleeper, a seated worker, a chest rising and falling. The answer from this study is that no test showed it, and that the shipped breathing pipeline was never given a fair chance to show it. The first was a property of the instrument (12.5); the second was an engineering state, since repaired, in which accuracy was still not tested.

### 9.1 The change detector

The per-link motion metric is a change detector. A seated or sleeping body is absorbed into its baseline within 3-8 s, and the raw-motion alternative separated rooms worse (Mann-Whitney U 12 of 20 against 16 of 20) [L051] (MEASURED; mechanism in 12.5). The overnight test followed from that. Motion level was asked to separate occupied sleeping rooms from empty rooms; U was 16 of 20 against a critical value of 19, the occupied rooms ranked 2-5, and an empty living room ranked first [L052] (MEASURED, NOT-PROVEN).

Holding a presence verdict for T minutes is the obvious patch for still sleepers, and it fails on the other axis. For T of 0, 30, 60 and 120 min, home detection was .57, .91, .95 and 1.00, but false presence while away was .48, .87, .93 and .93 over 344 away minutes [L017] (MEASURED, FAIL). The hold moves the operating point along a curve; it does not improve the curve. The recorded cause was that the bottleneck is false presence from animals and radio-frequency changes, not memory [L017].

### 9.2 The breathing pipeline as shipped

The shipped breathing path needs 30 s of uninterrupted samples. On 6 of 9 nodes the window was never full, and it cleared 1.7-15.6 times per minute [L053] (MEASURED). The cause was a retune loop: the rate estimate wandered and each correction discarded the buffer. Replay against live showed the retune sawtooth was real bursts, with replay-to-live rate ratios of 0.87-1.48x on 7 of 8 nodes and a maximum CSI gap of 10,662 ms on node 0 [L055] (MEASURED).

Two fixes worked as engineering. A slow moving-average rate estimate cut clears from 10.4-14.4 to 0.00-0.30 per minute offline, and live windows were full 92-99 % of the time [L056]. Resampling instead of discarding, plus a 300 s dwell gate, cut replayed fires from 584 to 151 and live clears about 42x, to 0.583 per hour [L058] (MEASURED, BUILT-WORKS). Neither fix tested whether the resulting breathing estimate is accurate; no reference breathing rate was ever compared against it.

The cause of the clears was also not people. In a natural experiment, clears per hour were 27.2 with the house empty and 10.0 or 36.1 in two occupied periods, 9.3 while occupants slept [L057] (MEASURED, SUPPORTED). The retune chased WiFi traffic, not bodies, so the empty house produced more clears than most occupied periods.

A diagnostic error belongs here. The vital-signs endpoint reported a buffer of 0/300 and the authority abstained. That buffer belonged to a detector in the shared server state that nothing on this hardware feeds; the per-node path was live, and publication was suppressed because no field model had ever been built [L136] (MEASURED). The first framing, why the sample count was zero, was INVALID-DESIGN and cost a session. A related rate question, whether the 8 Hz edge DSP rate was a defect, was answered no: edge rate 7.96-7.99 Hz against a CSI rate of 22-29 Hz, by design [L158] (MEASURED).

Authority to publish vitals needs a calibration that cannot run here. It requires 600 s of an empty house, repeated every 12 h, with a baseline that expires at 86,400 s [L054] (MEASURED/CLAIMED). With animals always home (5.4), the precondition is never met. The route chosen for giving vitals authority in a home with animals produced no result [L059] (CLAIMED, OPEN).

### 9.3 Direct tests

Breathing-peak test. Windows with a spectral peak-to-background ratio of at least 4 were 8 % of master-node windows in the occupied night and 19 % in an empty period; the steady-peak share was about 20 % in both, and median rates were 20-26 breaths per minute in both [L060] (MEASURED, NOT-PROVEN). A sleeper did not make the breathing peak more common.

Shape-change tests. A rule that still people appear as link-shape change against empty failed: occupied ratios of 1.3-19 against controls up to 51 [L061] (MEASURED, NOT-PROVEN). A pre-registered still-person model, asserting that link shape changes if and only if an endpoint room is occupied, scored AUC .62, .88 and .75 on three cases, with room calls right in 0 of 7, 2 of 7 and 0 of 7, and only 6-8 links judgeable [L062] (MEASURED, FAIL).

The watch night. A smartwatch gave a heart-rate truth for one sleeper. The node nearest the bed held motion at 0.95-1.2x its median all night (maximum 1.29), and its Spearman correlation with heart rate was -0.07 [L063] (MEASURED, FAIL). The respiration-window share was 9 % asleep against 15 % awake [App. D]. This is n = 1: one night, one subject, and perhaps 5-10 effective samples, for which a correlation would need to exceed 0.64 in magnitude to be significant [App. D]. The test also could not separate a blind node from blind geometry, since the link crosses only the foot half of the bed [App. D]. It is a lead, not a proof (5.4): no evidence of a signal, not evidence that none exists.

### 9.4 The stock fall detector

The firmware's stock fall detector was enabled by default. Overnight it logged 13 "Fall detected" events across 5 nodes while nobody fell [L065] (MEASURED, FAIL). It was disabled, and no such events followed.

**Chapter summary.** Still bodies are absorbed by the change detector in 3-8 s, and holding presence to compensate buys detection with false presence. The breathing pipeline was repaired, with clears about 42x lower, but vitals accuracy was never tested, and clears tracked WiFi traffic rather than people. Direct tests of a breathing peak, still-person shape change and a watch-referenced night all found no signal; the watch night is n = 1 and underpowered. The stock fall detector produced only false alarms and was disabled.


## 10. RQ5: Pets are always home

### 10.1 Why it matters

In this house, "empty" never meant "nothing moving". The only empty-house truth available is "no people, animals present" [App. D], and the animals were home in every labelled window (section 5.4). Any feature that responds to variability responds to animals as well as people. The empty-room baselines in the literature contain no animals, so a published false-presence rate measured against them does not transfer to a home with pets. The study also could not ask the household to remove the animals, stage empty-house sessions or label walks; that constraint is set out in chapter 15. The question here is how much of the false presence the animals explain.

### 10.2 Results

The live sink reported presence on 99.95 % of lines in an animals-only window, with 2 to 20 persons counted [L006] (MEASURED). The two amplitude features gave animals-only false presence of 37-93 % and 67-94 %, with median waking AUCs of only 0.57 and 0.58 [L009] (MEASURED).

The decisive comparison is per node. Three rooms the animals never enter, with closed doors, fired about as often with the house empty as occupied: .49 against .44, .38 against .45 and .46 against .45 [L067]. Only node 6 discriminated (.14 against .52), and its away minutes were all daytime [L067] (MEASURED). A pet cannot explain a node the pet cannot reach.

A later analysis of 1587 animals-only, 4453 awake-home and 1795 asleep minutes found the score above threshold in 40 %, 51 % and 63 % of them [L068]. The nodes nearest the animals were not the worst offenders on empty minutes (70 % and 68 %). AP channel statistics correlated at |rho| <= 0.2, and heating and cooling ran in 34 % of awake but 10 % of empty minutes while nodes fired less during operation [L068]. Pets, AP radio and heating are FALSIFIED as drivers; the driver is unknown (MEASURED).

Separating person events from animal events per node failed its pre-stated bar. Band loudness reached an AUC of 0.86 on node 5 (event CI 0.71-0.97) and 0.81 on node 6 (0.60-0.97) [L069]. Node 6's person truth was entirely microwave use, so that figure may measure the appliance, and node 5 had 14 person events on 4 days. Duration, peak and co-firing did no better (0.63, 0.48, 0.44 on node 5) [L069] (MEASURED). This is a truth-scarcity failure more than a proof of inseparability.

Whether animals trigger the animal-room node at night is NOT-PROVEN: it made 4 events in one night of about an hour while sleepers' nodes stayed quiet, and every confirmed animals-only window was daytime [L070] (MEASURED).

One mitigation was built. A within-window activity ratio (bar 3.0) never fired on real nights, the highest node reaching 2.32; a quiescence rule passed 16 of 51 windows against 0 of 51 for the old rule [L066] (SYNTHETIC/MEASURED). It was dropped as superseded and never proved a pet-occupied node detectable on real data.

### 10.3 Reading

Animals are a real confound, since a learned empty baseline taken while they are home is not empty, but they are not the main one. Rooms they never enter showed the same false presence, and the dominant driver is the channel and the instrument (chapter 12). These results are mostly no evidence of a signal rather than evidence of none (section 5.4). The constraint appears chiefly as truth scarcity: person truth on only a few nodes, animals-only windows only by day, and no way to hold the animals out.

**Chapter summary.** With animals home, the live sink reported presence on 99.95 % of animals-only lines, but rooms the animals never enter fired as often empty as occupied, so false presence is mostly the RF method. Pets, AP radio and heating were FALSIFIED as drivers; the driver is unknown. Per-node person-vs-pet separation failed its bar, and the night test is NOT-PROVEN. The animals are a confound whose main cost is truth scarcity.


## 11. RQ6: Would a model learn what the features miss?

Every earlier chapter scored hand-built per-minute features. The remaining objection is that the features discard what a learned model could use. Two tests addressed it: a small neural net on the same features (11.1) and a random-feature readout on the raw capture (11.2). Both used the leakage-free protocol of Chapter 5 (section 5.2).

### 11.1 The small neural net

A one-dimensional convolutional net of about 4-5 thousand parameters read an 8-minute window from all nine nodes. It was scored leave-one-day-out over four development days, with five seeds and a shuffled-label control, and was re-run on a second feature set (best of 18 configurations) [L074]. On the development days the supervised net reached a median waking-minute AUC of 0.68 unmasked and 0.73 masked, against 0.56 and 0.51 for the shuffled control; the label-free variant reached 0.54 and 0.57 (MEASURED) [L074]. The edge over the control, about 0.05-0.10, sat inside the spread of four autocorrelated days, where shuffled per-seed-day AUC alone ranged 0.10-0.94 [L074].

The primary test was a one-shot score on three post-fix days under a bar frozen before any of them existed [App. C]. The supervised net's median waking AUC was 0.67, 0.69 and 0.58. False presence was 47 % of 425 confirmed animals-only minutes, and detection was 61 % (MEASURED) [L074]. The verdict was FAIL, and on every day the best shuffled seed beat the worst supervised seed [L074]. After a labelling defect was corrected, the supervised edge fell from 0.68 to 0.47 unmasked and from 0.73 to 0.53 masked (MEASURED) [L074]. Labels bought nothing. The earlier adaptive classifier tells the same story from the other side: its training accuracy was 0.486 against a chance level of 0.25, and it had been trained on a three-board setup that no longer existed (MEASURED) [L071]. The limit is the features, not the learning head.

### 11.2 The raw-CSI probe

**Question.** Does raw per-node CSI hold presence information that the per-minute features drop?

**Design.** The bar was committed before any CSI was decoded [App. C]. The probe (P) applied 200 random, untrained convolution kernels per node (ROCKET style; length 9, dilations 1, 2, 4, 8, 16, 3,600 features per minute) to the raw capture, followed by a ridge readout. The regularisation strength was chosen by inner leave-one-day-out AUC, and the outer fold was leave-one-day-out [L075; App. C]. The null shuffled labels within each day in 10-minute blocks, 100 times, and the 99th percentile was the bound [App. D]. Three comparators used the same readout: summary features (F), traffic counts only (T), and time of day only (H). PASS required all of: P AUC at least 0.75, P above the null's 99th percentile, P at least 0.05 above F, P at least 0.05 above T, and H below 0.65. If H reached 0.65, the verdict was INVALID-DESIGN whatever P scored [L075].

**Data.** Empty minutes (E) came from owner-confirmed animals-only windows, and occupied minutes (O) from the minutes around a wall-keypad press, which the animals cannot make. The total was 2,177 minutes (E 1,682, O 495) on 13 of 16 days; one damaged capture hour was skipped and three days were dropped for having too few valid nodes [L075; App. C]. The windows were logged outings, not random draws, so E and O fall at different hours [App. D].

**Result.** Pooled AUC was P 0.595, F 0.618, T 0.261 and H 0.729, with a P null 99th percentile of 0.622; per-day P AUC ranged 0.37-0.83 (MEASURED) [L075]. Four of the five bar terms failed (only P above T held). The verdict is INVALID-DESIGN, because time of day alone separated the labels: H 0.729 exceeds the 0.65 ceiling [L075].

**Reading.** The confound runs in the probe's favour. Any readout that can learn the hour of day gains from the E/O hour imbalance, so P had an advantage the design did not intend. It still landed inside its own null, below the summary-feature readout (F 0.618), and far below the 0.75 bar [L075]. That is no evidence of information beyond what the features already carry (MEASURED). The test cannot show that none exists. The pooled AUC lets between-day differences count as signal, and the block-shuffle bound of 0.622 implies about 12.6 times the variance of independent minutes, an effective sample near 175 of the 2,177 minutes [App. D].

A cleaner test would restrict both classes to the same hours. It was not run: matching by hour leaves 320 minute pairs (MEASURED, count only) [L075], and at an effective sample this small the null bound would sit higher than 0.622. A readout that was inside its null with the hour confound working for it is unlikely to pass a stricter comparison, though the confound could also have hurt it, since traffic varies by hour and the traffic-only readout pointed the wrong way (T 0.261). The question therefore remains open: no evidence of extra information, not evidence of its absence.

### 11.3 Upstream pretrained models

Upstream's pretrained encoder was trained on one sleeper in one room: eight unnamed features at 1 Hz, 6,063 frames. Its 82.3 % figure is triplet ordering, not presence [L076]. Its pose model scores 64 % zero-shot across subjects and about 10 % across environments, and has no mean-pose baseline (CLAIMED by upstream; code reading MEASURED) [L076]. A random frozen encoder with a trained readout came within 2-4 points of the trained one, which agrees with the fingerprinting results here [L076]. Chapter 2 sets out the claims. None of it is usable for this fleet.

### 11.4 Implication

On this hardware, nine single-antenna nodes, a raw-CSI model for presence is not supported by anything measured: the learned net lost to shuffled labels, and the random-feature readout stayed inside its null. GPU-scale training has no evidence to justify it. The bottleneck is the information in the input, not the capacity of the model.

**Chapter summary.** A small supervised net failed its pre-registered bar and lost to its own shuffled-label control on every primary day [L074]. A random-convolution probe on raw CSI scored AUC 0.595, inside its null (p99 0.622), below summary features (0.618), and was declared INVALID-DESIGN because time of day alone scored 0.729 [L075]. Upstream's pretrained models were trained on one sleeper in one room [L076]. The evidence points to information in the input, not model capacity, as the limit; an hour-matched raw test remains unrun.


## 12. What the channel is actually doing

This chapter explains why the people-sensing tests failed or were not provable. The claim is limited: the record shows large sources of amplitude variance independent of a person, several tracking occupied hours. It does not show that no person-dependent signal exists. Causal links the record leaves unresolved are marked OPEN.

### 12.1 The access point is not a stable illuminator

Most of the usable frames came from the associated access point (AP-A). An analysis of its frames with the house holding only animals found that its HE-SU frames were beamformed on about 96 % of frames, against 0 % for the nodes' own frames, and that 13-23 % of the AP's frames at each node were addressed to that node [L108] (MEASURED). Two effects distorted per-frame shape. First, on 15-45 % of AP frames per node, the upper buffer (tones +24 to +122) jumped 3-5x and turned noise-like (correlation 0.05-0.23 with neighbouring frames), with a boundary at the same buffer index on all nine nodes; the bursts were memoryless and independent across receivers, and their cause inside the C6 is unknown [L108]. Second, per-destination steering explained 0.07-0.62 of the variance of the lower-segment shape (shuffled control 0.001-0.005); steering was shared across receivers, with a median dwell of one frame [L108] (MEASURED).

The consequence is that consecutive frames from the same transmitter are not repeated measurements of one channel. The AP's frames to different clients look different, so the shape of a frame depends on which client the AP was serving, that is, on household traffic. A transmitter filter by BSSID removes other transmitters but leaves this dependence in place.

### 12.2 Rate, airtime and congestion

The fleet's own behaviour dominated the channel. The 2.4 GHz channel was 80-85 % busy by daily median from 09-23 to 09-30 [L103] (MEASURED). Nodes with weak links repeatedly latched to 1-2 Mb/s 802.11b rates, where the same frame occupies the channel about 24 times longer [L095, L104]. Nodes 4 and 6 consumed about 88 % of the fleet's payload airtime from rate alone (8.9 % and 4.8 % against 0.20-0.34 % per other node) [L104], and nodes 5 and 6 carried 18 % of the data and 91 % of the airtime in one capture [L095]. Delivered fleet fps correlated -0.84 with the sum of 1/rate across nodes [L104] (MEASURED).

The designed mitigations did not hold. Raising the SSID minimum rate to 12 Mb/s left node 5 at 1 Mb/s on 5,910 of 6,174 captured frames [L093]; the driver-level 11b block returned success on all nodes while rate fallback still sent 1,093 of 10,263 frames per minute at 11b rates from node 6 [L102]. A dedicated 2.4 GHz SSID plus firmware 0.9.4 reduced the daily median channel use from 80-85 % to 44 % and fleet accepted fps rose from 200 to 253 (+26 %) [L103], but the two changes could not be separated, and the slide recurred. Slides were not sticky: nodes 4 and 6 recovered without reboot [L103]. Onset correlated with smart-thermostat rejoins (18 of 27 sustained onsets within 3 minutes of one) [L104], but the hypothesis that a schedule, heartbeat or HVAC change triggers the rejoins was FALSIFIED [L110], and the trigger is OPEN. Lowering the self-ping rate or changing IDF version did not stop the slide [L105]. Capture on a congested night showed slow minutes were exactly the minutes at 70 % busy or more, with the fleet's own transmit share only 13-17 % [L109] (MEASURED).

This matters for presence because a level feature moves with congestion, not with people. On one day the fleet's motion energy doubled together with channel load and stayed doubled; the stated chain was rate collapse, airtime, collisions, noisier CSI, higher motion energy, false presence [L107] (MEASURED). The shipped presence flag read true 99.78 % of empty-house samples and 99.89 % of occupied ones [L005], and firmware motion energy was higher when the house was empty than when it was occupied [L007] (MEASURED). Empty-versus-occupied score differences were about 0.1-0.25 of an interquartile range, with no environmental driver found: AP busy, own and other airtime, retries and rate each had |rho| at most 0.2 (0.44 for rate in one window), and pets, AP radio and heating were FALSIFIED as drivers [L068] (MEASURED). The driver of the remainder is OPEN.

### 12.3 The transmitter population moves

The set of audible transmitters is not constant. After an overnight mains interruption on 09-02 the audible set was swapped wholesale and every scalar metric moved; one device went from 1.1 to 6.9 simultaneous links [L082] (MEASURED). The fleet's delivered rate also stepped together with no intervention (about 200, 242, then 165 fps across 09-22..09-23), with per-node coefficient of variation 12.9-19.2 % and fleet 14.8 % [L096]; a six-node, four-window partial gave 2.9 % (UNDERPOWERED) [L096]. A population-shift detector flagged 3 windows [L096] and fired five crossings of 5x-31x the median distance, but whether these land on the fps dips is NOT-PROVEN [L092]; the cause of the common steps is OPEN. A step on 11-12 from about 191 to about 268 fps coincided with a firmware roll and is attributed to the nodes reaching their configured ping rate [L114] (MEASURED, mechanism CLAIMED). AP-A crashed repeatedly: five times on 10-04 and 0-3 times on most days from 09-23, each costing about 2 minutes of zero CSI [App. E: F14]; after the 11-05 crash fleet accepted fps stayed near 80 for four days [App. E: F40]. The 11-19 rack power loss took both APs [App. E: F49].

Channel experiments gave no second view. A first channel-split snapshot moved RSSI by 16-23 dB up on some nodes and 22-24 dB down on others and is NOT-PROVEN [L083]. Moving the whole fleet to the second AP (AP-B) improved only node 5 (-77 to -60 dBm); eight of nine nodes lost 2-21 dB and fleet fps fell 28 % [L113] (MEASURED). AP-B remained as failover only.

### 12.4 Illuminators and geometry

Household devices could not supply a second viewpoint. Blind arcs of 101-286 degrees surrounded each node, with six of nine blind to the north-west, so which-link-is-dented could not give direction [L080]. Three basement nodes depended on one basement device as their nearest emitter [L081]. Every positioned IoT emitter was a 64-bin grid sender at 0.1 fps or less, and only 3 of 36 reached two receivers at 1 fps or more; two receivers essentially never caught the same frame [L088] (MEASURED, FALSIFIED as illuminators). The limit is the transmit rate of consumer devices, which was not controllable.

Pairing losses also came from the receive path. A per-link grid gate raised links from 31 to 137 [L084]. A sequence gate raised the paired-frame fraction from 0.2434 to 0.5605 (2.30x), while a mesh-aligned gate gave +0.5 points [L085]. Pinning the fleet to one transmitter raised the fraction to 0.7287 but cut paired observations per second by 44 % [L091]. Node-to-node links were limited to about 0.5 fps because the firmware kept one of every eight frames; a bypass lifted node 0 from a median 0.51 to 6.1 fps and fleet node-to-node rate by 3.3x [L117] (MEASURED, cause CLAIMED from code reading).

### 12.5 The instrument

The motion metric is a change detector. A seated or sleeping body is absorbed by its baseline within 3-8 s, and the raw alternative discriminated worse (Mann-Whitney 12 of 20 against 16 of 20) [L051] (MEASURED). Absolute per-link phase was unusable (adjacent-subcarrier phase differences, which cancel the per-packet offset, were usable and were tried as a presence feature [App. B: M05]): the resultant length of phase Doppler was 0.011-0.032 against 0.000 for uniform random, because packet-detection timing re-randomises phase every packet [L077] (FALSIFIED). The C6 gain field was found in word 5 of rx_ctrl, bits 24-31, with Spearman rho -0.89 to -0.98 on 9 of 9 nodes, but units are unverified and the FFT scale field was not identified [L100]. A decisive test found AGC gain steps not enriched in quiet-hour motion spikes (RR 0.742, p 0.879), closing the gain-lock option [L101] (MEASURED; the first attempt's RR 1.636 was a capture error). Of HE PPDUs, 69 % carried an invalid channel estimate; whether dropping them helps was never scored [L115] (OPEN).

### 12.6 Physical models that failed on real data

The link-line model failed. In one room with three co-located sensors, response was inverse to distance from the link line (peak |z| 3.8 at 11 in, 13.0 at 47 in), the link-line solve gave 58 in mean error against 49 in for always answering a fixed point, and AP links carried no position information [L118] (FALSIFIED, MEASURED; scope one room). The co-denting premise also failed: a body near a receiver did not dent all its links. Co-denting of two links on a tick occurred on 0.6 %, 7.6 % and 13.6 % of receiver events across three recordings [L122] (FALSIFIED, MEASURED; synthetic build 16 of 17 bars). The large dents were single-link events.

### 12.7 Synthesis

| Source of variance | Mechanism | Magnitude | Effect on a presence feature |
|---|---|---|---|
| AP beamforming | Per-client steering, upper-band bursts | 96 % of AP frames beamformed; steering R2 0.07-0.62 [L108] | Shape varies with household traffic |
| PHY-rate collapse | Nodes fall to 1-2 Mb/s | -0.84 with sum(1/rate); nodes 4+6 about 88 % of airtime [L104] | Level rises with congestion (false presence) |
| Channel busy | Own nodes, IoT, retries | 80-85 % busy, 44 % after SSID change [L103] | No occupancy information |
| Population shift | Transmitters swap after power events | 5x-31x median distance [L092] | Window comparability lost; cause of fleet steps OPEN |
| AP crashes | AP radio fault | about 2 min zero CSI each [App. E: F24] | Outages and long rate depression |
| Receive gate | 1-in-8 frame filter | 0.51 to 6.1 fps, node 0 [L117] | Few paired frames |
| Illuminators | Only the AP and nodes are fast | 3 of 36 usable [L088] | No second viewpoint |
| Change detector | Baseline absorbs stillness | 3-8 s [L051] | Still bodies vanish |
| Invalid HE estimates | Estimator unreliable | 69 % of HE PPDUs [L115] | OPEN |

The person effect is small compared with these. Several of the larger terms follow traffic, and household traffic follows occupied hours, so a level feature can correlate with occupancy for reasons unrelated to presence. This is consistent with the unstable results: a waking AUC of .50, .76, .14 and .65 across four days [L009] and an AUC of 0.45 on a clean same-day transition [L008] (MEASURED). That traffic correlation is an inference from these records, not a measured decomposition; no entry attributes the variance of one test to each source (GAPS 6), so the shares are OPEN.

**Chapter summary.** The nine-node channel carried large variance unrelated to people: beamformed AP frames whose shape follows client traffic, 1-2 Mb/s rate collapses that filled the channel to 80-85 %, shifting transmitter populations, AP crashes, and a receive gate that discarded seven of eight frames. The instrument added a baseline that absorbs still bodies in 3-8 s and unusable phase. Two physical models, link-line and co-denting, were falsified on real data. These sources, several correlated with occupancy hours, are a plausible explanation for the failed and day-flipping presence results; it is an inference from the records, not a measured decomposition, and the individual shares are OPEN.


## Part II. What worked

### W.1 Why this part exists

A negative sensing result is credible only if the platform beneath it was sound. Part I reports that every people-sensing question failed or was not provable on nine single-antenna nodes (chapters 6 to 11, with the method in section 5 and the channel diagnosis in section 12). This part answers whether the nodes, uplink, recorder and software chain were sound, with the same discipline as Part I. Every "better than upstream" claim carries a MEASURED before/after number, or it is tagged CLAIMED with the words "no before/after measured". Hardware claims say whether the evidence is a real-node log ("silicon") or a build or host test only. Tags follow the brief: MEASURED has a reproducer or captured log, CLAIMED was built without a before/after, SYNTHETIC rests on a simulator or unit test. Upstream status is given per item.

Scale: 277 commits touched firmware, UI or the sink crate; the diff against upstream main is 140 files, +36,042/-886 lines [Part II]. The fleet is nine ESP32-C6 nodes on one access point and channel.

### W.2 Firmware

The strongest items come first. "Upstream before" is judged against upstream main at the time of the work.

| Item | Upstream before | Ours | Measured effect | Evidence |
|---|---|---|---|---|
| Rollback bootloader, installed over the air | Absent; six USB-flashed boards had no rollback | Fingerprinted bootloader pushed by OTA | 3 of 9 valid -> 9/9 valid; the proof OTA read back `ota_state valid` [L157] | silicon |
| Multi-hop mesh sync (protocol v2, re-election) | Single-hop; cannot sync nodes outside leader range | Hops 1-3 | v1 6/9 valid, 2 leaders -> v2 9/9 valid, 1 leader; 500 simulated seeds clean [L087]; residual skew -4.46 and -7.42 ppm [L078] | silicon (9/9), simulator (seeds) |
| Link-rate watchdog and `/link/reassoc` | Absent | Detects a slide to a low PHY rate and rejoins | 19 actions, 15 reached the node, 15/15 recovered within 3 minutes; 7 soak nights without a slide of 10+ minutes [L178] | silicon |
| Peer-beacon bypass of the 1-in-8 receive gate | Gate starves node-to-node links | Bypass (default off in the image) | Node-to-node 122 -> 418 fps (x3.3); pairs at >= 2 fps 24 -> 39 [L117] | silicon |
| ESP-NOW send-callback race fix | In-flight counter race wedged the send path | Race fixed | Stall lines 133/day -> 0 on one canary node [L190] | silicon, one node |
| Trial-config rollback | Absent | A configuration change that cuts connectivity reverts itself | After a deliberately wrong Wi-Fi password the node recovered in 60.5 s [session record] (CLAIMED) | one trial reported in a session; no node log retained |

Reading the table. The bootloader and sync results are fleet-wide counts on real nodes. The watchdog figure is a recovery rate, not a controlled trial: night-report rows moved from 146 to 0 node-minutes at or below 2 Mbps across two nights, with other changes in the same interval [Part II]. The bypass result is real but bought no sensing gain. The race-fix baseline is one node; the trial-config figure is one trial reported in a session with no node log kept, so it stays CLAIMED until repeated with a captured log [session record].

Further MEASURED items, each with a number:

- **`node_log` and crash retention.** An on-flash log with remote pull. Rollout 9/9; one pull returned 183,760 records with 0 CRC failures, and the audit found the 8 MB flash volume full of non-log data [L160, L192]. Merged upstream as #1878 and #1890.
- **Crash diagnosis.** Five fleet crashes in one day were retained as KERNEL_CRASH with the same fault registers, locating the fault at the access point [L180].
- **Uplink stall rules.** 0 false reboots in 3 nights (7,096 records) [L179]; the IP-dead rule is host-tested only (SYNTHETIC) [L193].
- **HE20 truncation** fixed and merged (#1792); duplicate-define warnings 9 -> 0 [L135]. **Raw rx_ctrl logging:** 4,198/4,198 RSSI match, fps 221 -> 246 [L167]. **BLE scanner cost:** 120 KiB heap free, own fps +0.45 (about 2 %) [L173]. **`/config` over the network:** used for every push (no efficiency number); merged (#1827 via #2289) [Part II].

CLAIMED only, no before/after measured: fps-floor watchdog; OTA send-health confirm and safety timer (rollback trigger not forced on silicon); BSSID pin with fallback (build/test only [L189]); free-heap floor for raw sends [L192]; 200 ms self-ping bound; thermal and health monitoring merged from upstream (#1797, #1796) [Part II]. Of 21 firmware items, 9 carry a before/after number and 6 are CLAIMED only [Part II].

### W.3 Operations UI

The branch carries pages that upstream main lacks (upstream has the Hardware and Dashboard tabs only [Part II]): a Nodes page, Room Builder, link mesh panel, fleet KPI page, groups, illuminators, mesh matrix and RF distance map, 3D building view, tapper, survey, walk and mark pages, and a waterfall. No page has a before/after measurement. The evidence is that the fleet was operated through them for about three months, on browser and live-fleet use.

- **Nodes page (CLAIMED).** Per-node configuration, LED, OTA deployment, an in-flight configuration trial that blocks further edits while pending, and log and coredump downloads through a server proxy that holds the OTA key. Server side open as #1812.
- **Room Builder (CLAIMED).** Footprint, per-floor canvases, magnetic rooms, walls and access-point placement, replacing a fixed rectangle; operator-reported only. Footprint open (#1823); storeys and walls closed (#1824). Branch-only.
- **Fleet KPI page and night report (MEASURED).** Whether each node's CSI reaches the sink and whether the node is healthy. The night report matched a hand-built report [L168]. It caught real events: a node offline 435 minutes, fleet fps 82 against a 200 floor, access-point crash boots [Part II]. KPI open (#2076).
- **Mesh matrix, 3D view, tapper (CLAIMED).** Open (#1837, #1834, #1825).
- **Never show invented data as sensing (MEASURED root cause).** A fabricated browser signal produced the absent/present flip-flop; the simulation is now default-off. Merged (#1794).

Fleet frame rate rose from about 260 fps to about 500 once the bypass and receiver changes applied; that is a transport rate, not a sensing result [Part II].

### W.4 The position and visualisation stack

Nine pieces were built and run as software: the est-rti-v1 estimator, a Rust link-line estimator, a per-link fusion API, an hourly position job (chunks byte-identical to a continuous run [L185]), a playback view, a scorer, Room Builder geometry tooling and floor-plan views. As code they are BUILT-WORKS; on real data none produced a localisation (chapter 8).

**Demo design.** Unit tests had fed the estimator hand-made vectors; this SYNTHETIC end-to-end demo covers the whole chain [Part II, synthetic demonstration]. A made-up two-floor, six-room house with 9 nodes and an access point; one scripted walker visiting all six rooms (8 dwells of 90-120 s, two staircase transits) after 25 minutes empty and before 8 minutes empty; synthetic captures in the real wire format (81 links at 4 fps, about 1 M frames per arm). They pass through the real, unchanged extraction, estimation, hourly merge and playback scripts. Thresholds were written before any capture was generated: room hit >= 0.60, floor hit >= 0.85, median error <= 3.0 m, empty-period blob rate <= 0.10 [Part II, synthetic demonstration]. Arm A assumes the response falls with distance from the link line. Arm B uses the response measured on the real fleet, which grows with distance (11 in -> |z| 3.8; 47 in -> 13.0 [L118]).

| Metric (18 scored minutes, 534 steps) | Arm A (pre-registered) | Arm B (measured physics) | Arm A2 (post hoc) | Chance |
|---|---|---|---|---|
| Room hit, per minute | 0.444 | 0.056 | 0.889 | 0.178 |
| Floor hit, per minute | 0.444 | 0.444 | 0.889 | 0.5 |
| Median error, blob on right floor | 0.76 m | 5.27 m | 0.65 m | |
| Blob rate, empty periods | 0.0 | 0.0 | 0.0 | |
| Verdict against the bars | FAIL (recall) | no threshold | pass, post hoc | |

All values SYNTHETIC [Part II, synthetic demonstration]. The pre-registered arm A FAILED. It failed on recall, not accuracy: where it drew a blob it was right (floor 100 %, 0.76 m), but it drew none for the whole ground-floor part of the walk. The summed excess there was 0-3.5 against a gate of 4.0, upstairs 13-18 [Part II, synthetic demonstration]. Arm A2 changed one parameter (modulation depth 0.30 -> 0.60) after the failure was seen; it is not a pre-registered pass and must not be cited as one.

The three-statement reading: (1) the chain is correct when it fires, and it draws no blob in empty periods in any arm; (2) its sensitivity is bounded by a gate tuned to real noise, which a still or slowly moving person does not clear; (3) under the physics the real fleet measured (arm B) it draws a blob at every step and is wrong, with room hit below chance and a top blob upstairs in 439 of 551 blob steps, so the floor rate equals the share of the walk spent upstairs and is a constant answer. The software is not the limit; the sensor is.

Limits. Arm A is circular by design: it validates the software chain, not the physics. Arm B's extrapolation beyond 1.19 m is an assumption, and its measurement came from one room with three co-located sensors. One person can appear as up to 3 blobs per step (only the strongest is scored). No truth marker is drawn on the screenshots yet [Part II, synthetic demonstration].

### W.5 Sink, recorder and automation

- **UDP drops and CPU (MEASURED).** The sink could not keep up with nine-node CSI: 44 % UDP drops at 101-102 % CPU. Memoising the Doppler computation gave 0 drops and 25.6 % container CPU; the baseline replay showed 17 % kernel drop [L176]. Live counters and pcap replay. Fleet-sink fork; not offered.
- **Parser fixes (MEASURED).** 9/9 nodes delivering, fps before/after within +-2.3, and three unparsed packet types at about 9/s that had been silently dropped [L165]. #1860 and #1791 merged.
- **Persistence.** An audit found two states over an hour to rebuild and no SIGTERM handling [L169] (MEASURED); the fix was not adopted by default (CLAIMED).
- **Unattended recorder (MEASURED).** Packet capture runs independently of the sink process and kept raw capture for 100+ nights. 23 h was lost once to a stopped job [L161].
- **Damaged-capture handling (MEASURED).** Hour 1: about 67 KB salvaged across 4 holes; hour 2: about 5 MB unreadable [L187].
- **Retry skip list.** The cause of 2,016 hourly-job failures a day was found and a skip list added [L191]; post-fix count not recorded (CLAIMED).
- **Night report, cron container.** Report in W.3. The cron container replacing host crontabs (lost twice) is CLAIMED, no before/after measured.
- **Local-model records tooling.** Claude 9.6 against local models 4.9-6.5 of 10 [L183].

### W.6 Contributions upstream

66 pull requests were opened: 22 merged, 38 open, 6 closed (read-only count, 2026-11-20 [Part II]). Notable merged work: `/config` (#1827) and link reassociation (#2278), both via the maintainer's integration PR #2289; thermal and health (#1797); retry-forever with uplink watchdog (#1795); HE20 truncation (#1792); persistent node log (#1878, #1890); server (#1857, #1793, #1791, #1860); UI (#1794). The largest open cohort is UI and server work (#1806, #1823, #1828, #1833, #1834) with no reviewer engagement since early September. An audit triaged 307 upstream PRs, of which 126 touch the paths this study changed; of 353 unmerged local commits, 46 are present exactly upstream, about 258 adapted and about 34 absent [L175]. ### W.7 What did not work in the engineering

- **Batched, staggered uplink.** Raw airtime rose on flashed nodes (2.5 -> 8.3 %, 4.3 -> 12.3 %, 3.0 -> 15.7 %) while retries fell, and four boards crashed at small batch sizes [L159].
- **Hot-resizing the build disk.** Resizing the iSCSI LUN under it aborted the journal and lost the disk and a 745 MB replay output [L170].
- **Green tests, inert feature.** 531 server tests passed while a dead match arm disabled the feature (11 of 13 branches alive) [L131]; a grid-lock fix passed 1,127 tests while five nodes stayed stale for ten minutes [L162].
- **Silent no-ops.** A CRLF compose file made an anchored `sed` change nothing; a commit hook never ran; stale-version OTA builds needed a forced clean. The count of silently ineffective OTA pushes was not recorded in the sources read (the figure of 7 could not be confirmed) [Part II].
- **Dark nodes.** Five nodes delivered zero accepted frames [L162]; one stayed dark 94 minutes [L193].
- **Edge feature packets.** 1-7 % of uplink airtime, no consumer [L163].
- **Fake-live values.** Zero-filled values passed as live in several places [L135]. **Recorder gaps:** 23 h lost once; two hours later found damaged [L161, L187].
- **Mesh sync v1** was structurally single-hop; v2 is the repair, not the original design.

### W.8 Summary table

| Item | Upstream before | Ours | Measured effect | Evidence | Tag | Upstream status |
|---|---|---|---|---|---|---|
| Rollback bootloader | Absent (3/9 valid) | OTA-installed | 3/9 -> 9/9 [L157] | silicon | MEASURED | Not offered |
| Multi-hop mesh sync | Single-hop | Protocol v2 | 6/9 -> 9/9; 2 -> 1 leader [L087] | silicon + simulator | MEASURED | Not offered |
| Link-rate watchdog | Absent | Built | 15/15 recovered in 3 min [L178] | silicon | MEASURED | #2278 merged; #2593 open |
| Node-to-node bypass | Gate starves links | Bypass | 122 -> 418 fps [L117] | silicon | MEASURED | Not offered |
| Send-callback race | Broken | Fixed | 133/day -> 0 (canary) [L190] | silicon | MEASURED | Not offered |
| Trial-config rollback | Absent | Built | Recovered in 60.5 s [session record] | one reported trial, no log | CLAIMED | Open (#1812 server) |
| Sink UDP and CPU | Could not keep up | Memoised Doppler | 44 % -> 0 drops; 101 % -> 25.6 % CPU [L176] | live + replay | MEASURED | Not offered |
| Position stack, synthetic demo | Absent | Built | Arm A FAIL; arm B wrong [Part II, synthetic demonstration] | host test | SYNTHETIC | Partly open (#1809 closed) |

**Part summary.** The engineering was sound and, for the items listed, measurably better than upstream main on real nodes: rollback coverage, mesh sync, link-rate recovery, uplink rate and sink drops each have a before/after number. The operations UI and several firmware safeguards are CLAIMED, with no before/after measured. The position chain runs and is correct when it fires, but its pre-registered synthetic test failed on recall and under measured physics it is wrong. The platform under the negative sensing result was therefore not the cause of it. Sixty-six pull requests were opened, 22 merged.


## 13. Synthesis: why it failed

This chapter adds no results. It reads chapters 5 to 12 together, using their ledger entries, and states what the evidence supports and what it does not.

### 13.1 The pattern across six questions

| Question | Best result | Verdict | Kind of failure (5.4) | Chapter |
|---|---|---|---|---|
| Is anyone home? | Best unmasked false presence 0.192 against a 0.05 bar [L018]; production algorithm detection 0.62, false presence 0.63 [L019] (MEASURED) | FAIL | Secure evidence that these detectors cannot meet the bar (gaps 0.1-0.6), not of the absence of any person signal; no evidence from the screens (0 survivors in 155 tests, p floor 0.0625) [L021-L023] | 6 |
| Is this room active? | Sealed rule passed precision on two nodes by not firing; caught 0 of 99 in the blind week [L035; L039] (MEASURED) | NOT-PROVEN | Evidence of no response for that rule and truth; false-alarm half no evidence (3.03 of 10 h) | 7 |
| Where is the person? | Lift 0.197 with the bypass off and on [L050]; 13 hits, lift +0.04 [L049] (MEASURED) | FAIL | No evidence (about 38 % power; truth about 90 % one room) | 8 |
| Still people, vitals? | Watch-night Spearman -0.07 [L063]; breathing peak 8 % occupied against 19 % empty [L060] (MEASURED) | FAIL / NOT-PROVEN | No evidence (n = 1; peak test NOT-PROVEN) | 9 |
| Pets are always home | Rooms the animals never enter fired .49 against .44 [L067]; person-vs-pet AUC 0.86 on one node, bar failed [L069] (MEASURED) | FAIL | Mostly no evidence (truth scarcity) | 10 |
| Would a model learn more? | Net 0.67/0.69/0.58 median waking AUC [L074]; raw probe 0.595 against null p99 0.622 [L075] (MEASURED) | FAIL / INVALID-DESIGN | No evidence of extra information (effective n near 175 of 2,177 minutes) | 11 |

Every question ended in FAIL, NOT-PROVEN or INVALID-DESIGN, and none produced a PASS that survives its own caveat. The failures were of two kinds, and only the first is a negative finding.

### 13.2 Attribution

Three limits recur: the channel (12.1-12.4), the instrument (12.5) and the truth supply (5.4). The table below is this study's reading of the records. It is not a measured variance decomposition: no ledger entry attributes the variance of one test to each source, and 12.7 states that the shares are OPEN. "Dominant", "contributing" and "--" are judgements.

| Test | Channel | Instrument | Truth | Note |
|---|---|---|---|---|
| House presence, R9 week (ch. 6) | dominant | contributing | contributing | Displayed label read present 100 % of minutes, AUC 0.50 [L012]; the flag was true 99.78 % empty against 99.89 % occupied [L005]; waking AUC flipped .50/.76/.14/.65 across days [L009] |
| Sealed room rule and blind week (ch. 7) | contributing | contributing | dominant | Seven of nine rooms had no truth and one appliance was shared [L155]; 3.03 of 10 guard hours [L039] |
| Position bypass re-score (ch. 8) | contributing | contributing | dominant | Truth about 90 % one room (90 of 99) [L050]; power near 38 % [App. D]; link-line response inverted [L118] |
| Still person, watch night (ch. 9) | -- | dominant | dominant | Baseline absorbs a body in 3-8 s [L051]; one night, one subject, |rho| above 0.64 needed [L063; App. D] |
| Person vs pet (ch. 10) | dominant | contributing | contributing | Rooms animals never enter matched occupied rates [L067]; person truth only 14 events on node 5 [L069] |
| Net and raw probe (ch. 11) | contributing | contributing | dominant | Time of day alone scored 0.729 [L075]; null spread 0.10-0.94 per seed-day [L074] |

Justifications. Channel dominates the presence test because the shipped flag could not tell the two states apart while rate collapse and busy airtime were measured to move level features (-0.84 with the sum of 1/rate [L104]; chain to false presence [L107]). It dominates pets because pets cannot explain a node they cannot reach; the same fact implicates the instrument, which fired on those rooms as well. The instrument dominates stillness because a change detector removes a still body within seconds, whatever the channel did. Truth dominates rooms, position and the learned tests because the labels, not the radio, set the largest uncertainty: appliance-only, one-room or hour-confounded truth. A cell marked "--" means the records give no fact placing that limit in the test, not that it was absent.

### 13.3 What would have been detectable

[App. D] gives the smallest effects the main tests could have seen (derived). The family-wise screens needed a feature to shift by about one standard deviation of its own window-to-window spread (1.13 for [L023], 0.89 for [L022], 1.77 for [L021]), in about 11 of 15 windows. At day level, an AUC edge below about 0.3 was indistinguishable from the day-to-day swing, so only a feature holding 0.7 on every day could register [L009]. The neural net needed a true AUC near 0.9 on every day [L074]. The raw probe had to beat 0.622 and the bar was 0.75 [L075]. Position at 107 moments could see a lift near +0.05 above control [L049], but the bypass re-score had about 38 % power for its +0.10 bar [L050]. The sealed week would have passed on 3 catches of 99 [L039]. The watch night needed a correlation above 0.64 [L063].

What follows is bounded. A modest person effect could exist below these thresholds, in a feature or on a link these tests did not isolate, or only on days the channel was quiet. The study rules out a large, stable, cross-day person effect on these nodes in this house, by the methods used. It does not rule out every effect, and it does not rule out a differently instrumented system.

### 13.4 The software was not the limit

The synthetic demonstration (8.5; Part II) isolates the software. The chain was correct when it fired (median error 0.76 m) and drew no blobs when nobody was present; its sensitivity was bounded by a gate tuned to real noise, and its own pre-registered arm failed the bar (room hit rate 0.444 against 0.60) [Part II, synthetic demonstration] (SYNTHETIC). Under the measured inverse response it was wrong (0.056, below chance 0.178) [Part II, synthetic demonstration]. The engineering held up where it was measured [Part II]. Rollback bootloaders went from 3 of 9 to 9 of 9 valid [L157]. Mesh sync went from 6 of 9 with two leaders to 9 of 9 with one [L087]. Node-to-node frames rose 3.3x through the receive-gate bypass [L117]. The send-callback race fix cut stall lines from 133 per day to 0 on the canary node [L190]. A sink fix took 44 % UDP drops to none [L176]. The recorder ran unattended, and its two damaged capture hours were detected, listed and skipped by every reader [L187] (MEASURED).

For anyone building on the same hardware: the open problems are in the radio (a transmitter that beamforms per client, a congested band, single-antenna frames with no usable absolute or cross-antenna phase), not in firmware, sink or display code that already works. Better software on these nodes will not recover a signal the input does not hold; a new test should start by controlling the channel and securing truth before it adds a model.

### 13.5 Is it worth pursuing?

On this evidence, for presence in a home with animals, no. The bounded statement is this. Nine single-antenna ESP32-C6 nodes, one AP, one occupied house with animals always home, and self-learning without labels: across about 12 weeks and every question above, none beat a bar fixed before scoring or a non-RF baseline (a phone rule, whole-house power) [L020; L025]. The failures are secure where gaps to the bar were large, and no-evidence elsewhere. Other hardware with phase-coherent multi-antenna capture, an empty-house truth supply, or a different home might behave differently. That is a HYPOTHESIS (CLAIMED), supported by [L041] and the literature and untested here.

**Chapter summary.** Six questions ended in FAIL, NOT-PROVEN or INVALID-DESIGN; the large-gap failures securely show that these detectors cannot meet their bars, and the rest are no evidence of a signal. Attribution to channel, instrument and truth supply is this study's reading, not a measured decomposition, and the shares are OPEN. The tests could rule out a large, stable, cross-day person effect on these nodes, not every effect. The software and engineering worked; the sensor and the house, not the code, were the limit.


## 14. Threats to validity

### 14.1 External validity

The study ran in one house, with one household schedule, one pair of access points (AP-A and AP-B, one model class) and one fleet of nine ESP32-C6 nodes. Every negative result is a statement about that arrangement. Several channel findings (per-client beamforming, PHY-rate collapse, AP crashes; chapter 12) are properties of this AP model and its firmware, and a different AP could behave differently. The prospects for placement with other hardware are a HYPOTHESIS (CLAIMED), not a finding (chapter 15). The study does not show that WiFi sensing fails in general; it shows that this class of node, in this kind of home, did not carry a person signal that survived held-out evaluation.

### 14.2 Fleet and sink churn

The fleet changed continually. Of 62 scored tests, 60 had at least one scope-matched fleet change inside their span; the exceptions are the one-day breathing-window check (L053) and the upstream-model review (L076), which is not a fleet-data test [App. E]. The most confounded were the raw-CSI probe (L075, 21 major changes inside its span) and the whole-capture screens and the phone rule (L004, L020, L037: 11 to 14 each) [App. E]. The blind confirmation week (L039) ran with node 3 unpowered and an AP-crash aftermath in which fleet accepted frames stayed near 80 fps for four days [App. E: F40], leaving 70.5 of 168 hours with data [App. C; L039]. The least confounded tests are the one-day L053 and, among the sealed tests, the H3 test week [L035], whose span still holds seven changes counting its development days [App. E].

Churn does not rescue a negative result. A usable person signal would be expected to survive firmware revisions and sink changes, since the signal is physical and the changes were mostly reliability fixes. Churn does widen the uncertainty: a feature that failed under one firmware could in principle have passed under another, and cross-test comparison is weakened because the instrument differed between tests.

### 14.3 Label noise

Truth was scarce and imperfect (section 5.4). Phone Wi-Fi sessions mislabelled a 110-minute window as away while a person was home, and that single error accounted for much of the apparent supervised advantage [L148] (MEASURED). Corrected labels left every re-run set at FAIL [L148]. Labels from footage, plate reads and phones were each partial; the remaining error rate is unknown, so effect sizes near the bars carry unquantified label uncertainty.

### 14.4 Analyst degrees of freedom and post-hoc items

Bars were fixed before scoring for most tests, but not all. Of 24 bars, 18 are proven to predate scoring and six are weak: sealed only by file timestamp, claimed, or rewritten [App. C; section 5.1]. Post-hoc items are labelled where they occur:

- the five-minute home/away variant was the development study's post-hoc pick; the one-minute rule was the pre-stated one [App. C];
- the second demo arm (A2) and the receive-observation screen were added after the first arm returned zero survivors, and the shift control for that family did not come out clean [App. C];
- the strong-node hold and witness-rule redesigns (R22b, R23b) were derived after reading the bar-day results, and their decision days were VOID because the reproduction gate mismatched on two of three days [L015, L016];
- the breathing "resting person nearby" lead was found after the fact [L060];
- the positions bypass check added a confusion matrix after the bar, which showed the apparent lift was a room-bias artefact [App. C].

Looks are counted where known (521 on the blind week [App. C]).

### 14.5 Reproducibility limits

Not everything can be re-run. Scripts named in ledger entries and absent from every branch and from the transcripts include the thermostat rejoin analysis [L110], the walk-survey smoothness scripts [L043], the OTA roll script [L104], the recorder harness [L052], the shape and drift scripts [L061], the parity checks [L014, L147] and scratch replays [L055] [[script]; App. B]. Of 14 missing items, two were recovered: the blind-analyst brief from a session transcript (L188) and the placement poller from a copy of the owner's desktop folders (L079) [App. B]. Early transcripts are lost, so the record for August and early September rests on notes.

The raw household data (captures, labels, phone and camera records) are not published, for privacy, so the results can be checked against the ledger and scripts but not re-scored by outsiders. Some capture hours are damaged: the raw-CSI probe skipped one hour and dropped three days (L075), and the blind week lost 97.5 hours to the outage. Sources disagree in places (verdict FAIL versus NOT-PROVEN for the same numbers, different moment counts for the same span); both values are given in the ledger entries [GAPS 2].

### 14.6 Records written by the analysts

The analysis, the scorers and most of the record were produced by LLM coding agents directed and reviewed by the owner (section 5.6). Records were written by the same agents that ran the analyses, so errors of interpretation can be shared between the test and its description. Mitigation was structural: bars in git, scorers separate from analysts, and a blind analyst for one family (L038). It is not independent review.

**Chapter summary.** The study is one house, one schedule, one AP model and one fleet, so it supports "no evidence of a signal here", not a general negative. Almost every test spanned fleet changes; the signal that failed is the kind that should have survived them, but the uncertainty is wider than a controlled trial would give. Phone labels were noisy, six of 24 bars are weak, and several designs were revised after seeing results, each flagged. Thirteen of 14 named missing scripts are unrecoverable and the household data are unpublished. The records were written by the agents that did the analysis.


## 15. The constraint frame: what would change the answer

Every idea still open after the tests needs one standing constraint relaxed. The constraints are not incidental. They were set by the owner for the study: a hobby budget of about $100 with no new hardware, a requirement that the system learn without end-user labelling, fixed node positions, and a house that is never empty of animals. This chapter lists each idea, what it needs, which constraint blocked it and what the record says about its prospects. None of the right-hand column is a result for the open ideas; where it is not a measurement it is tagged CLAIMED.

### 15.1 The table

| Idea | What it needs | Why it was not done (constraint) | What the record says about its prospects |
|---|---|---|---|
| Phase or angle of arrival | Two-antenna, phase-coherent boards | Hobby budget of about $100; the owner ruled no new hardware | Not tested. The C6 gives one antenna: per-subcarrier amplitude and adjacent-subcarrier phase differences, but no absolute or cross-antenna phase (12.5). Prospects from multi-antenna literature in lab conditions are CLAIMED, and the field evidence for them is nil |
| Supervised walk-throughs with clean truth | Labelled passes through each room | A product must self-learn; labelling is overhead a general user will not carry | One supervised result exists: labelled fingerprints classified 6/10 among five spots with mean error 27 in, in one room with three co-located sensors [L041] (MEASURED, SUPPORTED); correct matches were the three strong spots only. It does not transfer to the fleet [L118] |
| Re-placing nodes for link geometry | Nodes moved to improve link coverage | Nodes are tied to outlets, and moving them invalidates the raw archive that every later test uses | The link-line model was FALSIFIED on this fleet: response was inverse to link-line distance (peak |z| 3.8 at 11 in, 13.0 at 47 in), and the link-line solve missed by 58 in against 49 in for a fixed answer [L118] (MEASURED, one room). Geometry alone did not help |
| Empty-house baselines | Hours of a confirmed-empty house | The animals are always home; at most one short, stressful window was available | Sixteen empty windows in total, too little to learn a per-hour baseline (App. D). Sink presence was true on 0.9978 of an animals-only window [L005] (MEASURED); that gap is the case for needing the baseline |
| More transmit-only boards | Beacon-only boards, for example on the basement ceiling | No new hardware; boards below the first floor add little to floors 1 and 2 | Simulation only: floor-correct share 0.13, 0.10, 0.11 for fleets A, B, C under a line-of-sight model; 0.52, 0.55, 0.56 under a multipath model [L048] (SYNTHETIC; NOT-PROVEN). The advice to buy one and measure was overruled by the no-new-hardware rule |

### 15.2 Placement with other hardware: a bounded hypothesis

The owner's expectation is that placement would probably work to some degree with other hardware. The study records this as a HYPOTHESIS (CLAIMED). Its support is limited to two things: the supervised fingerprint result in one room [L041] (MEASURED, one room, supervised, three sensors) and the multi-antenna phase literature (lab conditions). It was never tested on other hardware, and the fleet's own results run against it: the link-line model failed [L118], and the radio-tomography estimator on nine nodes performed at chance on room and showed a room-bias artefact (chapter 8). A fair bound is therefore this: some placement skill is plausible if the sensor carries phase or more than one antenna per node and the person is moving, and the study provides no evidence of how much. It supplies no evidence at all for a still person, which the instrument absorbs in 3 to 8 seconds regardless of placement (chapter 12; section 12.5).

### 15.3 Why the constraints make the negative result general

These are not the constraints of a bench. They are the constraints of a real home product. A consumer system must use cheap single-antenna parts, install in whatever outlets the household has, learn without asking occupants to walk labelled routes, work in a house that animals never leave, and run without a clean empty-house recording. A method that needs any of the five relaxations is a laboratory method or a method for a different product. The negative result is therefore relevant beyond this house: it describes what a home deployment under ordinary constraints can expect from single-antenna nodes, even though the numbers belong to one house (chapter 14).

**Chapter summary.** Five ideas remain open, and each needs one constraint relaxed: more antennas and phase, labelled walk-throughs, moved nodes, an empty-house baseline, or additional transmitters. The record bounds each. The one supervised result is one room and does not transfer, the geometric model was falsified, an empty baseline cannot be had, and the transmitter simulation was not proven. The claim that placement would work with other hardware stays a hypothesis tagged CLAIMED. Because the constraints are those of a real home product, the negative result matters beyond this house.


## 16. Recommendations for upstream

These recommendations are offered by one downstream user to the maintainers and to other researchers who build on RuView. They come from the evidence in the preceding chapters, and each states that evidence. They are written as proposals, to be adopted, adapted or declined, and they refer to the open pull requests described in Part II where code already exists.

### 16.1 Claims and evaluation

1. **Tag every capability claim in the README as MEASURED, CLAIMED or SYNTHETIC.** A MEASURED tag should name its reproducer. Evidence: the 17-keypoint skeleton in the observatory is built from a canned animation with sine and hash noise seeded by person index, with CSI entering only as a gain [L127] (MEASURED, code reading), and a reader cannot tell this from a pose estimate without reading the code.

2. **Publish cross-subject, cross-environment and held-out-day numbers next to every random-split number.** Evidence: a headline of 81.63 % torso PCK@20 on a random split compares with about 11.6 % leakage-free cross-subject, against a target of 30 % [L042] (CLAIMED, issue-tracker review). The upstream pretrained model card reports zero-shot cross-subject 64 % and cross-environment about 10 % [L076] (CLAIMED). Outside projects show the same pattern: 82.7 % on a random split against 52.3 % grouped by session on identical data [L124] (CLAIMED, external).

3. **Require a mean-pose baseline and a shuffled-label baseline for any pose or classification number.** Evidence: the pose evaluation reviewed has no mean-pose baseline, and a random frozen encoder with a trained readout came within 2 to 4 points of a trained one [L076] (MEASURED, code reading).

4. **Make empty-room tests include animals and multi-day held-out data.** Evidence: in an animals-only window the live sink reported presence on 6,098 of 6,101 lines [L006] (MEASURED). A single-room, single-sleeper model card cannot show this failure, and upstream's own architecture decision records an incumbent per-node classifier with 99.8 % false positives on an empty room [L076] (CLAIMED, their measurement).

5. **Report false presence on confirmed-empty time, not detection alone.** Evidence: the displayed label said present in 100 % of minutes on all three scored days, so detection was 100 % and false presence 100 % of confirmed animals-only minutes (waking AUC 0.50) [L012] (MEASURED). A detection rate with no false-presence rate is uninformative. Per-node moving energy was essentially equal empty and occupied, and firmware presence was 0.99 to 1.00 on every node in the empty window [L007] (MEASURED). The study's comment on upstream issue #2522 (empty-room false presence) points to the same measurements [L005] (MEASURED).

### 16.2 Behaviour of shipped code

6. **Default off, and visibly label, any simulated or demo output in the UI.** Evidence: after five failed reconnects the browser client invented sensing frames and rendered them like live values while the sink reported steady presence; the fix (opt-in, label "INVENTED DATA - NOT MEASURED") is merged as #1794 [L127; Part II] (MEASURED).

7. **Fix or disable the stock fall detector.** Evidence: 13 "Fall detected" events across 5 nodes overnight with the house asleep, from a detector present since the first edge commit, whose only consumer was unused; setting the threshold to 0 gave no events afterwards [L065] (MEASURED). Default-off with an explicit calibration step is a conservative option.

8. **Make demo CLI modes refuse to run silently on synthetic data.** Evidence: reading the code, the sensing-server `--embed`, `--build-index` and `--pretrain` modes run on synthetic sine data when no capture is supplied, and so produce plausible outputs that carry no sensing content: upstream issue #2591 (CLAIMED, from code reading). A hard error, or a printed banner with a non-zero exit unless `--synthetic` is passed, would remove the risk.

9. **State hardware limits next to each capability.** A single-antenna C6 cannot support angle-of-arrival or cross-antenna phase methods, and its absolute phase re-randomises every packet, and the instrument absorbs a still body in 3 to 8 seconds (chapter 12). Evidence: breathing, heart rate, still-person detection and position all failed on this hardware, and a reader needs the hardware limit beside each of those capabilities [L060, L063, L046] (MEASURED). A table of capability by chip, with the antenna count and the output type (amplitude or phase), would prevent the mismatch.

### 16.3 Reliability items from the study

10. **Merge the reliability firmware if not already in.** Each has a silicon measurement (WORKED F1, F2, F4-F7):
- link-rate watchdog: 15 of 15 acts recovered within 3 minutes [L178] (MEASURED); reassociation merged as #2278, the watchdog itself open as #2593;
- uplink watchdog (stalled-TX and IP-dead rules): 0 false reboots in 3 nights; open as #2589 and #2594, with rule 3 host-tested only [L179] (MEASURED for rules 1-2);
- rollback bootloader installed over the air: 3 of 9 boards valid to 9 of 9 [L157] (MEASURED); not offered upstream because it is fleet-local;
- multi-hop ESP-NOW sync: 6 of 9 to 9 of 9 nodes valid, two leaders to one [L087] (MEASURED); not offered because the protocol chain is fleet-local, so a maintainer decision is needed on adopting it.

The send-callback race fix (133 stall lines a day to 0 on one canary node [L190]) is not separately offered.

### 16.4 Privacy hygiene for contributors

11. **Guard contributions against house data.** A prior audit missed real device addresses in test fixtures, and a non-absolute hook path left the pre-push hook absent on branches cut from main [L130] (MEASURED). Contributors should grep added lines for real addresses and use an absolute hook path.

**Chapter summary.** The proposals reduce to one rule: say what was measured, on what split, against what baseline, and with what hardware. Evidence from this study shows how a random split, a missing baseline, a synthetic fallback or a single-room model card can produce a number that does not hold in a home. Four concrete code changes are named: default-off simulation, a disabled fall detector, demo modes that refuse synthetic data, and the reliability firmware. Pull requests that carry the reliability items are listed with their status, and the fleet-local items need a maintainer's decision. The tone intended is collegial: these were the failures this study hit first.


