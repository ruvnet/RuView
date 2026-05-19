# ADR-102: Edge Module Registry Integration

- **Status:** Accepted
- **Date:** 2026-05-19
- **Deciders:** ruv

## Context

The Cognitum app ecosystem publishes a canonical app store catalog at:

```
https://storage.googleapis.com/cognitum-apps/app-registry.json
```

As of v2.1.0 (2026-05-13) the registry advertises **105 cogs across 11 categories** (health, security, building, retail, industrial, research, ai, swarm, signal, network, developer). Each entry carries `id`, `name`, `category`, `version`, `description`, `size_kb`, `difficulty`, `sha256`, `binary_size`, and a `config[]` schema describing the runtime parameters the appliance offers when installing the cog.

RuView today has no live awareness of this catalog. The `README.md` capability table is hand-curated; the UI surfaces only the capabilities the dashboard's HTML knows about; nothing in `wifi-densepose-sensing-server` references the registry. Result: when Cognitum ships a new cog (the registry was last updated 6 days ago — a fast cadence), RuView stays unaware until someone manually edits the README. Customers running the RuView dashboard against a real appliance see a 10-capability bag in the UI while the appliance is actually capable of installing 105 cogs.

Today's `cog-pose-estimation@0.0.1` release (PRs #642 / #643, ADR-100, ADR-101) is the first cog this repo ships to that registry. We need the discovery side to match.

## Decision

`wifi-densepose-sensing-server` will fetch `app-registry.json` on demand, cache it in process memory with a TTL, and serve it back through a new endpoint:

```
GET /api/v1/edge/registry
GET /api/v1/edge/registry?refresh=1   (force-bypass cache, log if abused)
```

The registry is **passively surfaced**, not modified. RuView is a presentation layer for the canonical Cognitum catalog; it never re-signs entries or re-hosts binaries.

### Module

`v2/crates/wifi-densepose-sensing-server/src/edge_registry.rs` — small, ~150 lines.

```rust
pub struct EdgeRegistry {
    cached: RwLock<Option<CachedEntry>>,
    ttl: Duration,
    upstream_url: String,
}

struct CachedEntry {
    payload: serde_json::Value,
    fetched_at: Instant,
    upstream_sha256: String,
}
```

Cache semantics:

- TTL **3600 s (1 hour)** by default — registry updates land on a roughly-weekly cadence and a stale-by-an-hour catalog is fine.
- `?refresh=1` bypasses the cache but writes a debug log so accidental abuse is visible.
- On upstream fetch failure when the cache is non-empty, **serve the stale cached copy** with a `stale: true` marker in the response and a 200 status (preserve UI), not a 5xx.
- On upstream fetch failure when the cache is empty, return 503 with the upstream error in the body.

### Response shape

```jsonc
{
  "fetched_at": 1779200000,           // server-side fetch timestamp
  "ttl_seconds": 3600,
  "stale": false,                     // true when serving past TTL because upstream is down
  "upstream_url": "https://storage.googleapis.com/cognitum-apps/app-registry.json",
  "upstream_sha256": "<sha256-of-payload-bytes>",
  "registry": { /* full canonical JSON as returned upstream */ }
}
```

The `registry` field is the upstream JSON inlined verbatim so consumers don't need to make a second hop. `upstream_sha256` lets a paranoid consumer compare against a pinned hash.

### Trust / verification

- Bucket is public-read with object versioning enabled (per ADR-100 §"GCS misconfiguration risks").
- The cog-level `binary_sha256` + `binary_signature` (ADR-100) are the trust roots for *installs*. The registry itself is not signed today.
- We deliberately **do not** add a signature requirement to the registry JSON in this ADR — that would block the integration on a parallel infrastructure project. A future ADR can layer signature checks on top once the publisher pipeline emits them.

### UI surfacing

New page `ui/edge-modules.html` renders the registry into category sections with cog cards. Each card links out to the Cognitum V0 appliance's `/cogs` page (`http://cognitum-v0:9000/cogs#<id>`) for the install action — RuView itself never installs.

The existing dashboard's "Capabilities" section continues to show RuView-native sensing capabilities (presence, breathing, pose, etc. — the things RuView itself runs); the new edge-modules page shows the broader Cognitum cog catalog. The two are distinct surfaces and shouldn't be merged.

### Failure modes

| Scenario | Behaviour |
|---|---|
| Upstream returns 200 with valid JSON | Cache it, return it. |
| Upstream returns 200 with invalid JSON | Treat as failure; serve stale if available else 503. Log the upstream sha + the parse error. |
| Upstream returns 4xx / 5xx | Same as JSON-invalid: serve stale if available else 503. |
| TLS / DNS / timeout error | Same. |
| Upstream is permanently moved | Operator updates the `upstream_url` config (CLI flag added). No code change required to migrate registries. |

### Configuration

- `--edge-registry-url <URL>` — override the default (default: `https://storage.googleapis.com/cognitum-apps/app-registry.json`)
- `--edge-registry-ttl-secs <N>` — override the cache TTL (default: 3600)
- `--no-edge-registry` — disable the endpoint entirely (returns 404). For air-gapped deployments.

## Consequences

### Positive

- One source of truth for the cog catalog across RuView + Cognitum dashboards.
- Zero ongoing maintenance: when Cognitum publishes registry v2.2.0, RuView sees it within an hour without a release.
- The endpoint is also useful for non-UI consumers (CI checks, fleet automation, third-party integrations).
- Lets us deprecate the hand-curated README capability table in favour of generated content (separate PR).

### Negative

- Adds an outbound HTTP dependency to the sensing-server. Air-gapped deployments must use `--no-edge-registry`.
- Stale-but-served behaviour can mask upstream outages from operators. Mitigation: include `stale: true` + `fetched_at` in the response so the UI can render a "registry possibly out of date" badge.

### Risks

- **Upstream rug-pull**: if `cognitum-apps` is deleted or replaced, the endpoint goes dark. The `--edge-registry-url` flag lets operators repoint without a code change. Long-term, RuView could mirror the registry into its own GCS bucket if the relationship requires it.
- **Cache poisoning**: the upstream is public-read; an attacker who breaches Cognitum's GCS write could push a bad registry. The cog-level signatures (ADR-100) limit the blast radius — bad registry entries can't install bad binaries, only show wrong metadata. Acceptable until registry-level signing lands.

## Security review

A real review of the attack surface this endpoint introduces.

### Threats considered

| # | Threat | Mitigation in this ADR |
|---|--------|------------------------|
| T1 | **SSRF** — operator-supplied `--edge-registry-url` redirects fetches to an internal target | Flag is operator-only (CLI / env) — there is no API endpoint to mutate it at runtime. Operators are already trusted (they control the binary). |
| T2 | **Outbound dependency reveals deployment** — a passive observer of the egress sees the appliance phoning home to GCS | Documented in the docstring + the runtime startup log. Operators wanting offline deployments use `--no-edge-registry`. |
| T3 | **Malicious upstream registry** — Cognitum's GCS bucket is breached and a poisoned `app-registry.json` is served | Two layers absorb this: (a) the registry's role is **discovery only** — installs verify the per-cog `binary_sha256` + `binary_signature` (ADR-100); a wrong description string can mislead a human, but a wrong binary still has to pass Ed25519 against `COGNITUM_OWNER_SIGNING_KEY`. (b) The endpoint exposes `upstream_sha256` so a paranoid operator can pin the expected registry hash externally and alert on drift. |
| T4 | **Response inflation** — upstream returns a multi-GB payload to exhaust memory | `MAX_PAYLOAD_BYTES = 8 MiB` cap (current registry is ~50–200 KB). Exceeding cap returns an error without buffering past the cap. |
| T5 | **Slow upstream blocking server threads** — Slowloris-style stall on the fetch | 10-second wire timeout via `ureq::AgentBuilder`. Per-handler fetch runs inside `tokio::task::spawn_blocking` so a stalled fetch never blocks the async runtime. |
| T6 | **Denial via `?refresh=1` abuse** — unauthenticated callers force-bypass the cache repeatedly | Cache lives in process; `?refresh=1` triggers a single upstream fetch behind a synchronous code path. A flood of refresh requests is rate-limited by the upstream's own throttling (GCS) and locally serialised by Rust's `RwLock`. Refresh requests are logged at `debug` so abuse is visible. **Follow-up:** add per-IP rate-limit middleware if seen abused (separate PR; tracked in #574-style follow-up). |
| T7 | **JSON deserialisation panics** — malformed registry triggers a Rust panic | Payload is parsed as `serde_json::Value` (opaque untyped tree) — never coerced into a strongly-typed struct that could panic. Failure is propagated as `FetcherError::Network` which the handler maps to 503. |
| T8 | **Stale-on-error masks outages from operators** | Response carries `stale: true` + `fetched_at` (unix timestamp). UI rendering MUST surface this badge — encoded as an explicit field, not an implicit silence. |
| T9 | **TLS downgrade / MITM on the fetch** | `ureq` is built with the `tls` feature (rustls) by default. No `--insecure` flag exists. If the upstream uses LetsEncrypt the cert chain is system-trusted; certificate pinning is out of scope (would block the bucket from rotating certs). |
| T10 | **Unauthenticated access exposes ‘what cogs exist’** | The registry is canonical-public information (already public-read on GCS via anonymous HTTP GET). Surfacing it on a local LAN HTTP API does not increase its disclosure. The endpoint stays under the project's existing `RUVIEW_API_TOKEN` Bearer auth — when set, the registry is gated like other `/api/v1/*` routes. |
| T11 | **Configuration injection via env var** — `RUVIEW_EDGE_REGISTRY_URL` set to a malicious URL by an attacker who controls the process environment | If an attacker controls the env, they own the process; this is not a new threat surface. Documented in the CLI help. |
| T12 | **Cache mutation across threads / poisoning** | The cache is `RwLock<Option<CachedEntry>>`. Writes go through `cached.write()` once per fetch. Snapshot reads `clone()` the `CachedEntry` (cheap — `Value` is reference-counted internally for large strings) so concurrent readers don't share mutable state. Tests cover the multi-call path; no `unsafe` is used. |

### What this ADR does NOT secure

- **Registry-level signing** — the JSON payload itself is unsigned. If/when Cognitum's publisher pipeline emits a registry sig (e.g. detached `.json.sig`), a follow-up ADR will require it. Today the per-cog binary signature (ADR-100) is the actual trust root for installs; the registry is metadata.
- **Per-client rate-limiting on `?refresh=1`** — relies on the upstream's own throttling. If we see abuse we'll add a token-bucket middleware; not needed for v0.0.1.

### Testing

| Test | What it verifies |
|------|------------------|
| `first_call_hits_upstream_and_caches` | Single fetch, then cache hit |
| `ttl_expiry_triggers_refetch` | Cache TTL bound respected |
| `force_refresh_bypasses_fresh_cache` | `?refresh=1` semantics |
| `stale_serve_on_upstream_failure_after_cached_success` | T8 explicit (`stale: true` returned) |
| `no_cache_no_upstream_returns_error` | T3/T5 — error propagated cleanly when nothing to fall back on |
| `upstream_invalid_json_is_treated_as_error` | T7 — malformed payload doesn't panic |
| `upstream_sha256_is_deterministic` | T3 — hash field is reliable for external pinning |

All 7 tests in `src/edge_registry.rs::tests` pass.

## Migration

1. Land this ADR + the implementing PR.
2. UI: ship `ui/edge-modules.html` and link from `index.html`.
3. After two clean releases of the endpoint, remove the hand-curated "Capabilities" table from `README.md` and replace with a small "see the appliance for the full catalog" pointer.
4. Future ADR: registry signing once Cognitum's publisher pipeline emits a sig.

## See also

- ADR-100: Cognitum Cog Packaging Specification (binary trust model).
- ADR-101: Pose Estimation Cog (the first repo-shipped cog visible in the registry).
- v0-appliance ADR-220: Cog management surface (where this registry is the input to install actions).
- `docs/benchmarks/pose-estimation-cog.md`: the per-cog benchmark format this ADR's response shape complements.
