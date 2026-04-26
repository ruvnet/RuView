# ADR-091: Stand-off Radar Tier Research — 77 GHz High-Power and 100–200 GHz Coherent Sub-THz

| Field          | Value                                                                                   |
|----------------|-----------------------------------------------------------------------------------------|
| **Status**     | Proposed — Research only. No production hardware integration. Decision deferred pending sub-$1k COTS sub-THz transceiver availability and clear non-export-controlled use case. |
| **Date**       | 2026-04-26                                                                              |
| **Authors**    | ruv                                                                                     |
| **Refines**    | ADR-021 (60 GHz / mmWave vital-signs pipeline)                                          |
| **Companion**  | `docs/research/quantum-sensing/16-ghost-murmur-ruview-spec.md` §6.3, ADR-029 (RuvSense multistatic), ADR-089 (nvsim simulator), ADR-090 (Lindblad extension) |

## 1. Context

### 1.1 Why this question now

On Good Friday 3 April 2026 the press reported a CIA system called "Ghost Murmur"
— a Lockheed Skunk Works NV-diamond + AI sensor reportedly used in the recovery
of an F-15E pilot in southern Iran. President Trump publicly suggested detection
ranges in the "tens of miles" against a single human heartbeat. RuView shipped
a research spec (`16-ghost-murmur-ruview-spec.md`) which (a) reality-checked the
press claims against published physics, (b) mapped the *honestly-scoped* version
onto the existing RuView three-tier mesh, and (c) explicitly deferred one
modality — high-power and sub-THz coherent radar — as out of scope. From §6.3
of that spec:

> 77 GHz automotive radars at higher power and 100–200 GHz coherent sub-THz
> radars **can** resolve cardiac micro-Doppler at 50–500 m in clear LOS. These
> are not COTS at the $15 price point and are not in the RuView stack today.
> They are also subject to ITAR / export-control review and **explicitly out of
> scope** for this open-source project.

That sentence is the trigger for this ADR. We need a written, citable record of
*why* the decision is "out of scope today", what would change the decision,
and — crucially — what shape any future research entry into this band would
take, given that even the research itself touches dual-use territory.

### 1.2 What gap a higher-frequency / higher-power tier would close

RuView's existing modality coverage (per the CLAUDE.md crate table):

| Modality | Crate / ADR | Honest LOS range for HR | Through-wall HR |
|---|---|---|---|
| WiFi CSI 2.4/5/6 GHz | `wifi-densepose-signal`, ADR-014, ADR-029 | 1–3 m (presence to 30 m) | 1 wall, weak |
| 60 GHz FMCW (MR60BHA2) | `wifi-densepose-vitals`, ADR-021 | 1–10 m | drywall only |
| NV-diamond magnetometer | `nvsim` (simulator), ADR-089/090 | <1 m (gradiometric, shielded) | n/a |

The ceiling of this stack on cardiac micro-Doppler in clear line-of-sight is
**~10 m** (60 GHz tier, ADR-021 / spec §6.1). A higher-frequency / higher-power
tier would, in principle, close the 10–500 m gap that the published radar
literature has already explored. The two candidate bands:

1. **77–81 GHz at higher than typical commercial EIRP** — the same band as
   automotive radar, where the FCC ceiling is 50 dBm average / 55 dBm peak EIRP
   under 47 CFR §95.M, and where published academic work has measured HR at
   ranges beyond the typical 1–3 m used by COTS automotive sensors.
2. **100–200 GHz coherent sub-THz radar** — where λ ≈ 1.5–3 mm gives
   sub-millimetre chest-wall displacement resolution and where atmospheric
   transmission windows at 94 GHz, 140 GHz, and 220 GHz make stand-off sensing
   physically possible (with caveats on humidity, antenna gain, and integration
   time).

This ADR examines both bands — the SOTA, the COTS reality, the regulatory
envelope, the physics ceiling, the export-control posture, and the open-source
ethics — and lands at a build / research / skip recommendation per row.

## 2. SOTA: 77–81 GHz automotive radar at higher power

### 2.1 Current COTS chips at the $20–$200 price point

The 76–81 GHz band is now densely populated with single-chip CMOS / SiGe
transceivers. Representative parts:

| Chip | Vendor | Tx / Rx | IF BW | Notes |
|---|---|---|---|---|
| AWR1843 | Texas Instruments | 3 Tx / 4 Rx | up to ~10 MHz IF | Single-chip 76–81 GHz with on-die DSP, MCU, radar accelerator. Long-range automotive ACC, AEB. ([TI AWR1843](https://www.ti.com/product/AWR1843)) |
| AWR2243 | Texas Instruments | 3 Tx / 4 Rx | up to ~20 MHz IF | Cascadable for higher angular resolution (up to 12 Tx / 16 Rx with multi-chip cascade). ([TI AWR2243](https://www.ti.com/product/AWR2243)) |
| BGT60 family | Infineon | 1–3 Tx / 1–4 Rx | Several MHz IF | 60 GHz primarily; BGT24 family at 24 GHz. Smaller, lower power, gesture / presence focus. |
| TEF82xx | NXP | up to 4 Tx / 4 Rx | several MHz IF | Automotive-grade 76–81 GHz. |

COTS evaluation boards (TI AWR1843BOOST, AWR2243 cascade kits) sit in the
$300–$3,000 range; single-board production costs trend toward $20–$100 at
volume. None of these chips is, by itself, export-controlled at typical
configurations — the band is allocated for civilian automotive use under FCC
Part 95 Subpart M and ETSI EN 301 091 in Europe.

**EIRP envelope**: 47 CFR §95.M (and the historical §15.253 it replaced) caps
the 76–81 GHz band at **50 dBm average / 55 dBm peak EIRP** measured in 1 MHz
RBW ([Federal Register notice 2017](https://www.federalregister.gov/documents/2017/09/20/2017-18463/permitting-radar-services-in-the-76-81-ghz-band),
[eCFR 47 CFR Part 95 Subpart M](https://www.ecfr.gov/current/title-47/chapter-I/subchapter-D/part-95/subpart-M)).
That is roughly 100 W EIRP average, 316 W peak. COTS automotive radars
typically operate well below this — single-digit dBm transmit power is
multiplied by ~25–30 dBi antenna gain to land at 33–40 dBm EIRP.

### 2.2 What "higher power" actually means in regulatory terms

Three regulatory paths exist for an open-source project that wants to push
beyond typical commercial deployment power:

1. **Stay inside FCC Part 95 §95.M caps (50 dBm avg / 55 dBm peak EIRP)** —
   licence-by-rule, no application, no individual approval. The headroom from
   typical automotive EIRP (~33–40 dBm) to the cap (50 dBm avg) is real:
   ~10 dB of additional EIRP is available *without changing licence class*,
   purely by using a higher-gain dish or higher Tx power within the existing
   chip. This is the upper bound of "stand-off radar that is still part-95
   legal".
2. **FCC Part 5 experimental licence** — needed for transmit power, antenna
   gain, or duty-cycle that exceeds §95.M. Application-based, time-bounded,
   non-renewable beyond limits. Typical academic radar ranges (e.g. the
   long-range cardiac measurements in §2.3 below) operate under this regime.
3. **No US authorisation at all** — only legal as receive-only, or as a
   simulator. Any unlicensed transmission above §95.M at 76–81 GHz is a
   prohibited emission under 47 CFR §15.5 / §95.335.

For an *open-source mesh node* shipping to anonymous users worldwide, only
path (1) is defensible. Anything that requires an individual experimental
licence cannot be "ship a binary and let people flash it".

### 2.3 Published cardiac micro-Doppler at 77 GHz beyond 5 m

The 77 GHz cardiac literature is dominated by short-range work (0.3–2 m), e.g.:

- Chen et al. (2024). "Contactless and short-range vital signs detection with
  doppler radar millimetre-wave (76–81 GHz) sensing firmware." *Healthcare
  Technology Letters*. ([PMC11665778](https://pmc.ncbi.nlm.nih.gov/articles/PMC11665778/),
  [Wiley HTL 2024](https://ietresearch.onlinelibrary.wiley.com/doi/full/10.1049/htl2.12075))
  — TI IWR1443BOOST at 0.30–1.20 m, suggested 0.6 m.
- Wang et al. (2020). "Remote Monitoring of Human Vital Signs Based on 77-GHz
  mm-Wave FMCW Radar." *Sensors* 20, 2999.
  ([PMC7285495](https://pmc.ncbi.nlm.nih.gov/articles/PMC7285495/),
  [MDPI Sensors 2020](https://www.mdpi.com/1424-8220/20/10/2999)) — typically
  short-range bench measurements.
- Liu et al. (2022). "Real-Time Heart Rate Detection Method Based on 77 GHz
  FMCW Radar." *Micromachines* 13, 1960.
  ([PMC9693980](https://pmc.ncbi.nlm.nih.gov/articles/PMC9693980/),
  [MDPI](https://www.mdpi.com/2072-666X/13/11/1960)) — 2.925% mean HR error,
  short-range.
- Iyer et al. (2022). "mm-Wave Radar-Based Vital Signs Monitoring and
  Arrhythmia Detection Using Machine Learning." *Sensors*.
  ([PMC9104941](https://pmc.ncbi.nlm.nih.gov/articles/PMC9104941/))

The most cited *long-range* radar cardiac measurement is at 24 GHz, not 77 GHz:

- **Massagram, W., Lubecke, V. M., Høst-Madsen, A., Boric-Lubecke, O. (2013).
  "Parametric Study of Antennas for Long Range Doppler Radar Heart Rate
  Detection."** *IEEE EMBC* / republished in *PMC*.
  ([PMC4900816](https://pmc.ncbi.nlm.nih.gov/articles/PMC4900816/),
  [PubMed 23366747](https://pubmed.ncbi.nlm.nih.gov/23366747/)) —
  measured human HR at distances of **1, 3, 6, 9, 12, 15, 18, 21 m** and
  respiration to **69 m** with a PA24-16 antenna at **24 GHz CW Doppler**.
  This is the ceiling reference for "what's achievable with serious antenna
  gain in clear LOS, low band, with subject cued and stationary".

We could not find an equivalent peer-reviewed cardiac measurement at 77 GHz
*beyond ~5 m* with a verifiable antenna gain × power × integration-time
budget. The work that exists at 77 GHz is overwhelmingly bench-scale (≤ 2 m).
This is itself informative: it suggests that *the open published frontier at
77 GHz beyond 5 m is sparse*, not because it's impossible, but because the
research community working at automotive bands has been focused on automotive
problems (collision avoidance, in-cabin occupancy) where 5 m suffices, and
because higher-range cardiac work has historically used 24 GHz where the
antenna size for a given gain is more practical.

### 2.4 Detection range as a function of antenna gain × power × integration time

The radar equation for chest-wall displacement detection scales roughly as:

```
SNR ∝ (P_t · G_t · G_r · σ_chest) / (R^4 · k T B · NF) · √(t_int / T_coh)
```

where σ_chest ≈ 10⁻³–10⁻² m² for the cardiac scatterer at 77 GHz, NF ≈ 10–15 dB
on COTS chips, and integration time t_int is bounded by T_coh ≈ 0.5–1 s
(physiological coherence — the heart period itself).

Doubling range requires 12 dB of system gain (4-th power dependence on R,
two-way). At the part-95 §95.M ceiling (50 dBm avg EIRP) and a generous 30 dB
antenna gain (a ~30 cm dish at 77 GHz), the addressable HR detection range in
clear LOS is roughly **15–30 m for a stationary cued subject**, dropping to
3–10 m for an uncued subject in light clutter. Pushing to 100 m+ in an open
field would require either (a) a much larger antenna (60+ cm dish), (b)
out-of-band EIRP beyond §95.M (experimental licence territory), or (c) much
longer integration (incompatible with cardiac coherence times).

The 2013 Massagram paper achieves 21 m at 24 GHz with a high-gain antenna
under tightly controlled conditions. Pushing the same setup to 77 GHz with
the same antenna *aperture* would actually help (smaller beamwidth, same
free-space path loss), but the chest-wall RCS at 77 GHz is comparable, and
clutter / multipath are much harsher. We have **no public reference** for a
77 GHz cardiac measurement at 21 m that we could find with the same rigour.

### 2.5 Cost ceiling for an open-source mesh node

An open-source mesh node spec implies "ships in a kit, does not require
individual licensing, fits the existing PoE / mini-PC edge model". That
implies:

- Single-chip transceiver at $20–$100 BOM.
- Antenna assembly at $50–$200 (high-gain dish or printed array).
- Mini-PC or Pi 5 host at $80.
- Total under $500 to be plausible.

The chip cost is already met by COTS. The antenna and host are met. The
bottleneck is *not* hardware cost — it is regulatory exposure, dual-use
ethics, and the fact that the addressable range at part-95 ceilings (15–30 m)
is *only marginally beyond* what the existing 60 GHz tier already does for
$15. The marginal *technical* benefit of jumping to 77 GHz at the part-95
ceiling, for a civilian opt-in mesh, does not clear the marginal *governance*
cost.

## 3. SOTA: 100–200 GHz coherent sub-THz radar

### 3.1 Why sub-THz

At 140 GHz, λ ≈ 2.14 mm. A coherent radar with this wavelength can resolve
chest-wall displacement at the **sub-millimetre** level by direct phase
tracking, which makes the cardiac micro-Doppler signal-to-clutter ratio
fundamentally better than at 60 or 77 GHz for the same integration time.
Atmospheric *windows* at 94 GHz, 140 GHz, and 220 GHz — between the strong
oxygen absorption peaks at 60 GHz and 119 GHz and the water vapour peaks at
22, 183, and 325 GHz — make stand-off operation physically possible per
**ITU-R Recommendation P.676** ([ITU-R P.676-11](https://www.itu.int/dms_pubrec/itu-r/rec/p/R-REC-P.676-11-201609-I!!PDF-E.pdf),
[ITU-R P.676-9](https://www.itu.int/dms_pubrec/itu-r/rec/p/R-REC-P.676-9-201202-S!!PDF-E.pdf)).

### 3.2 Atmospheric attenuation table (clear-air, ITU-R P.676)

Order-of-magnitude values for one-way attenuation through standard atmosphere
at sea level, taken from ITU-R P.676-11 Annex 1 / 2 figures (approximate
values; consult the recommendation for precise numbers at any (T, P, ρ)):

| Frequency | Dry air, dB/km | 7.5 g/m³ humid, dB/km | Notes |
|---|---|---|---|
| 60 GHz | ~14 | ~14.5 | O₂ absorption peak — terrible for stand-off |
| 77 GHz | ~0.4 | ~0.5 | Allocated for automotive radar |
| 94 GHz | ~0.4 | ~0.7 | First major window above 60 GHz |
| 119 GHz | ~2.5 | ~3 | O₂ subsidiary peak |
| 140 GHz | ~0.5 | ~1.5 | Second major window |
| 183 GHz | ~30+ | ~100+ | H₂O peak — unusable for outdoor stand-off |
| 220 GHz | ~2 | ~5 | Third window |
| 325 GHz | ~10+ | ~50+ | H₂O peak |
| 380 GHz | ~3 | ~20 | Imaging-band window, very humidity-sensitive |

For a 100 m one-way clear-LOS link at 140 GHz in 7.5 g/m³ humidity, atmospheric
attenuation alone is ~0.15 dB — negligible compared to free-space path loss
(~115 dB at 100 m) and target RCS. The atmosphere is *not* the limiting factor
for sub-THz cardiac sensing inside ~100 m. **Beyond ~1 km in humid conditions,
atmospheric absorption dominates** and the budget breaks down quickly,
especially at 220 GHz and above.

### 3.3 COTS chipsets and academic platforms

The sub-THz commercial landscape in 2026 is sparse and expensive:

- **Analog Devices HMC8108** — 76–81 GHz transceiver. Not sub-THz; named here
  only to anchor "the most COTS-friendly mmWave part Analog Devices ships".
- **Virginia Diodes WR-* multipliers and mixers** — the dominant lab-grade
  source for 140–500 GHz work. Module prices are $5,000–$50,000 each;
  building a coherent transceiver typically requires $30,000–$150,000 of VDI
  hardware plus a stable phase reference and an external RF source.
- **Wasa Millimeter Wave imagers** — passive imagers around 90 / 220 / 380 GHz.
  Receive-only.
- **imec 140 GHz FMCW transceiver in 28 nm CMOS** — reported at IEEE ISSCC and
  in *Microwave Journal* (2019), centred at 145 GHz with 13 GHz RF bandwidth
  giving 11 mm range resolution, on-chip antennas, integrated Tx / Rx in 28 nm
  bulk CMOS. ([Microwave Journal 2019](https://www.microwavejournal.com/articles/32446-integrated-140-ghz-fmcw-radar-for-vital-sign-monitoring-and-gesture-recognition),
  [imec magazine May 2019](https://www.imec-int.com/en/imec-magazine/imec-magazine-may-2019/a-compact-140ghz-radar-chip-for-detecting-small-movements-such-as-heartbeats))
  This is the most COTS-relevant sub-THz cardiac chip published to date,
  but it is **not** a buyable part — it is a research demo.
- **Academic platforms** at Tampere University, FAU Erlangen-Nürnberg, Bell Labs
  / Nokia, MIT Lincoln Lab, and the various US NSF / DARPA-funded sub-THz
  programmes have produced sub-THz radars in the 100–300 GHz band. None of
  these is a ship-it part.

### 3.4 Coherent vs. incoherent

A *coherent* sub-THz radar maintains phase reference between Tx and Rx (and
ideally across multiple Tx / Rx channels for MIMO or multistatic operation).
Coherent processing buys:

- **Matched-filter SNR scaling**: SNR improves linearly with integration
  time t (vs. √t for incoherent), bounded by the cardiac coherence
  time T_coh.
- **Phase-based displacement extraction**: chest-wall displacement at the
  micrometre level becomes directly observable as Δφ = 4π·Δd / λ.
- **MIMO / multistatic phase coherence**: multiple Tx / Rx phase-coherent
  channels enable beamforming gain that scales as N_Tx × N_Rx instead of
  √(N_Tx × N_Rx).

It costs:

- **Sub-picosecond clock distribution** between channels at sub-THz frequencies
  (a 1 ps clock skew at 140 GHz is 50° of phase error).
- **Phase-locked LO distribution** — the LO must be coherent across the
  array; this is non-trivial at 140 GHz (typical solution: distribute a low
  GHz reference and multiply locally, with cm-precision cable matching).
- **Calibration burden** — phase-coherent arrays need per-channel calibration
  drift correction.

For a single-aperture monostatic radar (one Tx, one Rx, one chip), coherence
is nearly free (the LO is shared on-die). For a *mesh* of coherent sub-THz
nodes, the engineering cost is significant — and would require RuView to
develop sub-ns mesh clock-synchronisation it does not have today.

### 3.5 Published cardiac micro-Doppler at sub-THz

The published peer-reviewed cardiac literature at 100–300 GHz is sparse but
not empty:

- **Mostafanezhad & Boric-Lubecke (2014).** "Benefits of coherent low-IF for
  vital signs monitoring." *IEEE Microw. Wireless Compon. Lett.* 24. — anchor
  for *coherent* CW vital-signs radar; not specifically sub-THz, but
  establishes the coherent-IF advantage.
- **imec (2019) — 140 GHz FMCW transceiver demonstration.** Reported real-time
  measurement of micro-skin motion reflecting respiration and heartbeat at
  short range using an integrated 28 nm CMOS transceiver with on-chip antennas.
  Cited above; engineering demo, not a published systematic range study.
  ([Microwave Journal 2019](https://www.microwavejournal.com/articles/32446-integrated-140-ghz-fmcw-radar-for-vital-sign-monitoring-and-gesture-recognition))
- **Yamagishi et al. (2022).** "A new principle of pulse detection based on
  terahertz wave plethysmography." *Scientific Reports* 12, 2022.
  ([Nature SREP](https://www.nature.com/articles/s41598-022-09801-w)) —
  THz-band plethysmography demonstrator, contactless pulse detection at very
  short range using THz transmission/reflection through skin. Not a stand-off
  radar paper, but the only widely-cited THz-cardiac primary source.
- **Zhang et al. (2021).** "Non-Contact Monitoring of Human Vital Signs Using
  FMCW Millimeter Wave Radar in the 120 GHz Band." *Sensors* 21.
  ([PMC8070581](https://pmc.ncbi.nlm.nih.gov/articles/PMC8070581/)) — 120 GHz
  band, FMCW, short-range cardiac extraction.

**Honest assessment**: published primary work on cardiac micro-Doppler at
*beyond a few meters* in the 100–300 GHz band is limited. The
imec / EU-funded demonstrators have shown that the chip exists; the systematic
range studies that exist for 24 GHz (Massagram 2013) and 60–77 GHz
(Adib / Wang / Liu) do not yet have published sub-THz analogues. Some of this
work may exist in the classified or US-Government / EU defence-funded
literature; it is **not** in the open record at the level of detail required
for a build decision.

## 4. Physics ceiling for RuView's heartbeat-mesh use case

### 4.1 Cardiac signal vs. distance, multi-band comparison

For a stationary, cued, line-of-sight subject with chest-wall displacement
~0.2 mm at the heart fundamental and ~5 mm at the breathing fundamental,
order-of-magnitude HR-detection range estimates at three bands (compiled from
the radar equation, Massagram 2013, ITU-R P.676, and standard chest-RCS
estimates):

| Band | λ | Required Δφ for HR | Free-space loss @ 30 m | Atm loss @ 30 m | Estimated HR range (cued LOS, COTS Tx + 30 dBi antenna, part-95) |
|---|---|---|---|---|---|
| 24 GHz CW | 12.5 mm | 0.36° | 89 dB | <0.01 dB | 21 m measured (Massagram 2013) |
| 60 GHz FMCW | 5.0 mm | 0.9° | 97 dB | 0.4 dB | 5–10 m (ADR-021 / spec §6.1) |
| 77 GHz FMCW | 3.9 mm | 1.2° | 99 dB | 0.01 dB | ~15–30 m (estimated, no rigorous public ref beyond 5 m) |
| 140 GHz FMCW | 2.1 mm | 2.2° | 105 dB | 0.04 dB | ~30–100 m (estimated, sparse open lit) |
| 220 GHz FMCW | 1.4 mm | 3.3° | 109 dB | 0.15 dB | ~30–100 m (estimated, sparse open lit, humidity-sensitive) |

The phase-displacement resolution *improves* with frequency (Δφ for the same
displacement scales as 1/λ), but the link budget *degrades* (R⁻⁴ in
two-way path loss, plus atmospheric absorption, plus higher noise figure on
sub-THz LNAs). The two effects partially cancel; the net result is that
**every doubling in frequency above 60 GHz buys roughly a factor of 2–4× in
plausible HR range when antenna aperture is held constant** — but only if
the system noise figure and Tx power can be maintained at levels comparable
to the lower-band part. Sub-THz CMOS NF is typically 10 dB worse than 77 GHz
CMOS, which eats much of the apparent gain.

### 4.2 Two-way path loss + atmospheric absorption

| Range | 77 GHz total loss | 140 GHz total loss | 220 GHz total loss |
|---|---|---|---|
| 1 m | 70 dB + 0 | 76 dB + 0 | 80 dB + 0 |
| 10 m | 90 dB + 0.01 | 96 dB + 0.03 | 100 dB + 0.1 |
| 100 m | 110 dB + 0.1 | 116 dB + 0.3 | 120 dB + 1 |
| 1 km | 130 dB + 1 | 136 dB + 3 | 140 dB + 10 |
| 10 km | 150 dB + 10 | 156 dB + 30 | 160 dB + 100 |
| 65 km (40 mi) | 168 dB + 65 | 174 dB + 200+ | 178 dB + impossible |

**Observations**:

- At 1 km, 220 GHz loses 9 dB more to atmosphere than 77 GHz; at 10 km it
  loses 90 dB more. Sub-THz is fundamentally a sub-1-km modality in humid air.
- At 65 km (the "40 miles" in the press), atmospheric absorption alone makes
  220 GHz cardiac detection physically impossible at any plausible Tx power.
  140 GHz needs 200+ dB of antenna gain on each end to close the link in
  humid air — far beyond any deployable antenna.
- **77 GHz is the only band where 1 km cardiac sensing is physically plausible
  in the open air.** It is also the band that is closest to civilian COTS.

### 4.3 Required antenna gain × power × integration time

Holding integration time at 0.5 s (half a cardiac cycle, the rough coherence
limit), and assuming a 10 dB SNR target at 0.2 mm displacement, the required
EIRP × antenna-gain product to detect HR at various ranges in clear LOS at
77 GHz:

| Range | Required EIRP × G_r (one-way) | Achievable under FCC §95.M? |
|---|---|---|
| 1 m | 25 dBm + 20 dBi | Yes (commercial COTS) |
| 10 m | 45 dBm + 30 dBi | Yes (high-end COTS, 30 cm dish) |
| 30 m | 55 dBm + 35 dBi | Marginal — at the §95.M peak ceiling |
| 100 m | 70 dBm + 45 dBi | No — above §95.M, experimental-licence territory |
| 500 m | 90 dBm + 55 dBi | No — military / experimental only |
| 1 km | 100 dBm + 60 dBi | No — military only |
| 10+ km | beyond physical antenna realisability for civilian use | No |

**Bottom line**: 30 m is the honest ceiling for cardiac sensing inside FCC
§95.M power limits with a 30 cm dish at 77 GHz. Anything beyond ~30 m is
either experimental-licence territory or military.

### 4.4 Fold-over with the Ghost Murmur "tens of miles" claim

The press claim of HR detection at "40 miles" (65 km) corresponds to a one-way
path loss at 77 GHz of roughly 168 dB (free space) plus ~65 dB of atmospheric
absorption (humid). Closing this link to detect a 0.2 mm chest-wall
displacement would require:

- **Required EIRP**: roughly 200 dBm (10²⁰ W) in the simplest analysis. For
  context, the entire global average solar flux is ~1.4 kW/m². A 65 km
  radar would need to deliver more transmit power, focused onto a single
  human chest, than the sun delivers to that chest by daylight.
- **Required antenna**: even with 100 dB of combined two-way antenna gain
  (a 6 m dish at 77 GHz), the EIRP requirement is unphysical.
- **Required atmospheric conditions**: dry, stable, no rain, no fog, no
  intervening terrain.

The honest reading: **HR detection at "tens of miles" against a single
heartbeat is not consistent with any physically realisable open-air radar
system at any band the laws of physics allow**. The claim either refers to
*cued* detection (i.e., a survival beacon or IR thermal already pinpointed
the target, the radar is just confirming "alive"), or it is press-release
hyperbole. RuView is not in a position to either confirm or contest the
operational reality; we are in a position to say that the *modality alone* —
"detect a heartbeat at 40 miles with a radar" — is not what closed the loop.

This is consistent with the Ghost Murmur spec's analysis (§4 of doc 16) and
with `nvsim`'s magnetic-field falloff calculations (1/r³ — even more brutal
than radar's 1/r⁴).

## 5. Regulatory + ethics

### 5.1 FCC envelope summary

| Use | FCC path | Practical for open source? |
|---|---|---|
| 60 GHz unlicensed (existing tier) | Part 15.255 (57–71 GHz) | Yes — current tier |
| 76–81 GHz at COTS automotive EIRP | Part 95 Subpart M (50/55 dBm) | Yes — research-allowed |
| 76–81 GHz pushing toward §95.M ceiling | Part 95 Subpart M | Yes — single-installation |
| 76–81 GHz beyond §95.M | Part 5 experimental licence | **No** for shipping firmware |
| 90–300 GHz coherent radar | Mostly experimental-only | **No** for shipping firmware |
| 300+ GHz transmitters | Almost all unallocated for civilian active use | **No** for shipping firmware |

For an *open-source civilian project*, only the unlicensed and part-95
licensed-by-rule categories are defensible. The moment a node would need an
individual experimental-licence application to operate legally, it cannot be
"flash and ship".

### 5.2 ITAR / EAR posture

- **ECCN 6A008** controls radar systems and components under the EAR
  ([BIS Commerce Control List Cat. 6](https://www.bis.doc.gov/index.php/documents/regulations-docs/2340-ccl9-4/file)).
  The general radar control sub-paragraph 6A008.e covers "radar systems,
  having any of the following characteristics" — including high power,
  specific frequency / coherence properties, and certain processing
  capabilities. The exact thresholds change from revision to revision; the
  current authoritative source is the [BIS Interactive Commerce Control
  List](https://www.bis.gov/regulations/ear/interactive-commerce-control-list).
- **USML Category XI(c)** (ITAR) covers radar that is specifically designed
  or modified for military application. Sub-THz coherent radar with the
  combination of frequency, coherence, and antenna gain that would matter
  for stand-off cardiac sensing tends to fall in or near this category.
- **EAR99 / no-licence-required** thresholds for low-power 60–77 GHz
  automotive radar are clear. Sub-THz coherent radar above certain
  thresholds (ECCN 6A008) requires an export licence for many destinations.
  Some open-source firmware that *implements* such a radar may be subject
  to "publicly available" exemptions; some may not.
- **Open-source publication.** EAR §734.7 / §734.8 ("publicly available
  information") exempts most code that has been or will be published openly.
  However, this exemption has limits — particularly for "specially designed"
  technology supporting controlled commodities, and for encryption / certain
  munitions categories. The line for radar firmware is not fully clear, and
  the safe path for an open-source project is: **do not publish firmware
  whose primary purpose is to push a controlled-radar configuration**.

The correct posture for RuView is: **assume the worst case**. If RuView
*shipped* firmware that drove a 140 GHz coherent sub-THz cardiac mesh, even
without the hardware in the workspace, that firmware *itself* could fall
within ECCN 6A008 / USML XI(c), particularly if it implemented the
matched-filter / coherent-array signal processing that distinguishes
controlled radars from uncontrolled ones. We do not ship that firmware.

### 5.3 Open-source ethics and dual-use risk

The Ghost Murmur spec (§9) is explicit about RuView's civilian-only ethics
framing:

1. Civilian, opt-in deployments only.
2. No directional pursuit.
3. Data minimisation.
4. PII detection on the wire.
5. Adversarial-signal detection.
6. **No export-controlled hardware.**

Stand-off radar at 77 GHz with §95.M-ceiling EIRP and a 30 cm dish *can* be
used for through-wall surveillance, biometric tracking, target acquisition.
Sub-THz coherent radar can do the same with finer resolution. Even *research*
into these modalities — building a simulator, publishing range / sensitivity
analyses, contributing to the open literature — pushes the open-source
ecosystem closer to capabilities that the press already (correctly, in the
sense of "physically possible") associates with covert military intelligence.

Two specific dual-use risks if RuView research were to ship anything beyond
this ADR:

- **Through-wall surveillance**: high-power 77 GHz radar with a wide-band
  FMCW chirp can resolve human presence and coarse pose through interior
  drywall at tens of meters. This is the literal Ghost Murmur use case at
  short range. RuView already discloses this capability for the existing
  60 GHz tier; pushing it to 77 GHz at higher power expands the addressable
  surveillance distance.
- **Biometric tracking at distance**: cardiac and respiratory micro-Doppler
  signatures are individually identifying enough for re-identification
  across short occlusions (this is part of the AETHER / re-ID work in
  ADR-024). Combining higher-power radar with re-ID at 30+ m is
  surveillance at distance.
- **Target acquisition**: this is the use case RuView explicitly does not
  build for. Period.

## 6. Build / Research / Skip decision matrix

| Tier | Build now | Research only | Skip permanently | Notes |
|---|---|---|---|---|
| 77 GHz commercial COTS (already shipping at low EIRP via the 60 GHz tier; mentioned for completeness) | — | — | — | Already covered by 60 GHz tier ADR-021. No action. |
| 77 GHz higher-power experimental (≤ §95.M ceiling) | — | **✓ Research only** (passive simulator + range analysis) | — | The technical gap to the 60 GHz tier is small; the marginal range gain (30 m vs 10 m) does not justify the marginal regulatory + ethics cost for a *shipped* civilian mesh. Research / simulation only. |
| 77 GHz beyond §95.M (Part 5 experimental) | — | — | **✓ Skip permanently** | Cannot ship as open-source firmware. Individual experimental licences are not delegatable. |
| 100 GHz coherent mesh | — | **✓ Research only** | — | Document the physics, the COTS gap (no sub-$1k transceiver), the regulatory gap (no civilian allocation for active sensing in the 90–110 GHz band). Build only if all three conditions in §7.4 below trigger. |
| 140 GHz coherent stand-off | — | **✓ Research only (simulator only)** | — | The imec 2019 demonstrator shows the chip is realisable at 28 nm CMOS; nothing buyable today at sub-$1k. ECCN 6A008 risk is real. Simulator OK; firmware no. |
| 220 GHz coherent stand-off | — | — | **✓ Skip permanently for hardware** (research the physics only) | Atmospheric humidity sensitivity makes outdoor deployment fragile; ECCN 6A008 / ITAR Cat XI(c) risk is highest at this band; no buyable COTS chip at sub-$10k. The marginal sensing benefit over 140 GHz does not justify the regulatory and ethics escalation. |
| 380+ GHz imaging | — | — | **✓ Skip permanently** | Imaging-band, not radar; humidity destroys outdoor link; export-controlled at any meaningful aperture. Not RuView's modality at any plausible build. |

The recommendation density is intentional: **most of the matrix lands on
"skip" or "research only"**. Only one row (77 GHz at the §95.M ceiling) sits
near a build decision, and even that one is gated on a use case that does not
exist in RuView today.

## 7. If we research: what does RuView ship?

### 7.1 Mirror the `nvsim` pattern

ADR-089 / 090 established the precedent: when a sensing modality is
*physically interesting but not buildable today*, RuView ships a deterministic
forward simulator, not hardware. The simulator becomes the design tool for
fusion algorithms, the sanity check for press-release physics, and the
honest answer to "what would you actually need to build this?"

Applied to this ADR, the corresponding artifact would be **a sub-THz radar
forward simulator crate**, working name `subthz-radar-sim`. Scope:

- Forward-model the 77 GHz / 140 GHz / 220 GHz radar equation including
  ITU-R P.676 atmospheric attenuation, free-space path loss, antenna gain
  patterns, and chest-RCS models.
- Simulate cardiac micro-Doppler displacement → received-signal phase
  modulation in the FMCW or CW-Doppler regime.
- Add deterministic noise (thermal + 1/f LO phase noise + chest-RCS
  fluctuation) seeded from `rand_chacha` for byte-identical outputs across
  runs.
- Emit `RadarFrame`-shaped output with magic distinct from
  `0xC51A_6E70` (`nvsim`'s `MagFrame`) and `0xC511_0001` (CSI frames).
- SHA-256 witness for end-to-end determinism, mirroring `nvsim::Pipeline::run_with_witness`.

### 7.2 Hard constraints on what the crate can ship

- **No firmware.** Not for ESP32, not for any SDR, not for any FPGA. The crate
  is host-side only. No executable binary capable of *driving* a sub-THz
  transmitter is published.
- **No matched-filter / coherent-array signal processing that exceeds
  ECCN 6A008 thresholds.** The crate documents the physics and simulates the
  forward path. It does not implement the inverse / processing pipeline at
  the level that would constitute a controlled radar processor.
- **No beamforming primitives for actively-steered phased arrays.** Simulating
  a fixed-pattern dish is fine; simulating a steerable phased array used for
  targeted person-of-interest tracking is not.
- **No re-identification across the simulated radar stream.** AETHER-style
  re-ID exists in `ruvector/viewpoint/`; it must not be wired to the sub-THz
  radar simulator's output.
- **Documented dual-use posture.** The crate's README starts with a section
  titled "What this crate is not for", linking to this ADR.

### 7.3 What the simulator answers

The same questions `nvsim` answers for NV-diamond, the sub-THz simulator
would answer for radar:

- "If a 140 GHz transceiver has noise figure 12 dB and Tx power 0 dBm with a
  35 dBi antenna, what's the joint posterior P(human alive at (x, y))
  given my CSI + 60 GHz + 77 GHz + 140 GHz radar evidence at 5 m, 30 m,
  100 m?"
- "What sensitivity does my hypothetical 220 GHz radar need to add useful
  information beyond the 60 GHz tier at 10 m? And does the answer change
  in 7.5 g/m³ humidity vs. 1 g/m³ dry air?"
- "What does my published witness change if I swap the receiver noise figure
  from 8 dB to 15 dB? From 15 dB to 25 dB?"

These are pre-build sanity checks. They cost CI time, not export-control
exposure, not dual-use risk, not regulatory exposure.

### 7.4 Conditional triggers (mirror ADR-090's pattern)

Promotion of any "research only" row in §6 to "build" requires *all three*
of:

1. **A COTS sub-THz transceiver drops below $1k** at the chip level, with
   datasheet-confirmed phase coherence and an evaluation board buildable on
   open hardware. (Today: nothing.)
2. **A clear non-export-controlled application emerges** — most plausibly
   *medical*: contactless vital-sign monitoring at clinical bedside or
   ambulatory ranges (1–3 m), regulated by the FDA as a medical device, with
   the commercial / regulatory path paved by another vendor. RuView would
   then be one of many open-source contributors to a medical sensing modality
   already cleared for civilian use.
3. **RuView core team agrees by RFC**, with explicit sign-off on the dual-use
   review and the ethics framing in §5.3.

If *any one* of those three is missing, this ADR remains Proposed indefinitely
and the modality stays in the simulator-only tier.

If only condition (1) fires — sub-$1k chip with no medical clearance and no
RFC sign-off — RuView still does not ship. The simulator might be expanded;
no firmware ships.

## 8. Related work / cross-references

### 8.1 ADRs

- **ADR-021** — Vital-sign detection via 60 GHz mmWave + WiFi CSI. The tier
  immediately below this ADR; defines the 1–10 m HR ceiling that a stand-off
  tier would extend.
- **ADR-029** — RuvSense multistatic sensing mode. Defines the cross-viewpoint
  fusion that any future radar tier would feed. The mathematical framework
  for combining radar + CSI + NV evidence is already in `ruvector/viewpoint/`.
- **ADR-089** — `nvsim` NV-diamond pipeline simulator. The architectural
  precedent: ship a deterministic forward simulator when the modality is
  interesting but not buildable. Same proof / witness pattern applies here.
- **ADR-090** — `nvsim` Lindblad / Hamiltonian extension. Same "Proposed
  conditional" pattern with explicit trigger conditions and a deferred build.
  This ADR follows the same shape.
- **ADR-040** — PII detection gates. Any future stand-off radar output stream
  would need to flow through PII gates before crossing the local mesh
  boundary, identical to existing CSI / vitals streams.
- **ADR-024** — AETHER contrastive embedding. Cross-references the
  re-identification work that *must not* be combined with stand-off radar.
- **ADR-028** — ESP32 capability audit + witness verification. The
  deterministic-witness pattern applies to any new simulator crate.

### 8.2 Research docs

- `docs/research/quantum-sensing/16-ghost-murmur-ruview-spec.md` — the
  Ghost Murmur reality-check spec. §6.3 is the explicit boundary that
  triggered this ADR. §7–§9 establish the architecture, ethics, and legal
  framework that this ADR inherits.

### 8.3 Primary literature (radar at 24 / 77 / 120–140 GHz)

- **Massagram, W., Lubecke, V. M., Høst-Madsen, A., Boric-Lubecke, O.
  (2013).** "Parametric Study of Antennas for Long Range Doppler Radar
  Heart Rate Detection." *IEEE EMBC* 2013.
  ([PMC4900816](https://pmc.ncbi.nlm.nih.gov/articles/PMC4900816/))
  — HR @ 21 m, respiration @ 69 m at 24 GHz CW.
- **Mostafanezhad, I., Boric-Lubecke, O. (2014).** "Benefits of Coherent
  Low-IF for Vital Signs Monitoring." *IEEE Microw. Wireless Compon. Lett.*
  24(10), 711–713.
- **Adib, F. et al. (2015).** "Smart Homes that Monitor Breathing and Heart
  Rate." *Proc. CHI 2015*. Short-range through-wall.
- **Wang, G. et al. (2020).** "Remote Monitoring of Human Vital Signs Based
  on 77-GHz mm-Wave FMCW Radar." *Sensors* 20(10), 2999.
  ([PMC7285495](https://pmc.ncbi.nlm.nih.gov/articles/PMC7285495/))
- **Liu, J. et al. (2022).** "Real-Time Heart Rate Detection Method Based on
  77 GHz FMCW Radar." *Micromachines* 13(11), 1960.
  ([PMC9693980](https://pmc.ncbi.nlm.nih.gov/articles/PMC9693980/))
- **Chen, J. et al. (2024).** "Contactless and Short-Range Vital Signs
  Detection with Doppler Radar Millimetre-Wave (76–81 GHz) Sensing Firmware."
  *Healthcare Technology Letters* 11.
  ([Wiley HTL](https://ietresearch.onlinelibrary.wiley.com/doi/full/10.1049/htl2.12075))
- **Iyer, S. et al. (2022).** "mm-Wave Radar-Based Vital Signs Monitoring
  and Arrhythmia Detection Using Machine Learning." *Sensors*.
  ([PMC9104941](https://pmc.ncbi.nlm.nih.gov/articles/PMC9104941/))

### 8.4 Primary literature (sub-THz)

- **imec / Peeters et al. (2019).** Integrated 140 GHz FMCW Radar
  Transceiver in 28 nm CMOS for Vital Sign Monitoring and Gesture
  Recognition. *Microwave Journal* 2019-06-09; imec magazine May 2019.
  ([Microwave Journal](https://www.microwavejournal.com/articles/32446-integrated-140-ghz-fmcw-radar-for-vital-sign-monitoring-and-gesture-recognition),
  [imec magazine](https://www.imec-int.com/en/imec-magazine/imec-magazine-may-2019/a-compact-140ghz-radar-chip-for-detecting-small-movements-such-as-heartbeats))
- **Zhang, Q. et al. (2021).** "Non-Contact Monitoring of Human Vital
  Signs Using FMCW Millimeter Wave Radar in the 120 GHz Band." *Sensors*
  21. ([PMC8070581](https://pmc.ncbi.nlm.nih.gov/articles/PMC8070581/))
- **Yamagishi, H. et al. (2022).** "A new principle of pulse detection
  based on terahertz wave plethysmography." *Scientific Reports* 12,
  2022. ([Nature SREP](https://www.nature.com/articles/s41598-022-09801-w))
- ITU-R Recommendation **P.676-11** (2016). "Attenuation by atmospheric
  gases." International Telecommunication Union.
  ([P.676-11 PDF](https://www.itu.int/dms_pubrec/itu-r/rec/p/R-REC-P.676-11-201609-I!!PDF-E.pdf))
- 47 CFR Part 95 Subpart M — The 76–81 GHz Band Radar Service.
  ([eCFR](https://www.ecfr.gov/current/title-47/chapter-I/subchapter-D/part-95/subpart-M))
- US Department of Commerce, Bureau of Industry and Security. **Commerce
  Control List Category 6 — Sensors and Lasers**, ECCN 6A008.
  ([BIS CCL Cat. 6](https://www.bis.doc.gov/index.php/documents/regulations-docs/2340-ccl9-4/file))

### 8.5 Reviews

- **Li, C. et al. (2024).** "Radar-Based Heart Cardiac Activity Measurements:
  A Review." *Sensors*. ([PMC11645089](https://pmc.ncbi.nlm.nih.gov/articles/PMC11645089/))
- **Frontiers in Physiology (2022).** "Radar-based remote physiological
  sensing: Progress, challenges, and opportunities."
  ([Frontiers](https://www.frontiersin.org/journals/physiology/articles/10.3389/fphys.2022.955208/full))

## 9. Open questions

These are the questions that, if answered differently, could move a row of
the §6 decision matrix:

1. **Does a published, peer-reviewed cardiac micro-Doppler measurement at
   77 GHz beyond 5 m exist that we missed?** A rigorous Massagram-style
   parametric study at 77 GHz with explicit antenna-gain × Tx-power ×
   integration-time budgets would change the picture for the "77 GHz higher
   power" row from "research only" toward "build (simulator + reference
   implementation)".
2. **Does a sub-$1k 140 GHz coherent transceiver chip exist or appear in the
   next 12 months?** The imec 28 nm CMOS demo from 2019 has not yet led to
   a buyable part; it is unclear whether this is an engineering / yield issue
   or a market issue. If a part appears, condition (1) of §7.4 fires.
3. **Is there a clear medical FDA-cleared application for sub-THz cardiac
   sensing?** This is the single most important gating condition. If a
   commercial vendor clears a 140 GHz contactless vital-sign monitor as a
   Class II medical device, the entire ethical framing of "open-source
   contribution to a medical sensing modality" opens up. Without that
   clearance, RuView remains in the simulator-only tier.
4. **Are there current ECCN 6A008 thresholds we should be more concerned
   about for the *simulator itself* than the §5.2 analysis suggests?** The
   simulator is forward-only and emits IQ samples and a SHA-256 witness.
   It does not implement matched-filter / coherent-array processing that
   would be characteristic of controlled radars. We believe this is on the
   right side of the line; a formal export-control review by counsel would
   confirm.
5. **Should RuView contribute the sub-THz simulator to a neutral upstream**
   (e.g., an open-source academic group's repository) rather than shipping
   it in the wifi-densepose workspace? Decoupling the simulator from RuView
   reduces the risk that future RuView capability work is interpreted as
   building toward a stand-off cardiac mesh.
6. **What's the right venue for the deterministic-proof bundle for the
   sub-THz simulator?** Same question that ADR-089 left open. Probably
   the same answer: in-tree fixture + tagged release artifact.

## 10. Decision summary

This ADR is **Proposed — Research only**. The decision matrix in §6 lands on:

- **Skip permanently**: 77 GHz beyond §95.M, 220 GHz coherent stand-off
  hardware, 380+ GHz imaging.
- **Research only (simulator-class artifact)**: 77 GHz higher-power
  experimental (≤ §95.M ceiling), 100 GHz coherent mesh, 140 GHz coherent
  stand-off.
- **Build now**: nothing.

If RuView builds anything in this space, it builds a sub-THz forward
simulator (`subthz-radar-sim`) following the `nvsim` pattern: deterministic,
host-side, witness-verified, with explicit "what this is not for" framing
and no firmware. The simulator does not ship until conditions §7.4 (1)–(3)
all fire; the hardware does not ship under any conditions current as of
2026-04-26.

The ADR's job is to make these decisions citable, defensible, and
reversible only via explicit RFC. It is not a build commitment.
