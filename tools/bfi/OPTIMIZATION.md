# BFI optimization evidence

**MEASURED, September 29, 2026 local time (September 30 UTC):** the final
decoder is 3.80× faster on warm decoding of the original real reports and
2.97× faster on complete capture analysis. Two higher-traffic hardware trials
returned 66 and 96 independently verified reports. A subsequent lower-traffic
control returned one. These results validate the bounded capture tools and
specific report formats; sustained sensing and movement inference remain open.

## Changes

- Cache immutable steering matrices and their orthogonality residuals for the
  finite SU angle codebooks. Return independent mutable rows to each caller.
- Decode complete HE Ng4 allocations at 20 and 40 MHz as well as 80 MHz. The
  router/client actually varied feedback width during these trials.
- Add bounded traffic-rate and duration options, compensate for per-send
  processing overhead, and record pacing delays without catch-up bursts.
- Add a capture-quality report that verifies the PCAP hash and full observation
  window. Count the expected client-to-AP direction, retries, formats, empty
  bins, leading/trailing silence, and available beacon metadata explicitly.

The default traffic profile remains 250 packets/second. For another capture
test on this setup, use `--pps 1000` with the default 30 seconds of traffic and
45 seconds of capture. Profile bounds, route/client checks, the active-capture
marker and Wi-Fi restoration remain enforced. This is an empirical starting
profile, not an established optimal sounding rate.

## CPU measurements

Python 3.12.3 on the Linux capture host, kernel 6.17.0-20-generic. The baseline
and candidate process identical bytes. Before timing, the benchmark verifies
bit-identical results for previously supported reports, complete capture
summaries, and rejection outcomes. It alternates baseline/candidate order over
nine samples, with five iterations per warm sample. Cold samples clear the
candidate cache first; warm samples reuse it. Measurements below are medians.

| Workload | Baseline | Optimized | Speedup |
|---|---:|---:|---:|
| Real 21-report decoding, cold | 8.221 ms | 4.205 ms | 1.96× |
| Real 21-report decoding, warm | 8.200 ms | 2.156 ms | 3.80× |
| Complete 473-packet analysis, cold | 9.405 ms | 5.363 ms | 1.75× |
| Complete 473-packet analysis, warm | 9.386 ms | 3.157 ms | 2.97× |
| SYNTHETIC 256-report mix, warm | 48.107 ms | 16.755 ms | 2.87× |

The synthetic mix covers both dimensions and codebooks and includes 1,320
malformed-input comparisons. The cache is capped at 2,176 entries. Filling it
retained 1,012,448 traced bytes; peak traced allocation including the current
report was 1,081,244 bytes. These are Python traced allocations, not process RSS.
Cold and warm results describe this workload and host; neither measures RF
report production or application latency.

## Real capture results

Archer BE550 v1, associated iPhone 17, MT7927 passive capture, fixed channel 36,
80 MHz receiver width, center 5210 MHz. All trials below used the same temporary
driver and bounded ordinary UDP downlink. Router settings and firmware were
unchanged during this optimization. See [initial validation](VALIDATION.md)
and [driver scope and rollback](driver/README.md).

| Trial | Actual UDP pps | Duration | Valid reports | Reports/s | Longest interval without a report | Occupied 1 s bins |
|---|---:|---:|---:|---:|---:|---:|
| Original acceptance, requested 250 pps | 250 requested¹ | 45.033402 s | 21 | 0.4663 | 33.479 s | 6/46 |
| Higher traffic 1, requested 1000 pps | 948.998 | 45.024473 s | 66 | 1.4659 | 23.291 s | 13/46 |
| Higher traffic 2, requested 1000 pps | 949.198 | 45.033785 s | 96 | 2.1317 | 16.393 s | 21/46 |
| Fresh control, requested 250 pps | 246.533 | 45.028772 s | 1 | 0.0222 | 28.016 s | 1/46 |

¹ The original run sent 7,500 datagrams; it predates the runner's actual-rate
receipt. The newer runs sent 28,470, 28,476 and 7,396 datagrams respectively.
The payload was always 1,200 bytes, sent to client UDP port 9. Successful sends
do not prove application delivery.

Rates and silence include the full capture, including the approximately
15 seconds after traffic ends. Silence is not an estimate of packet loss.
The two higher-traffic trials also had maximum gaps of 5.361 and 6.437 seconds
between reports. Occupied bins do not imply uniform samples within each bin;
the final bin is partial. Duration-weighted occupied-bin coverage was 13.32%,
28.87%, 46.63% and 2.22% respectively.

The higher-traffic observations repeated, but this short, sequential comparison
does not isolate traffic rate from phone scheduling or changing RF conditions.
Earlier 250 pps repeats returned zero reports. No general causal speedup in RF
sampling is claimed. Median first-antenna beacon signal was −78 dBm in all
three new trials, compared with −80 dBm in the original acceptance.

### Independent report validation

All reports from the four captures above matched tshark 4.2.2 for every integer
angle, selected MIMO control fields and signed SNR: 21/21, 66/66, 96/96 and 1/1.
Every report came from the expected client to the AP, with no discarded retry
duplicates and no unsupported feedback in these captures.

| Trial | HE 20 MHz | HE 40 MHz | HE 80 MHz |
|---|---:|---:|---:|
| Original acceptance | 0 | 0 | 21 |
| Higher traffic 1 | 1 | 0 | 65 |
| Higher traffic 2 | 0 | 42 | 54 |
| Fresh control | 0 | 0 | 1 |

These real reports use SU, Nr=2, Nc=2, Ng=4, codebook 1. Full allocation tone
counts are 64, 122 and 250, with RU ranges 0–8, 0–17 and 0–36. The source
reference is the [pinned Wireshark 4.2.2 dissector](https://github.com/wireshark/wireshark/blob/40459284278611128aac5cef35a563218933f8da/epan/dissectors/packet-ieee80211.c#L15925),
SHA-256 `a60a3ead0e5aca8dadd6c08411073049593aec4d6a9ff7fa998d2d4e4445e92a`.
Other supported dimensions/codebooks are covered by synthetic tests, not these
radio observations. Angle comparison does not independently validate matrix
reconstruction or physical direction estimates. Matrix elements have a separate
closed-form synthetic oracle. Different tone grids must remain distinct in any
future motion features.

All new captures had zero reported kernel drops, complete packets, consistent
timestamps and no indicated bad FCS. FCS bytes were absent, so no independent
CRC claim is made. Each run restored the original NetworkManager connection
UUID and verified wired management. Temporary driver modules remain loaded;
installed driver files and DKMS registration are unchanged.

## Reproduce and audit

Follow the verified-address setup and commands in [README.md](README.md).
Keep all captures and private addresses outside Git. Run the decoder with the
manifest duration, the quality report with the matching manifest and client,
and the independent comparator with `--max-reports 128` for these captures.
An empty oracle result never passes. Unsupported formats remain explicit.

```bash
python3 tools/bfi/benchmark_bfi.py --baseline /private/trusted-baseline.py \
  --pcap /private/original-acceptance/capture.pcap \
  --ap <own-ap-bssid> --repeats 9 --iterations 5
python3 -B -m unittest discover -s tools/bfi -p 'test_*.py' -v
```

The benchmark executes the supplied trusted baseline Python module. Use a
capture supported by both versions for output-parity checks; newly recognized
formats intentionally change prior unsupported results.

Private capture directories retain `manifest.json`, `traffic.json` where
available, `decoded-final.json`, `oracle-final.json`, `quality-final.json` and
`final-source-receipt.json`. The final benchmark receipt is retained separately
with its trusted baseline. Raw packets, angles, credentials and identifiers
are excluded from the repository.

Capture SHA-256:

- Original: `461035ba62b8f0d1de18abb694240c5bd074eeeb522c02e866f47b95569e6344`
- Higher traffic 1: `747bc01cd6689388ac03b88aeef8cea8b85147e72e628ca7899e721a2d8cf9bb`
- Higher traffic 2: `f6308f822d59875f1c3bf8a028280fddc6649ba045be7a2ef2a572389aa23031`
- Fresh control: `b143fcd736a705b6fa8d836e43eede78932a892887e9da33b9729970f264214f`

Measured source SHA-256:

- Baseline decoder: `027c66014c27009a24c2a899cd37593e0b9f44d67dc91731ffb2b95a99d54c21`
- Final decoder: `6d0dc57e809e851d3cc3076f03cdbe685f2373a73dcd2bbd6497a953c4b2ad34`
- Quality report: `85903b684462e5e6d695d75b7daf0c9f2b1671a786b5b0e080122044073f86f4`
- Independent comparator: `0451d9d2f866a36c5246b159953659c086b03f07f4fa6ce18e580a16ed8ce9ae`
- Trial runner: `ee0279127208f1c391d4158adadf99e071f4e472e2d32b35bc3fa25096f262bc`
- Capture helper: `151c0750e5902c0b75ba05ccb87744451bd58f22f2edd44bdcf79cdcdce21a72`

## Remaining acceptance boundary

Capture, decoding, CPU improvement and observed restoration are measured.
Production BFLD integration, uniform sampling, movement classification, AoA,
2D tracking and a position-error reduction are not validated by this work.
The longest gaps and weak received signal are the next hardware constraints
to resolve before making motion-performance claims.
