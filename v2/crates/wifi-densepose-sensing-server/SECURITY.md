# Security notes — wifi-densepose-sensing-server

## HTTP and WebSocket API bind

`--bind-addr` (env `SENSING_BIND_ADDR`) defaults to `127.0.0.1`. The server
refuses to start when the bind is routable (`0.0.0.0`, `::` or a LAN IP) and
API auth is off, because `/api/v1/*` and `/ws/sensing` would then be readable
by anyone who can reach the host. Pick one:

- **Enable auth.** Set `RUVIEW_API_TOKEN=<token>` (or `RUVIEW_OAUTH_ISSUER`).
- **Stay on loopback.** Use `--bind-addr 127.0.0.1` and put a reverse proxy in
  front for remote access.
- **Opt in explicitly.** Set `RUVIEW_ALLOW_UNAUTHENTICATED=1` (`true`, `yes`
  and `on` also work). The server starts and logs a warning. Only do this on an
  isolated, trusted network.

This is the same rule the Docker entrypoint already applies (#864), so a
container that starts today keeps starting. The refusal exits with status 1;
the entrypoint's own check exits with 64 before the binary runs.

## UDP CSI data plane (ADR-296)

The sensing server ingests CSI/radar frames over UDP from ESP32, MediaTek,
Qualcomm, and RTL8720F sensor nodes. A valid-shaped frame flips an
auto-detecting server into a live source state and influences
presence/vital/automation outputs.

### Threat model

Any host that can reach the UDP port can inject a valid-shaped frame. Prior to
ADR-296 the receiver bound `0.0.0.0` unconditionally, so on a routable
deployment the data plane was open to the entire LAN.

The controls in ADR-296 (step one) are:

- **`--udp-bind` (env `RUVIEW_UDP_BIND`), default `127.0.0.1`.** The receiver is
  loopback-only by default and not reachable off-host. Binding to a routable
  address (`0.0.0.0` or a LAN IP) is now an explicit operator choice, mirroring
  the HTTP `--bind-addr` path.
- **`--udp-allow <IP/CIDR,...>` (env `RUVIEW_UDP_ALLOW`).** An optional source
  allowlist. When set, frames from non-matching sources are dropped and counted;
  loopback is always allowed.
- **`--udp-insecure-lan` (env `RUVIEW_UDP_INSECURE_LAN`).** A routable bind with
  no allowlist is *refused at boot* unless this override is passed. The name
  makes the residual risk legible.

A startup security log line states the resolved bind scope and whether an
allowlist is active.

### Residual risk — the allowlist is not authentication

An IP/CIDR allowlist restricts *which addresses* may deliver frames. It does
**not** authenticate the sender. On a trusted LAN an attacker who can spoof a
source IP, or who controls an allowlisted host, can still inject frames. Treat a
routable bind as a soft control, not a security boundary.

### Deferred to a follow-up ADR (step two)

The following are **not** implemented yet and the data plane must not be
presented as authenticated:

- per-device provisioned keys
- message authentication / AEAD (MAC over each frame)
- device identifiers
- monotonic sequence numbers
- a freshness window
- replay rejection

Real-silicon validation of the LAN path remains required before any deployment
claim.

### Safe deployment

- Prefer the loopback default. Co-locate sensor decoding on the same host, or
  place a trusted gateway in front.
- If you must bind routable, always pass `--udp-allow` scoped to the sensor
  subnet, and segregate sensors on their own VLAN.
- Do not rely on the allowlist alone against an on-LAN adversary until step two
  ships.
