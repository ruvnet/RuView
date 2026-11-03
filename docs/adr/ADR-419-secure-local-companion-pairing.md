# ADR-419: Secure local Mac–iPhone companion pairing

- Status: Accepted decision (2026-10-12); implemented; software verified, physical qualification pending
- Decision owner: product owner, approved in the implementation session
- Related: ADR-271 (OAuth), ADR-272 (WebSocket tickets); ruview-mobile ADR-046
- Updated: 2026-10-20 — bounded transient sensor preview; physical qualification pending.

## Context

The managed Mac helper uses an ephemeral, full-authority owner token. Earlier
phone testing connected to an unauthenticated LAN helper. Exporting the owner
token or disabling authentication would not establish a safe public companion
workflow. An IP address and Bonjour identity are routing hints, not trust anchors.

## Decision

Use a separate opt-in TLS companion listener (managed Mac port 3443), with a
persistent installation certificate. The owner app remains on authenticated
loopback HTTP. Its token never leaves that owner channel. Companion management
routes require both the actual loopback socket peer and the exact owner token;
OAuth administrator authority and forwarded headers do not substitute for either.

The Mac displays a five-minute, one-use QR invitation containing version,
installation ID, HTTPS local endpoint, certificate DER SHA-256 fingerprint,
invitation ID, and a 256-bit secret. The phone pins the certificate before sending
any secret. It generates an independent 256-bit device secret, submits enrollment,
and displays a comparison code. The Mac operator approves the matching request.
Observing the invitation alone cannot retrieve an approved phone's credential.

The phone stores its device secret and certificate pin in device-only Keychain.
The server persists only device-secret hashes. Pairing lasts at most 30 days;
expiry requires explicit pairing again. A paired phone obtains a 12-hour access
token, capped by pairing expiry; access tokens and calibration grants are not
persisted on the server. Restart refresh uses the durable device proof over pinned
TLS. Permission to calibrate is a separate owner-granted 15-minute capability.
Only one phone holds calibration authority at a time; granting another phone
explicitly transfers control and removes the previous grant, while read access
remains.
Read authority never authorizes model, recording, configuration or administrator
mutations. Existing OAuth scope semantics are unchanged.

Companion WebSockets use native authenticated TLS upgrades, not legacy browser
WS tickets. The initial supported stream is `/ws/sensing`. Unsupported streams
fail closed. Revocation and access expiry close established streams and prevent
new requests. Corrupt private identity/grant state or failed grant persistence
fails closed. No authentication disablement, certificate trust fallback, redirect
following or plaintext fallback is allowed.

Bonjour may rediscover a route, but the persisted certificate pin and installation
must still match. DHCP changes do not change pairing identity. There is no cloud
relay or automatic certificate rotation in this decision; lost identity requires
explicit re-pairing. This is distinct from Finder/Xcode device pairing and Cognitum
account login.

## Shared calibration authority

The Mac explicitly publishes one bounded current room geometry and RF placement
revision. Publication uses compare-and-swap; phone import is explicit. It does not
transfer raw CSI, photos, recordings, teacher samples, models, historical checks
or full-evidence receipts. Both clients derive the same installation/room/revision
binding from canonical integer millimetres and heading tenths, independently of
loopback, LAN origin and transport. Geometry changes require republishing.
Companion calibration must match the published digest and complete node set.
Existing server boot/session, accepted-grid and evidence checks remain mandatory.
A phone cannot label an old receipt current or shorten the required baseline.

## Deployment, rollback and verification

Land producer and consumer PRs separately and bundle the identified helper only
after both protocols are verified. An older helper must leave pairing unavailable
without weakening authentication. Roll back the app/helper together while retaining
private pairing files; no rollback silently reenables unauthenticated LAN access.

Required verification: exact compiled helper over real sockets; owner peer/token
isolation; QR expiry/replay/duplicate enrollment; unknown and oversized input;
wrong certificate/installation and redirect rejection; read-versus-calibration
scope; revocation of live streams; durable restart and corrupt-state behavior;
shared-room digest parity, revision conflicts and no stale-evidence import; native
iOS and Mac builds; actual phone–Mac pairing and reconnect. Synthetic tests and
authenticated transport are not physical sensing or room-selectivity acceptance.

## Implementation evidence (2026-10-12)

Producer PR #2249 and consumer `cognitum-one/ruview-mobile#57` implement this
contract. The producer package passed 1,143 tests (three existing ignores); the
compiled debug and release helpers each passed the 26-check real-socket probe.
The consumer passed 1,547 Jest tests, iOS/Mac builds and production Swift TLS,
path, recovery and disconnect-race checks. Adversarial review has no remaining
evidence-backed blocker for PR review. These results do not establish physical
QR scanning, room controls or distribution acceptance.

## Sensor preview amendment (2026-10-20)

Sensor sharing is a separate transient owner-granted capability, lasting fifteen
minutes and never implied by read or calibration authority. Owner grant/revoke
routes are `/api/v1/companion/devices/:id/sensors`; snapshot readback is
`/api/v1/companion/sensors`. Existing actual-loopback plus exact-owner-token
protection applies to all three. No companion may call owner routes.

The dedicated pinned TLS `/companion/v1/sensors` WebSocket authenticates the
existing pairing/access proof plus current sharing permission and immediately
sends `sensor_ready`. Each exact-schema geometry frame is bounded to 64 KiB,
1–512 integer-millimetre points, a rigid finite camera transform, explicit capture
lineage, monotonic sequence and bounded source age. Unknown fields, images,
empty/malformed geometry, replay, lineage substitution and excess rate fail
closed. Device identity comes from the authenticated grant, never packet data.

Only the latest frame exists in transient memory. Pairing revocation, access or
sharing expiry, owner revocation, supersession and socket closure suppress it;
owner readback marks frames stale after three seconds. A superseded socket cannot
clear its replacement's preview. The receiver additionally checks both source
and receive age before drawing geometry. No storage, camera feed, legacy relay
admin token, calibration promotion or ARKit-to-room alignment is added.

The consumer independently opts in to foreground sharing and renders remote
geometry separately from local sensing/calibration authority. Software protocol,
real socket and owner/companion middleware checks do not establish physical
phone-to-Mac capture accuracy or calibration acceptance.

## Retained replica boundaries (2026-10-22)

The helper's durable `companion-room.json` is one canonical geometry envelope,
not a replica of either app's scan, models or local evidence history. The app
containers own those records separately; Cognitum sign-in and local pairing do
not synchronize them. Expiry or revocation withdraws transport access and does
not establish deletion of already retained client data.

Every accepted room PUT increments the server revision. A client may avoid an
unnecessary new binding by validating and reusing the exact unchanged published
configuration instead of issuing another PUT. Importing that identical envelope
must preserve legitimate local evidence; a deliberate changed binding withdraws
current authority while local records remain historical. The client retention
repair is proposed in ruview-mobile ADR-047; it is not present in TestFlight
build 2026102213 and requires a subsequent delivered-build restart check.

The sensor preview remains transient and currently carries no room-binding
digest. A displayed phone mesh is not a saved canonical room, durable draft,
live AR alignment or RF calibration receipt. MAT HTTP composition likewise does
not expand native companion WebSocket paths; see ADR-001's privacy amendment.
