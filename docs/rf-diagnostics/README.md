# RuView RF diagnostics viewer

Open [index.html](index.html) in a modern browser, or use the deployed documentation page. This is a standalone local JSONL viewer, not a hardware connection. It has no runtime dependencies, telemetry, external assets, uploads or network API.

1. Run the producer described in [tools/w1700k](../../tools/w1700k/) and save its diagnostic JSONL output.
2. Select **Import local JSONL**. Files must be at most 10 MiB, with 1 to 10,000 records.
3. Inspect relative RMS, numeric rail fraction, reported missing samples, reason counts and the last 500 observations.
4. Select **Clear** to remove the loaded observations, or **Load synthetic demo** for a deterministic synthetic example.

Raw IQ and unknown fields are rejected. The viewer accepts the frozen `ruview.rf_diagnostics.v1` schema and requires `valid:false`, `config_verified:false` and the `configuration_unverified` reason. Integers must fit JavaScript's safe integer range; larger producer counters are rejected rather than silently rounded.

## Evidence boundary

| Producer source_kind | Display |
| --- | --- |
| synthetic | SYNTHETIC |
| captured_replay | CAPTURED REPLAY |
| live | LIVE UNVERIFIED |

Every imported file is replay. Source labels are claims from the input and do not prove that a device was used. Mixed input displays all applicable labels. Nothing here establishes human presence, motion, pose, CSI or calibrated RF power.

RMS is relative dBFS; numeric rails are an advisory clipping indicator. Missing sample totals sum the producer's per record values. Record order is preserved rather than interpreted as synchronized time. Host monotonic timestamps cannot establish wall clock freshness. The producer TTL is displayed but is never converted into a valid or fresh verdict for file replay.

All text is rendered with textContent. A restrictive content security policy blocks network connections. Invalid imports clear the previous dataset to avoid showing stale values as the new result.

## Verification

From the repository root:

```sh
node --test docs/rf-diagnostics/model.test.cjs
node --check docs/rf-diagnostics/app.js
```

Optional browser smoke with an existing Playwright installation and Chromium:

```sh
PLAYWRIGHT_MODULE=/absolute/path/to/playwright node docs/rf-diagnostics/browser-smoke.cjs
```

The browser test checks mobile layout, synthetic rendering, actual local file import, unverified live labels, malformed input rejection, clear and zero external requests. It does not establish physical hardware performance. No package is installed by these commands.

### Numeric contract

Sample rates are unknown (0), 40, 80, 160 or 320 Msps. Centre frequency is unknown or 2 to 8 GHz. Each record describes zero or 2048 samples; missing sample counts are multiples of 2048. Absolute sample start remains null. RMS is bounded to −200 through 16 dBFS to accommodate numeric filter headroom, and an available RMS requires an available rail fraction. Reason names are from the producer whitelist and cannot repeat.
