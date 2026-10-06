# MacWifi.app: macOS host Wi-Fi helper with Location Services

`--source wifi` on macOS reads the connected Wi-Fi link through CoreWLAN. Without Location Services, macOS (measured on macOS 27) redacts the SSID and BSSID, while RSSI, noise and channel stay real. A plain command-line helper run from a terminal can never be granted Location Services, because macOS checks the *responsible* app, which is the terminal. This folder builds the helper as a small app bundle that the server launches through LaunchServices (`open`), so the bundle can hold the grant itself. See ADR-025, Amendments 1–2.

Without this app, the server falls back to a CLI `mac_wifi` on `PATH` and uses the redacted connected link: RSSI, noise and channel, no SSID or BSSID.

## Build and authorize

Needs the Xcode Command Line Tools.

```bash
tools/mac-wifi-helper/build.sh                 # builds ~/Applications/MacWifi.app
open -W ~/Applications/MacWifi.app --args --authorize
```

The second command shows the macOS Location Services prompt for "RuView WiFi Helper". Click **Allow**. It prints `{"location_authorization":"authorized",…}` when granted. Check later with `--status`.

**After every rebuild, authorize again.** The build uses an ad-hoc signature, which changes on each build, so macOS forgets the grant. A grant that survives rebuilds needs a stable signing identity (a self-signed certificate or an Apple Developer ID).

## How the server finds it

In this order: `$RUVIEW_MAC_WIFI_APP` (set it to an empty string to disable), `~/Applications/MacWifi.app`, `/Applications/MacWifi.app`. Each scan runs `open -W -g -n --stdout <tmp> <app> --args --scan-once` and reads the JSON from a fresh temp file.

## Privacy

The helper reads only the connected link's SSID, BSSID, RSSI, noise, channel and TX rate. It never starts location updates and never stores or sends a location. The SSID appears in the server's source label (`wifi:<ssid>`).

## Tested

On macOS 27 (Apple silicon), 2026-10-01: authorized via the prompt, then a scan launched with `open` returned a real SSID and BSSID. The same binary run directly from the terminal was still redacted. The sensing server with `--source wifi` labelled the source with the real SSID and kept up with a 500 ms tick.
