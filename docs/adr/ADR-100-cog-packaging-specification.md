# ADR-100: Cognitum Cog Packaging Specification

- **Status:** Accepted (formalises existing convention)
- **Date:** 2026-05-19
- **Deciders:** ruv

## Context

The Cognitum V0 Appliance (`/var/lib/cognitum/apps/`) deploys discrete units called **Cogs**. They appear in the Appliance dashboard (`http://cognitum-v0:9000/cogs`) under an app-store UI (Today / Apps / Categories / Search / Updates). Until this ADR, the packaging convention has been **implicit** — derived from inspecting installed cogs (`anomaly-detect`, `presence`, `seizure-detect`, etc.) on a live appliance. Bringing new Cogs to the platform required reverse-engineering the layout each time.

This ADR formalises the layout so:

1. A repo crate can be built into a Cog with a deterministic Makefile / CI pipeline.
2. Cog binaries can be cross-compiled for every supported architecture from a single source.
3. The appliance's installer (`cognitum-cog-gateway`) can verify manifests without bespoke per-cog adapters.
4. Future Cogs in this repo (starting with `cog-pose-estimation` — see ADR-101) follow a single rule.

## Decision

### On-device layout

Each installed Cog lives at:

```
/var/lib/cognitum/apps/<cog-id>/
├── cog-<cog-id>-<arch>          # single self-contained executable
├── manifest.json                # immutable; signed by the publisher
├── config.json                  # mutable; runtime config, owned by the appliance
├── pid                          # current PID when running; absent when stopped
├── output.log                   # stdout (truncated on rotation)
└── error.log                    # stderr (truncated on rotation)
```

`<cog-id>` is kebab-case, ASCII, `[a-z0-9-]{2,32}`. `<arch>` is one of:

| arch | target triple | hardware |
|------|---------------|----------|
| `arm` | `aarch64-unknown-linux-gnu` | Raspberry Pi 5 (cognitum-v0, cluster Pis) |
| `x86_64` | `x86_64-unknown-linux-gnu` | ruvultra, generic Linux dev |
| `hailo8` | `aarch64-unknown-linux-gnu` + Hailo HEF sidecar | Pi + Hailo-8 hat (26 TOPS) |
| `hailo10` | `aarch64-unknown-linux-gnu` + Hailo HEF sidecar | Pi + Hailo-10 hat (40 TOPS) |

### `manifest.json` schema

```json
{
  "id": "anomaly-detect",
  "version": "0.1.0",
  "binary_url": "https://storage.googleapis.com/cognitum-apps/cogs/arm/cog-anomaly-detect-arm",
  "binary_bytes": 461904,
  "binary_sha256": "<hex>",
  "binary_signature": "<base64 Ed25519 sig over binary_sha256, signed with COGNITUM_OWNER_SIGNING_KEY>",
  "installed_at": 1778772536,
  "status": "installed"
}
```

Fields:

- `id`, `version`, `binary_url`, `binary_bytes`, `installed_at`, `status` — already implemented and observed in production manifests (e.g. `anomaly-detect@0.0.0`). Documented here without change.
- `binary_sha256`, `binary_signature` — **new**, REQUIRED for any Cog shipped from this repo. Backwards-compatible with existing manifests: the appliance gateway treats both fields as optional today, MUST verify them when present. ADR-103 (witness chain) covers the trust model in more detail.
- `status` values: `"installed"`, `"running"`, `"stopped"`, `"failed"`, `"updating"`.

### Binary hosting

Cog binaries live in **Google Cloud Storage**, public-read, at:

```
gs://cognitum-apps/cogs/<arch>/cog-<id>-<arch>
```

The HTTPS form is `https://storage.googleapis.com/cognitum-apps/cogs/<arch>/cog-<id>-<arch>` (no trailing extension; the URL is the canonical artifact). For Hailo variants, the HEF model file is sibling: `cog-<id>-<arch>.hef`.

Bucket conventions:

- Bucket is public-read; write requires `roles/storage.objectAdmin` in project `cognitum-20260110`.
- Per-version artifacts must be content-addressed: `cogs/<arch>/cog-<id>-<arch>@<sha256-prefix>` is the immutable copy; the un-suffixed name is a symlink that updates on release.
- `COGNITUM_OWNER_SIGNING_KEY` (GCP Secret Manager) signs every binary before upload.

### Source-tree layout (this repo)

Each Cog lives under `v2/crates/cog-<id>/`:

```
v2/crates/cog-<id>/
├── Cargo.toml                # crate name = cog-<id>; binary = cog-<id>
├── src/
│   ├── main.rs               # CLI: cog-<id> run | status | version
│   ├── lib.rs
│   └── inference.rs          # the actual work
├── cog/
│   ├── manifest.template.json
│   ├── config.schema.json    # JSON schema for runtime config
│   ├── README.md             # consumer-facing description (used by the App Store UI)
│   ├── icon.svg              # 1024×1024 icon (used by App Store hero)
│   └── Makefile              # build / sign / upload targets
└── tests/
    ├── smoke.rs
    └── manifest_signature.rs
```

### Build pipeline

```
cd v2/crates/cog-<id>
make build-arm           # cross-compile to aarch64-unknown-linux-gnu
make build-x86_64        # x86_64 Linux build
make build-hailo8        # arm + HEF compilation (requires Hailo Dataflow Compiler)
make build-hailo10       # arm + HEF compilation
make sign                # produce binary_sha256 + binary_signature
make upload              # gsutil cp to gs://cognitum-apps/cogs/<arch>/
make manifest            # emit manifest.json with all fields filled
```

CI (GitHub Actions) MUST run `make build-arm` + `make build-x86_64` on every PR touching `v2/crates/cog-*/`. Hailo HEF compilation requires the proprietary Hailo SDK and runs only on the Hailo-capable runners (currently a labelled self-hosted runner on the Pi cluster — TBD, separate ADR).

### Runtime contract

A Cog binary MUST implement:

| Subcommand | Behaviour |
|-----------|-----------|
| `cog-<id> version` | Print `<id> <version>` and exit 0. |
| `cog-<id> manifest` | Print the embedded manifest JSON and exit 0. |
| `cog-<id> run --config /path/to/config.json` | Long-running. Writes structured JSON logs to stdout (parsed by `cognitum-cog-gateway`). Exit code 0 on graceful shutdown, non-zero on fatal error. |
| `cog-<id> health` | One-shot. Exit 0 if the cog could come up healthy; non-zero with diagnostic on stderr. Called by the gateway before `run`. |

stdout JSON line format (one event per line):

```json
{"ts": 1779210883.444, "level": "info", "event": "<event-name>", "fields": { ... }}
```

## Consequences

### Positive

- New Cogs can be added without RE-ing the layout each time.
- CI can verify the manifest schema before merge.
- Signed binaries close a real supply-chain gap — current installed cogs (`anomaly-detect@0.0.0`) have no signature, and a compromised GCS object could push malicious code to every appliance.
- The runtime contract (`run | health | version | manifest`) is uniform across cogs, so `cognitum-cog-gateway` can stop carrying per-cog adapters.

### Negative

- Existing installed cogs must be re-published with signatures within one minor release of the gateway adopting the verify-when-present rule.
- Hailo HEF cross-compile is gated on a self-hosted runner; we accept that PRs touching Hailo variants will be slower to land.

### Risks

- **Signing key rotation**: `COGNITUM_OWNER_SIGNING_KEY` (Ed25519) is a single root-of-trust today. ADR-103 (witness chain) describes the rotation/recovery path; this ADR depends on that.
- **GCS bucket misconfiguration**: a public-read bucket with versioning-off could allow rollback attacks. Bucket MUST have Object Versioning enabled + 90-day non-current-version retention.

## Migration

1. Land this ADR.
2. Land ADR-101 (`cog-pose-estimation` — first Cog built to this spec).
3. After two clean releases of `cog-pose-estimation`, re-publish the existing cogs (`anomaly-detect`, `presence`, etc.) with `binary_sha256` + `binary_signature`. Track in a follow-up issue.
4. Flip `cognitum-cog-gateway` from "verify when present" to "require signature" — separate ADR, separate review.

## See also

- ADR-101: Pose Estimation Cog (first Cog built to this spec).
- ADR-103: Witness chain trust model (signing key rotation, future ADR).
- `docs/adr/ADR-079-camera-ground-truth-training.md` — the training pipeline behind `cog-pose-estimation`.
- `CLAUDE.local.md` § "Fleet Infrastructure (Tailscale)" — appliance layout this ADR describes.
