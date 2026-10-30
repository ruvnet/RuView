# Publishing an ESP32 firmware release

This guide describes how an ESP32 firmware release is produced in this
repository, and what a stable release is allowed to claim.

## Contents

- [What are the two release channels?](#what-are-the-two-release-channels)
- [What does a stable release require?](#what-does-a-stable-release-require)
- [How do I run the hardware qualification?](#how-do-i-run-the-hardware-qualification)
- [How do I check that a version is ready to publish?](#how-do-i-check-that-a-version-is-ready-to-publish)
- [How do I publish a stable release?](#how-do-i-publish-a-stable-release)
- [How do I publish a prerelease?](#how-do-i-publish-a-prerelease)
- [What is left to the maintainer?](#what-is-left-to-the-maintainer)
- [See also](#see-also)
- [References](#references)

## What are the two release channels?

The ESP32 firmware is published on two channels, and they make different
claims.

A **prerelease** is published by CI. The `publish-release-assets` job in
`.github/workflows/firmware-ci.yml` runs on a `v*-esp32` tag, builds the
three variants, and publishes them with `prerelease: true`. Its own release
notes say so: "CI builds and host tests verify software behavior, not target
hardware. This prerelease has no new physical-board qualification or
sensing-accuracy claim." A prerelease is therefore a software artifact. It
has not been run on a board for five minutes and it makes no claim about
sensing.

A **stable** release is published by a person, after the firmware has passed
a hardware qualification on real boards. `v0.8.8-esp32` is the model: it
carries a measured hardware validation table, per-board download files, an
install procedure, and an explicit "what this release does not prove"
section.

Both channels carry the `vX.Y.Z-esp32` tag form. The channel is a property
of the publication, not of the tag.

The consequence for users is the reason the stable channel matters: the
download instructions in `firmware/esp32-csi-node/README.md` point at one
release, and that release should be one somebody qualified on hardware.

## What does a stable release require?

Four things, all of them checkable:

1. **The version in the tree is the version being released.**
   `firmware/esp32-csi-node/version.txt` holds it, and the release
   workflow's `version-guard` job refuses a tag that does not match. A tag
   `v0.8.16-esp32` requires `version.txt` to contain `0.8.16`.

2. **A release note exists.** It lives at `docs/releases/vX.Y.Z-esp32.md` and
   follows the shape of `docs/releases/v0.8.8-esp32.md`: what changed, the
   measured hardware validation, which download file belongs to which
   board, how to install it, and what the release does not prove. The
   publication body is the note.

3. **A hardware qualification record exists for that version and it
   passes.** It lives in `docs/validation/` and carries the verdict the
   qualification harness prints. Without measured hardware evidence there
   is nothing that distinguishes a stable release from a prerelease.

4. **The release workflow accepts the tag.** The trigger is the `v*-esp32`
   pattern; a version that does not match it publishes nothing.

The first and fourth are mechanical, the second and third are the substance.

## How do I run the hardware qualification?

The acceptance procedure is the one written down in
`docs/validation/2026-08-31-esp32-s3-rate-aware-transport.md`:

> Repeat this five minute procedure after any S3 timing, WiFi, transport, or
> task scheduling change. Pass only when raw callback yield remains at least
> 20 pps, node frame coverage remains at least 99 percent, and the device
> and server have zero send failures, offline frames, parser errors,
> WebSocket errors, watchdogs, panics, and unexpected resets.

`scripts/qualify-esp32-firmware.py` runs it and prints a verdict. It reads
two sides:

- the **server side** from the sensing server's `/ws/sensing` WebSocket: the
  frame count, the frames that carry the qualified node, the frames that do
  not, the parse errors, and the gap between frames;
- the **device side** from a capture of the node's console: the 1 Hz
  controller tick that reports the raw CSI yield, the mesh send-failure
  counter, the ENOMEM back-offs, the uplink watchdogs, and the restarts.

The device side needs a console capture, because the node's console is
already being read with whatever serial tool the operator has. The
authoritative copy of the device counters lives in the node's 60 s flash log,
which needs a flash dump to read; the console carries the same counters in
real time.

```bash
# 1. capture the node console for the length of the burn
#    (any serial tool will do; the harness only reads the text)

# 2. run the qualification
python scripts/qualify-esp32-firmware.py \
    --node 1 --seconds 300 \
    --console-log capture.log \
    --environment-note "ESP32-S3 DevKitC-1 N16R8, node 1 at 192.168.0.107, \
firmware 0.8.15, access point 192.168.0.1 on channel 6" \
    --out qualification.json --print-md
```

Two things about the harness are worth knowing before the run.

**The environment is mandatory.** `--environment-note` is required, because
the thresholds were measured on one board, one access point, and one room.
A verdict without the environment it was measured in is not evidence. The
thresholds are configurable for the same reason: `--min-yield-pps` and
`--min-coverage-percent` carry the values that apply to the environment
being measured, and the record states which ones were used.

**The yield depends on the network, not only on the firmware.** On a quiet
access point the node sees few CSI-eligible frames, because the access
point answers ICMP at 802.11b rates and those frames carry no CSI. The
bounded LAN traffic source in `scripts/csi-udp-poke.py` is the documented
way to give the node something to observe:

```bash
python scripts/csi-udp-poke.py 192.168.0.107 --hz 50 --seconds 300
```

The traffic source is part of the environment, so name it in the note.

## How do I check that a version is ready to publish?

`scripts/check-firmware-release-readiness.py` answers the question on the
evidence in the tree:

```bash
python scripts/check-firmware-release-readiness.py --version 0.8.16
```

It reports the four preconditions above, and it fails when any of them is
missing, when the qualification record reports a failure, or when the record
carries no verdict at all. A changelog heading is reported as an advisory,
not as a blocker.

The gate is a check, not a decision. It cannot tell you which version should
be stable, and it does not publish anything.

## How do I publish a stable release?

1. Qualify the version on the boards that will ship it, and commit the
   record with the harness's report in it:

   ```bash
   python scripts/qualify-esp32-firmware.py --node 1 --seconds 300 \
       --console-log capture.log --environment-note "..." \
       --out qualification.json --print-md
   ```

   Paste the Markdown report into the record at
   `docs/validation/<date>-<board>-<version>-qualification.md`. The gate
   looks for the `Result: **PASS**` line, so the report has to be there
   verbatim.

2. Write the release note at `docs/releases/vX.Y.Z-esp32.md`, following
   `docs/releases/v0.8.8-esp32.md`. It has to carry the measured hardware
   table, the per-board download files, the install procedure, and the
   limits of the claim.

3. Bump `firmware/esp32-csi-node/version.txt` to the version, commit, and
   let the tree go green.

4. Check the gate:

   ```bash
   python scripts/check-firmware-release-readiness.py --version 0.8.16
   ```

5. Tag the release and publish it. The tag form is `vX.Y.Z-esp32`; the
   workflow's version guard refuses anything that does not match
   `version.txt`. The publication is a release with the note as its body and
   the per-board files as its assets, and it is published as a stable
   release, not as a prerelease.

The CI workflow publishes prereleases. A stable release is published by a
person, with the assets the stable channel has always used:
`esp32-csi-node-vX.Y.Z-<board>-<size>-flash-bundle.zip`, the matching
application-only `.bin` files, and a `SHA256SUMS.txt` that covers all of
them. The CI names (`esp32-csi-node-firmware-<variant>.tar.gz`) belong to
the prerelease channel.

## How do I publish a prerelease?

Bump `firmware/esp32-csi-node/version.txt`, commit, and tag `vX.Y.Z-esp32`.
The workflow builds the three variants, publishes the
`esp32-csi-node-firmware-<variant>.tar.gz` archives with a
`SHA256SUMS.txt`, and marks the release as a prerelease. Nothing else is
needed, and no hardware is involved.

## What is left to the maintainer?

Two decisions that a pull request cannot make:

- **Which version becomes the stable one.** The gate reports whether a
  version is ready; it does not choose. The choice depends on which boards
  were qualified and what changed since the last stable release.
- **The release cadence.** How often a stable release is cut, and what a
  "patch" means for the firmware, is a project decision. This guide
  deliberately does not state one: the procedure above is fixed, the
  schedule is not.

A third decision is a matter of policy rather than mechanics: whether a
build that CI has only host-tested may ever be published as a stable
release. The current repository position is that it may not, and that is
what `prerelease: true` in the release workflow encodes.

## See also

- `firmware/esp32-csi-node/README.md` -- the download and flashing
  instructions a user follows.
- `firmware/esp32-csi-node/RUNBOOK.md` -- node operations.
- `docs/validation/2026-08-31-esp32-s3-rate-aware-transport.md` -- the
  qualification record that defines the acceptance procedure.
- `docs/releases/v0.8.8-esp32.md` -- the model release note.
- `docs/TROUBLESHOOTING.md` -- what to do when a node does not stream.

## References

- `.github/workflows/firmware-ci.yml` -- the release workflow, its tag
  trigger, its version guard, and the `prerelease: true` that keeps the CI
  channel on the prerelease side.
- `docs/validation/2026-10-20-esp32-csi-input-release-candidate.md` -- the
  record that states the prerelease policy in full.
- `firmware/esp32-csi-node/main/adaptive_controller.c` -- the 1 Hz
  controller tick the harness parses.
- `firmware/esp32-csi-node/main/csi_collector.c` -- the send-failure and
  ENOMEM counters the harness parses.
