# BFI research and next experiment

Primary-source snapshot: September 29, 2026. Identification, activity recognition
and positioning use different labels and evaluation protocols. The results below
are **CLAIMED by their authors**, not reproduced RuView results. They identify
useful baselines; they do not establish one universally best model.

## Recent work and practical fit

| Work and verified date | Task and evaluation evidence | Data/code and requirements |
|---|---|---|
| [BFId, CCS, October 2025](https://publikationen.bibliothek.kit.edu/1000185756/168100988) | Closed-set walking identification: 197 recruited, **161 usable BFI identities**. Authors report 99.5% ± 0.38 on normal walking, using 80:20 sequence splits repeated five times. This does not establish performance on a new acquisition day. | Standardized quantized angles plus inter-report time, LSTM classifier; 4×2 feedback at 160 MHz, about 10 reports/s. [Dataset access](https://ps.tm.kit.edu/bfid-dataset/) requires a signed noncommercial research agreement. Our 2×2/80 MHz input differs. |
| [BeamSense, Computer Networks, February 2025](https://www.sciencedirect.com/science/article/pii/S1389128624008521) | Activity recognition across three environments, three subjects and 20 activities; few-shot adaptation evaluates changes in deployment. Its accuracy comparisons depend on that protocol. | [Author code](https://github.com/kfoysalhaque/BeamSense) is GPL-3.0 and links [public data](https://huggingface.co/datasets/foysalhaque/BeamSense). Preprocessing separates stations and forms 0.1 s windows containing roughly ten BFI reports. Current local captures cannot supply that density. |
| [Si-FI, Computer Networks, February 2026](https://www.sciencedirect.com/science/article/pii/S1389128625008734) | Simultaneous activity classification using proximity, coarse activity and fine activity stages; three subjects, 20 activities and three environments. Authors report up to 99% classification accuracy and adaptation from 15 seconds of new data per task. | [Author code](https://github.com/kfoysalhaque/Si-FI) is GPL-3.0 and links datasets. Documented input is 802.11ac, 80 MHz BFAs extracted with Wi-BFI. Proximity to a station is part of the method; person identification is a separate task. |
| [LeakyBeam, NDSS, February 2025](https://www.ndss-symposium.org/wp-content/uploads/2025-5-paper.pdf) | Moving/stationary occupancy: eight APs, nine deployment settings, 49 hours. Reported overall TPR/TNR are 82.7%/96.7%; distance experiments extend to 20 m. These aggregate rates are not specifically the 20 m result. | Uses reconstructed BFI structure, antenna diversity and subcarrier fusion. Ordinary CSI phase correction cannot simply be applied to SVD-derived BFI. No author code/data release was located in the inspected primary sources. |
| [BFMLoc, ICC 2025; transferable BFI positioning, TMC 2026](https://nicelab.us/Recent-Publications/) | Author records verify transformer-based BFM positioning and its 2026 extension. The inspected primary pages did not provide enough detail to verify numeric performance or the full split protocol. | Spatial labels and deployment evaluation would be required. Public code/data availability remains unverified. The title alone does not establish device-free person tracking or AoA support. |
| [Wi-BFI, WiNTECH 2023](https://arxiv.org/abs/2309.04408), foundational extraction tool | Extraction rather than a sensing accuracy benchmark. Paper documents VHT/HE, SU/MU and multiple channel widths. | [GPL-3.0 author implementation](https://github.com/kfoysalhaque/Wi-BFI) includes traces, integer angles and reconstructed matrices. Its documented antenna configurations omit 2×2; adaptation and fixture checks are needed. EHT support is not documented. |

Si-FI's journal issue is February 2026 although its DOI contains `2025`; BeamSense's
issue is February 2025 although its DOI contains `2024`. Dates above follow the
publisher issue, not the DOI suffix. Another paper also uses the name BeamSense;
the HAR row specifically refers to Haque and colleagues' linked implementation.

### Public BFI data to investigate next

The author-released [CSI-BFI-HAR repository](https://github.com/kfoysalhaque/CSI-BFI-HAR)
documents activity, day, device and pseudonymous subject identifiers, separate
single/multiple-subject recordings, and environment/LoS metadata. This permits
explicit evaluation across days, people and rooms. Its example BFI extraction is
**AC MU 3×1, 80 MHz**, so it cannot pass through the current HE SU decoder unchanged.
Public Hugging Face and IEEE DataPort links are provided; archive contents and
dataset-use terms need inspection before a download. A code license must not be
treated as a dataset license. Use activity labels for the initial BFI task.

## What RuView has actually measured

**MEASURED:** captured HE SU Nr=2, Nc=2, Ng=4, codebook 1 reports at 20, 40 and
80 MHz, with 64, 122 and 250 reported tone groups respectively. Integer angles,
selected controls and SNR matched tshark for all 21, 66, 96 and one reports in
the documented trials. Matrix reconstruction has a separate synthetic oracle.
[Reproducer and evidence](../bfi/OPTIMIZATION.md)

The two higher-traffic captures averaged **1.4659 and 2.1317 reports/s across
45 seconds**, with full-window silence intervals of **23.291 and 16.393 seconds**.
Even within the observed report spans, maximum gaps were 5.361 and 6.437 seconds.
These are intermittent samples; no movement, identity, AoA or position accuracy
has been measured. [Initial acceptance and negative repeats](../bfi/VALIDATION.md)

## Proposed implementation order

1. **Keep protocol identity with every sample.** Key features by VHT/HE, SU/MU,
   dimensions, angle bit widths, Ng, channel width and RU/tone allocation. The
   radio's configured 80 MHz width did not prevent 20/40 MHz feedback. Preserve
   separate tone grids, real timestamps and missingness masks; reject unsupported
   EHT, segmentation and partial allocations explicitly.
2. **Establish simple feature baselines.** Compare standardized integer angles
   plus elapsed time against circular encoding of phi and bounded psi. Fit
   normalization on training recordings only. Add a timing-only control to detect
   shortcuts caused by traffic or scheduling rather than movement.
3. **Ablate reconstructed steering matrices.** Compare real/imaginary V with
   per-column products `v_j v_j^H`, checking stream ordering. This is a proposed
   phase-invariant feature, not a measured improvement. For our square unitary
   2×2 V, the full product **`V V^H = I` discards the useful variation**. V alone
   also cannot uniquely recover the original channel H: other SVD factors are
   missing. Average SNR does not restore them.
4. **Choose models after the data contract.** A small LSTM/GRU on irregular
   observations is a practical baseline. Evaluate few-shot/domain adaptation
   only after holding out whole acquisition sessions and deployments. A public
   CSI HumanID checkpoint is a separate CSI experiment; transfer to HE BFI needs
   its own measurement.

## Next measurable experiment

**PROPOSED, not executed by this research task:** first repeat the fixed
30-second traffic/45-second observation profile with unchanged placement and an
awake, stationary client. Report both full-window and traffic-window coverage.
Use an initial engineering gate of reports in at least 27/30 one-second traffic
bins and no traffic-window silence longer than two seconds, repeated three times.
This is a preregistered project target, not a published minimum sampling theorem.
If it fails, report the continuity failure before training on local sequences.

Once that gate passes, collect a small labeled motion pilot: three independent
sessions, each with two empty-room and two walking recordings in randomized
order. Keep device placement and traffic identical between labels and record
ground-truth times separately. Freeze feature choices using the first two
sessions; reserve the third for evaluation. Treat twelve recordings as a pilot,
not a generalization benchmark. Add occupied-but-still trials in a later stage
before claiming occupancy detection.

Compare a circular-angle-change threshold with a small sequence model. Report
recording-level balanced accuracy, confusion counts, false-alarm rate, latency
and the fraction of time for which the system abstains. Include constant and
timing-only baselines. Never fill a long observation gap with synthetic reports
and score the interpolated interval as measured sensing.
