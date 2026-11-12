# MR60 USB configuration v1

This opt-in protocol is compiled only for `CONFIG_C6_XIAO_MR60BHA2`.
It changes controller network settings, not the proprietary radar module firmware.
Capability proves controller firmware support; it does not prove attached hardware,
fresh radar reception, calibration, presence or vital-sign accuracy.

## Wire contract

All lines are bounded by the existing 384-byte serial input limit. Values are
space-delimited. Nonces have 32 hexadecimal characters; `device_id` is the existing
16-character, lowercase eFuse-MAC-derived `device_digest`, never an account ID.
Examples below are symbolic, not executable device commands.

An ordinary `RUVIEW_HELLO_V1 <nonce>` returns the existing HELLO plus:

```text
RUVIEW_RADAR_CAP_V1 nonce=<nonce> device_id=<digest> model=mr60bha2 config=1 radar_port=<saved-radar-port> raw_port=<unchanged-raw-port>
```

The capability is absent on generic builds, unreadable NVS, invalid split-port
state or failed stable-device-identity derivation. A radar port of zero denotes
legacy routing; it is readable but cannot be requested by this command.

```text
RUVIEW_RADAR_CONFIG_V1 <nonce> <device_id> <expected-saved-node-id> <private-IPv4> <radar-port> <SSID-base64-or-KEEP> <password-base64-or-KEEP>
```

Both Wi-Fi values must be KEEP, or both supplied. Existing byte/control-character
bounds apply. The radar port is 1024–65535 and must differ from the raw CSI port.
The nonce lasts at most 60 seconds and authorizes one attempted commit, bound to
the stable device ID and unchanged saved node ID. Freshness is not network
authentication; configuration is confined to a selected physical USB connection.

The only written keys are `target_ip`, `radar_port`, and (when explicitly changed)
`ssid`/`password` in `csi_cfg`. Raw `target_port`, node ID, antenna, update keys and
all other NVS entries are preserved. The numeric IP is shared by raw CSI and
radar; separate IPs, hostnames/mDNS and automatic DHCP recovery are not supported.
Use USB reconfiguration when the Mac's address changes.

After commit, the handler reads actual persisted target/radar/Wi-Fi values and
checks unchanged effective raw port/node identity. Only matching readback emits:

```text
RUVIEW_RADAR_CONFIG_OK_V1 nonce=<nonce> device_id=<digest> node_id=<saved-node-id> target_ip=<IPv4> radar_port=<port> raw_port=<unchanged-port> rebooting=1
```

NVS errors or mismatches do not emit success or reboot; partial application is
possible after an uncertain storage failure. Re-identify before retrying. HELLO
refreshes saved values rather than reporting stale pre-write RAM. Generic CSI
CONFIG is refused on dedicated MR60 builds to avoid rewriting the raw port.
No Wi-Fi values are echoed in replies. After reboot the Mac must perform a new
nonce-bound HELLO, compare identity/ports/settings, and separately observe a new
radar packet for the configured node. A CONFIG ACK alone is not sensor-ready.

## Verification

`make run_serial_radar run_serial_onboarding` executes the production handlers
with synthetic NVS/clock/identity ports, including generic-build denial. Full
`make host_tests` also exercises existing firmware controls. These are host
checks, not physical flashing, boot or network-change qualification.

The paired app consumer is tracked in cognitum-one/ruview-mobile#278. It must
not fall back to generic provisioning when this capability is missing.

The dedicated `sdkconfig.defaults.xiao-mr60bha2` profile sets the factory raw
port to55063; generic target defaults are unchanged. Build a clean C6 image
with the base, C6,4MB and MR60 profile defaults, then inspect its generated
sdkconfig and flash layout before packaging. Existing NVS values override
these defaults and are not rewritten by this source change. No qualified
factory image or physical onboarding is claimed by host tests alone.
