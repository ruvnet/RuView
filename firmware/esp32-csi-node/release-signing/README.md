# Firmware release manifest and signing

Every `v*-esp32` release publishes `firmware-manifest.json` next to the variant
archives and `SHA256SUMS.txt` (RuView#2394). Releases signed with the release key
also publish `firmware-manifest.json.sig`. The manifest is generated in the
`publish-release-assets` job of `.github/workflows/firmware-ci.yml` from the
exact-run build artifacts by `firmware/esp32-csi-node/tools/release_manifest.py`.
The same job verifies it before upload.

Checksums prove integrity. Only the signature proves origin: someone who replaces
an archive can replace `SHA256SUMS.txt` too, but cannot re-sign the manifest
without the private key.

## What each release contains

| Asset | Purpose |
|---|---|
| `esp32-csi-node-firmware-<variant>.tar.gz` | Flat bundle: images, `version.txt`, `flasher_args.json`, inner `SHA256SUMS.txt` |
| `SHA256SUMS.txt` | SHA-256 of each archive |
| `firmware-manifest.json` | Variants, images, digests, flash offsets, channel, revocations |
| `firmware-manifest.json.sig` | Base64 Ed25519 signature over the exact bytes of the manifest |
| GitHub artifact attestations | SLSA build provenance for the archives and the manifest (`gh attestation verify`) |

`flasher_args.json` is the flash plan ESP-IDF wrote for that build. The manifest's
offsets are read from it and are never hand-written. The ESP32-C5 bootloader sits
at `0x2000`; S3 and C6 bootloaders sit at `0x0`.

**Clients must not flash from `flasher_args.json` file names.** Its `flash_files`
entries are ESP-IDF build-tree paths (`bootloader/bootloader.bin`,
`partition_table/partition-table.bin`, `esp32-csi-node.bin`). The bundle renames
the application and partition table per variant (for example
`esp32-csi-node-c6.bin`, `partition-table-c6.bin`). The manifest's `images[].file`
is the bundled name to flash at `images[].offset`; use `flasher_args.json` only to
cross-check offsets, as `verify` does.

## Schema `ruview.firmware.release-manifest.v1`

```json
{
  "schema": "ruview.firmware.release-manifest.v1",
  "version": "0.8.18",
  "tag": "v0.8.18-esp32",
  "channel": "prerelease",
  "sourceCommit": "<40-hex commit the tag points to>",
  "workflowRun": "https://github.com/ruvnet/RuView/actions/runs/<id>",
  "minClient": {},
  "revokes": ["0.8.15"],
  "variants": [
    {
      "id": "c5-4mb",
      "chip": "esp32c5",
      "flashSizeMb": 4,
      "display": false,
      "archive": { "name": "esp32-csi-node-firmware-c5-4mb.tar.gz", "sha256": "<hex>", "bytes": 0 },
      "images": [
        { "role": "bootloader", "file": "bootloader.bin", "offset": "0x2000", "sha256": "<hex>", "bytes": 0 },
        { "role": "partition-table", "file": "partition-table-c5.bin", "offset": "0x8000", "sha256": "<hex>", "bytes": 0 },
        { "role": "ota-data", "file": "ota_data_initial.bin", "offset": "0xf000", "sha256": "<hex>", "bytes": 0 },
        { "role": "application", "file": "esp32-csi-node-c5.bin", "offset": "0x20000", "sha256": "<hex>", "bytes": 0 }
      ],
      "qualification": "experimental"
    }
  ]
}
```

- Keys are sorted, variants are sorted by `id`, and images are sorted by offset.
  The file is deterministic for the same inputs.
- `id` is the archive name without `esp32-csi-node-firmware-` and `.tar.gz`.
  `display`, the expected chip and the default `qualification` come from
  `VARIANT_PROFILES` in the tool. A test fails if the build matrix and that table
  drift apart.
- `qualification` is `ci-only` by default and `experimental` for C5. A maintainer
  can pass `--qualification <variant>=hardware-qualified` only with hardware evidence.
- `minClient` is `{}` unless `--min-client ruview-mobile=<semver>` is passed.
- `nextPublicKey` appears only during a key rotation (see below).

Clients must ignore unknown fields and reject an unknown `schema`.

## Channels and promotion

`firmware/esp32-csi-node/RELEASE_CHANNEL` holds one word, `prerelease` (the
default) or `stable`. The publish job reads it at the tagged commit:

- `prerelease`: the GitHub release is marked prerelease. If the signing key is
  absent, the manifest is published unsigned and the release body says so.
- `stable`: the GitHub release is not marked prerelease. The job **fails** unless
  the signing key secret and the committed public key are both present.

To promote, open a PR that changes `RELEASE_CHANNEL` to `stable` (with the
qualification evidence in the PR), merge it, bump `version.txt`, then tag
`v<semver>-esp32`. Put `RELEASE_CHANNEL` back to `prerelease` in a later PR for
releases that should not be stable. `make_latest` stays `false`, so firmware
releases never become the repository's "latest" release.

## Signing key (maintainer actions)

These steps are done by a maintainer once. CI never generates keys.

1. Generate the key pair on a trusted machine:

   ```bash
   python3 -m pip install cryptography
   python3 firmware/esp32-csi-node/tools/release_manifest.py keygen \
     --private-out firmware-manifest-signing.key --public-out public-key.ed25519.b64
   ```

   The private key is written with mode `0600` and is never printed. Both files
   hold base64 of a raw 32-byte Ed25519 key.
2. In GitHub, create the environment **`firmware-release`** (Settings ->
   Environments). Add required reviewers and limit deployments to tags matching
   `v*-esp32`.
3. Add the environment secret **`FIRMWARE_MANIFEST_SIGNING_KEY`** with the
   contents of `firmware-manifest-signing.key`. Store an offline backup, then
   delete the local copy.
4. Commit the public key as
   `firmware/esp32-csi-node/release-signing/public-key.ed25519.b64` and record its
   fingerprint here:

   **Current key fingerprint:** `SHA256:<to be filled in when the key is created>`

   The fingerprint is `SHA256:` plus the hex SHA-256 of the raw 32-byte public key.
   `keygen`, `sign` and `verify` all print it.

Never commit a private key. This directory holds only the public key and this README.

## Rotation

Clients embed the public key, not a release version. To rotate:

1. Generate the next key pair as above.
2. Cut a release whose manifest is still signed by the **current** key and carries
   the next public key: add
   `--next-public-key <next base64 or file>` to the `generate` step of that
   release. Clients that verified the signature with the current key may then
   trust `nextPublicKey`.
3. Replace the environment secret and the committed `public-key.ed25519.b64`, and
   update the fingerprint above. From then on, releases are signed by the new key.

If the current key is compromised, there is no safe in-band rotation. Ship a
client update that embeds the new key.

## Revoking a bad release

Add the bad version, one per line, to `firmware/esp32-csi-node/REVOKED_VERSIONS`
in a reviewed PR (`#` starts a comment). The publish job passes the list to
`generate --revokes`, so every later manifest carries it. Clients that trust the newest
signed manifest must refuse to flash revoked versions, even when they hold valid
archives for them. A manifest cannot revoke its own version. Also mark the bad
GitHub release clearly in its notes. Deleting it is a maintainer decision.

## Verifying a release

With the tool:

```bash
gh release download v0.8.18-esp32 -R ruvnet/RuView -D release
python3 firmware/esp32-csi-node/tools/release_manifest.py verify \
  --manifest release/firmware-manifest.json \
  --sig release/firmware-manifest.json.sig \
  --public-key firmware/esp32-csi-node/release-signing/public-key.ed25519.b64 \
  --assets release
gh attestation verify release/esp32-csi-node-firmware-c6-4mb.tar.gz -R ruvnet/RuView
```

`verify` exits non-zero on any mismatch. It checks:

1. the Ed25519 signature over the exact manifest bytes;
2. each archive against the release `SHA256SUMS.txt` and the manifest;
3. each archive's members (regular files only, no links, absolute paths or `..`)
   against its inner `SHA256SUMS.txt`;
4. `version.txt` against the tag;
5. the chip, flash size, image digests and offsets against the bundled
   `flasher_args.json`, including alignment and overlap.

A client that does not use the tool must do the same: verify the signature with
the embedded public key **before** parsing anything else; reject an unknown
schema or a revoked version; hash each archive; extract it without following
links; check the inner hashes; and flash each image at the manifest offset for the
chip it detects on the device.
