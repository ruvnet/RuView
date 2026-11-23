# Historical firmware snapshots

These tracked files are historical artifacts, not the current release channel.
Do not select them for a new installation: their board configurations and
versions differ, and older builds can produce insufficient CSI traffic.

Use the versioned, checksummed bundles on the
[ESP32 0.8.14 prerelease page](https://github.com/ruvnet/RuView/releases/tag/v0.8.14-esp32)
or the [previous stable 0.8.8 release](https://github.com/ruvnet/RuView/releases/tag/v0.8.8-esp32).
See the [firmware guide](../README.md#0-download-a-versioned-firmware-bundle)
for chip, flash-size and display selection. The 0.8.14 prerelease requires
physical-board qualification; CI build success is not hardware validation.

`c6-onboarding/` remains the historical 0.8.12 snapshot. Its local `version.txt`
and `SHA256SUMS.txt` describe that snapshot, independently of current source.
No tracked binary is updated as part of a source-only firmware release.
