# ADR 356: Rate truthful CSI and reliability gated room adaptation

Status: Accepted

Date: 2026-09-05

## Context

RuView mixed three different notions of time. ESP32 firmware requested a packet
rate, the server observed a delivered rate, and several server paths still used
fixed 2 Hz, 10 Hz, or 20 Hz assumptions. One feature named
`dominant_freq_hz` was not a temporal frequency at all. It was the index of the
strongest subcarrier multiplied by 0.05. A second defect compared a newly
inserted frame with itself, forcing that motion delta to zero.

These errors can move a spectral peak into the breathing or heartbeat band and
produce plausible but false values. They also make models sensitive to WiFi
traffic and processor scheduling rather than physical motion.

ESP-IDF documents that `wifi_csi_info_t.first_word_invalid` marks the first four
CSI bytes invalid because of a hardware limitation. It also documents that the
CSI receive callback runs in the WiFi task and should hand work to a lower
priority task rather than perform lengthy processing. RuView serialized the
invalid bytes unchanged and fed the original buffer to edge processing.

Recent work suggests four useful directions with different maturity:

1. OpenCSI represents each link as a dimensionless residual against its own
   quiet period temporal standard deviation and exposes baseline maturity. Its
   reported transfer results are limited to binary presence.
2. MORIC derives motion centered delay and Doppler views to reduce dependence
   on static environment specific CSI structure.
3. Parameter efficient CSI crowd counting combines self supervised pretraining,
   small room adapters, and a stateful counting machine.
4. WiFi JEPA masks complete radio links and predicts their latent embeddings,
   using real and ray traced CSI for pose representation learning.

The papers report promising external results. None is measured RuView evidence,
and none authorizes deployment without leakage free occupied and empty held out
evaluation.

## Decision

### Firmware ingestion

1. When ESP-IDF marks the first CSI word invalid, zero the first four serialized
   CSI bytes while retaining the fixed packet shape.
2. Set ADR-018 byte 19 bit 5 to record that sanitation occurred.
3. Feed edge DSP from the sanitized serialized payload, never the original
   invalid driver buffer.
4. Keep callback work bounded to sanitation, serialization, and the existing
   queue handoff. Do not add transforms to the WiFi task.

### Server time model

1. Estimate delivered CSI rate from accepted monotonic frame timestamps after
   five samples. Use 20 Hz only during startup.
2. Cap the physical input rate at the firmware contract of 50 Hz so a burst or
   timestamp artifact cannot retune downstream filters above their supported
   range.
3. Reconfigure the vital detector only after a 20 percent rate change. Clear
   temporal and smoothing state whenever its clock changes. Samples calculated
   with different clocks must never be blended.
4. Compute temporal dominant frequency from the recent mean amplitude series
   with demeaning, a Hann window, and bounded direct Fourier bins from 0.1 Hz to
   the smaller of 4 Hz or 45 percent of sample rate.
5. Return zero for short, flat, nonfinite, or spectrally ambiguous windows.
6. Compare the current frame with the prior frame, not itself, for motion delta.
7. Export zero sample rate until the estimate is mature. Never present a startup
   fallback as a measured node property.

### Model compatibility and room reliability

1. Preserve the historical fifth input feature for version 1 adaptive models at
   the model boundary. Version 2 and later models consume the true temporal
   frequency. This prevents an existing model from silently receiving a
   different input distribution after a server update.
2. A background model is unreliable until it contains at least ten finite,
   nonnegative residual references. An unreliable model cannot suppress a
   runtime change and exposes no conformance score.
3. Expose background maturity, reliability, and a robust residual Z score based
   on the median and median absolute deviation.
4. Keep the persisted empty home prior negative only. It cannot authorize
   calibrated presence, person count, pose, or numeric vitals.

### Model roadmap

The following work remains proposal only:

1. Evaluate an OpenCSI style per link quiet residual normalization for binary
   presence without replacing absolute magnitude features used for motion.
2. Evaluate delay and Doppler views after phase sanitation for activity.
3. Pretrain a frozen multilink encoder and adapt small room specific modules for
   counting.
4. Evaluate link masked self supervision for pose only when at least three
   spatially distinct links and synchronized positive pose labels exist.

Every candidate uses immutable manifests and splits by room, subject,
recording, and time. It is compared with the frozen baseline under identical
seeds. Counting includes confusion matrices and mean absolute error. Pose
includes PCK at 20 centimetres, MPJPE, and the mean pose baseline. A shuffled
label control must fail.

Promotion requires at least 10 percent held out lift, less than 10 milliseconds
added p95 inference latency, no more than 20 percent peak memory overhead, zero
leakage or secret findings, frozen anchor retention, and no regression in empty
rejection or occupied recall. Promotion remains an explicit maintainer action.

## Consequences

The firmware no longer preserves a known invalid prefix as sensor evidence. The
server uses an observed physical clock for spectral features and vital filters.
Existing version 1 adaptive models remain compatible. Sparse background samples
fail open to observation and fail closed for suppression.

The direct Fourier estimator is intentionally small and bounded to 128 temporal
samples. It is portable and deterministic, but it is not a replacement for a
phase sanitized delay and Doppler pipeline.

The largest remaining uncertainty is human recall. Empty home evidence can
measure false presence but cannot show whether weak occupied signals are being
suppressed. The fix path is a consented, leakage separated occupied holdout with
multiple spatial links and synchronized reference labels.

## Research basis

1. Espressif ESP-IDF WiFi CSI programming guide:
   https://docs.espressif.com/projects/esp-idf/en/v5.5.4/esp32/api-guides/wifi.html
2. Espressif ESP CSI reference implementation:
   https://github.com/espressif/esp-csi
3. OpenCSI:
   https://arxiv.org/abs/2607.26665
4. MORIC:
   https://arxiv.org/abs/2506.12997
5. Parameter efficient CSI crowd counting:
   https://arxiv.org/abs/2601.02203
6. WiFi JEPA:
   https://arxiv.org/abs/2607.11064

## Acceptance test

Run the firmware serializer host tests and the focused hardware, signal, and
sensing server Rust suites. The deterministic serializer test must prove the
first four bytes are zero only when ESP-IDF marks them invalid and bit 5 must
round trip through the Rust parser. The temporal estimator must recover a
1.2 Hz synthetic signal within 0.21 Hz, abstain on short and flat inputs, and
remain below 10 milliseconds p95 on the target Mac. Existing version 1 model
input must retain the legacy proxy. A background model with nine references
must never report a match or suppress a runtime change. Physical firmware is
not qualified until an S3 and C6 boot and stream the sanitized flag without
watchdog, memory, or transport regressions during a five minute burn in.
