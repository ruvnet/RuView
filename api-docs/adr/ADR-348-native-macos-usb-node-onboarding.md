# ADR 348: Native macOS USB node onboarding

## Status

Accepted and physically qualified with one attached ESP32 S3 and one attached
ESP32 C6 through an unsigned Mac Catalyst development build. Signed
distribution remains a release gate.

## Context

RuView operators currently provision ESP32 nodes with command line tooling.
The macOS app can list server accepted node identities, but it cannot establish
that an attached serial device is RuView firmware, assign a collision free node
identity, preserve its WiFi configuration, or prove that it joined the sensing
server. A UI that merely stores a friendly name would create false success.

Generic shell access from the app would also expand authority beyond the task.
Serial logs can contain operational data, and WiFi credentials must not enter
application diagnostics or provisioning receipts.

## Decision

1. Firmware 0.8.12 provides a bounded line protocol over the physical USB serial
   channel. A discovery request carries a random 128 bit nonce. The device
   response binds that nonce to chip family, firmware version, current node
   identity, private server target, configuration state, and a pseudonymous
   64 bit device digest derived from the base MAC under a RuView domain.

2. A configuration request is accepted only for the most recently issued
   nonce within 60 seconds. The nonce is single use. Node identity is limited
   to 1 through 255, the target is limited to RFC 1918 IPv4, and the target port
   is limited to 1 through 65535.

3. Existing WiFi configuration can be preserved. New credentials use strict
   base64 framing, bounded lengths, and control character rejection. Firmware
   commits NVS before acknowledging and rebooting. Credentials are never
   returned, logged, or included in the receipt.

4. The macOS client uses a native serial bridge. It allowlists exact character
   device prefixes, configures the port directly, and does not invoke a shell,
   Python, or an external process. It rechecks the device digest immediately
   before configuration to reduce port substitution risk.

5. The app suggests the lowest unused server node identity and the Mac private
   LAN address. Success is shown only after the configured node appears in the
   server inventory as active with evidence no older than three seconds.

6. Old firmware is reported as requiring an update. The app must not claim it
   provisioned a device based only on generic RuView serial logs.

7. Automatic firmware flashing is outside this decision. Flashing requires an
   exact chip and port check, a private backup, a matching image, and captured
   boot evidence. Release publication remains a separate maintainer action.

8. The device digest uses the PSA Crypto SHA 256 interface on ESP IDF 5.4 and
   6.0. This keeps the stable domain separated identity while avoiding the
   legacy Mbed TLS hash API removed by ESP IDF 6.

9. ESP32 C6 mmWave UART defaults use GPIO 4 and GPIO 5. Startup rejects any
   configured mmWave pin pair that overlaps the active console RX or TX pins.
   This preserves the onboarding input path after a failed mmWave probe.

## Security and privacy consequences

The app gains Mac App Sandbox USB and serial device access, but not general file
or shell authority. The protocol does not authenticate a device with a hardware
root of trust; the digest is stable identification, not a cryptographic device
certificate. Physical USB possession is therefore the trust boundary for this
version. An authenticated fleet identity remains ADR 305 scope.

The largest failure mode is a successful NVS write followed by failure to join
the LAN. The user receives an explicit bounded verification failure instead of
a success screen, while the board retains its committed settings and can be
rescanned over USB.

## Acceptance test

Connect an ESP32 with firmware 0.8.12 to a Mac that is already receiving RuView
nodes. Use only the macOS Add Sensor workflow to preserve WiFi, assign the
lowest unused identity, and set the Mac private address. Pass when the UI
reports the expected chip and firmware, the reassigned node remains fresh for
five minutes, the stale collided identity expires, app restart preserves the
profile, no server health, send, watchdog, panic, or reboot evidence appears,
and no credential appears in app logs or the provisioning receipt.

The 2026-08-31 physical run passed on an ESP32 S3. The measured witness is in
`docs/validation/2026-08-31-macos-usb-node-onboarding.md`.

The 2026-09-11 physical run passed on an ESP32 C6 over
`cu.usbserial-3120`. The Mac app flashed the image, received the nonce bound
hello receipt, preserved node identity 3, committed the server route, and
verified fresh server evidence at minus 34 dBm. The run also exposed and fixed
the GPIO 17 console RX conflict described above.
