# Firmware release manifest and signing

Every `v*-esp32` release publishes `firmware-manifest.json` next to the variant
archives and `SHA256SUMS.txt` (RuView#2394). The manifest is generated in the
`publish-release-assets` job of `.github/workflows/firmware-ci.yml` from the
exact-run build artifacts by `firmware/esp32-csi-node/tools/release_manifest.py`,
and the same job verifies it before upload.

Checksums prove integrity. Only the signature proves origin: someone who replaces
an archive can replace `SHA256SUMS.txt` too, but cannot re-sign the manifest
without the private key.

## Signing model: the key never enters GitHub

Signing mirrors the model ruflo already uses for its helper manifest
(`v3/@claude-flow/cli/scripts/sign-helpers.mjs`, verified in CI by
`helpers-manifest-guard.yml` / `verify-helpers.mjs`):

- The **private key lives outside GitHub**, in GCP Secret Manager (or a maintainer's
  local file). It is not a repository secret, not an environment secret, and no
  workflow can read it.
- The **public key is committed** at
  `firmware/esp32-csi-node/release-signing/public-key.ed25519.b64`.
- **CI only verifies.** On a tag it builds, generates the manifest, checks every hash
  and offset, attests build provenance, and publishes an **unsigned prerelease**.
- A **maintainer signs** afterwards with `release_manifest.py sign-release`, which
  downloads the published release, re-verifies everything, signs, uploads
  `firmware-manifest.json.sig` and, for the stable channel only, clears the
  prerelease flag.

Why: a signing key that a workflow can read is a key that a compromised workflow,
a malicious action update, or a poisoned dependency in that job can use. With the
key outside GitHub, an attacker who controls CI can still publish bytes, but cannot
produce a signature that verifies against the committed public key, so clients that
check the signature refuse them.

## What each release contains

| Asset | Purpose |
|---|---|
| `esp32-csi-node-firmware-<variant>.tar.gz` | Flat bundle: images, `version.txt`, `flasher_args.json`, inner `SHA256SUMS.txt` |
| `SHA256SUMS.txt` | SHA-256 of each archive |
| `firmware-manifest.json` | Variants, images, digests, flash offsets, channel, revocations |
| `firmware-manifest.json.sig` | Base64 Ed25519 signature over the exact bytes of the manifest. Added by the maintainer's `sign-release`, never by CI |
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
default) or `stable`. The publish job writes it into the manifest's `channel` at
the tagged commit, but **CI always publishes the GitHub release as a prerelease**,
whatever the channel.

- `prerelease`: `sign-release` signs and leaves the release marked prerelease.
- `stable`: `sign-release` marks the release non-prerelease **only after** the
  signature upload succeeds. An unsigned stable-channel manifest fails
  `verify-release`.

To promote, open a PR that changes `RELEASE_CHANNEL` to `stable` (with the
qualification evidence in the PR), merge it, bump `version.txt`, tag
`v<semver>-esp32`, wait for the publish job, then run `sign-release`. Put
`RELEASE_CHANNEL` back to `prerelease` in a later PR for releases that should not
be stable. `make_latest` stays `false`, so firmware releases never become the
repository's "latest" release.

## One-time setup (maintainer)

CI never generates or holds keys. On a trusted machine:

1. Generate the key pair:

   ```bash
   python3 -m pip install cryptography
   python3 firmware/esp32-csi-node/tools/release_manifest.py keygen \
     --private-out ruview-firmware-signing.pem --public-out public-key.ed25519.b64
   ```

   The private key is written as PEM PKCS8 with mode `0600` (`--format raw` writes
   base64 of the raw 32 bytes instead) and is never printed. The public key and its
   fingerprint are printed.
2. Store the private half in GCP Secret Manager, in a project you choose. The
   suggested secret name is `ruview-firmware-signing-key`:

   ```bash
   gcloud secrets create ruview-firmware-signing-key --project=<project> \
     --replication-policy=automatic --data-file=ruview-firmware-signing.pem
   ```

   Grant `roles/secretmanager.secretAccessor` on that secret only to the people who
   sign releases. Keep an offline backup if your policy requires one, then delete
   the local file.
3. Commit the public key as
   `firmware/esp32-csi-node/release-signing/public-key.ed25519.b64` and record its
   fingerprint here:

   **Current key fingerprint:** `SHA256:<to be filled in when the key is created>`

   The fingerprint is `SHA256:` plus the hex SHA-256 of the raw 32-byte public key.
   `keygen`, `sign`, `sign-release` and `verify-release` all print it.

Until the public key is committed, `sign` and `sign-release` refuse to run unless
you pass `--allow-unpinned`, which prints a loud warning. Use it only to test the
first key before committing it.

Never commit a private key. This directory holds only the public key and this README.

## Each release (maintainer)

1. Tag: `git tag v0.8.18-esp32 && git push origin v0.8.18-esp32` (after the
   `version.txt` bump is on `main`).
2. CI's `publish-release-assets` job publishes the archives, `SHA256SUMS.txt`, the
   unsigned `firmware-manifest.json` and build provenance, as a prerelease.
3. Sign it from your machine, with `gh` authenticated for the repository and
   `gcloud` authenticated with access to the secret:

   ```bash
   RUVIEW_FIRMWARE_SIGNING_SECRET=ruview-firmware-signing-key \
   RUVIEW_FIRMWARE_SIGNING_PROJECT=<project> \
     python3 firmware/esp32-csi-node/tools/release_manifest.py sign-release --tag v0.8.18-esp32
   ```

   `sign-release` (`--repo` defaults to `ruvnet/RuView`):
   1. refuses if the release already has `firmware-manifest.json.sig` (pass
      `--replace` to re-sign);
   2. downloads the manifest, archives and `SHA256SUMS.txt` into a temporary
      directory and runs the full `verify` (archive and inner hashes, member safety,
      `version.txt`, offsets against `flasher_args.json`, tag);
   3. signs, refusing a key whose public half differs from the committed one;
   4. checks the signature with the pinned public key;
   5. uploads `firmware-manifest.json.sig`;
   6. for `channel: stable` only, and only after the upload succeeded, runs
      `gh release edit --prerelease=false`.
4. Check the result: `python3 firmware/esp32-csi-node/tools/release_manifest.py verify-release --tag v0.8.18-esp32`.

### Private key sources

`sign` and `sign-release` take the first source that is set, in this order:

| Order | Source | Notes |
|---|---|---|
| 1 | `--stdin-key` | Piped input only, for example `gcloud secrets versions access latest --secret=ruview-firmware-signing-key \| ... --stdin-key`. A terminal is refused. |
| 2 | `--key-file PATH` | Local or air-gapped file. |
| 3 | `RUVIEW_FIRMWARE_SIGNING_SECRET=<secret name>` | Fetched with `gcloud secrets versions access latest --secret=<name>`. Project from `--secret-project`, else `RUVIEW_FIRMWARE_SIGNING_PROJECT`, else gcloud's active project. |
| 4 | `RUVIEW_FIRMWARE_SIGNING_KEY=<base64 raw key>` | For scripted local use. |
| 5 | `~/.ruview/firmware-signing.key` | Default file. |

Keys may be PEM PKCS8 Ed25519 or base64 of the raw 32 bytes. The tool keeps the key
in memory only: it never prints it, writes it to disk, passes it in argv, or forwards
gcloud's error output.

## Rotation

Clients embed the public key, not a release version. To rotate:

1. Generate the next key pair as above.
2. Cut a release whose manifest is still signed by the **current** key and carries
   the next public key: add
   `--next-public-key <next base64 or file>` to the `generate` step of that
   release. Clients that verified the signature with the current key may then
   trust `nextPublicKey`.
3. Add the next private key as a new version of the Secret Manager secret (or a new
   secret), replace the committed `public-key.ed25519.b64`, and update the
   fingerprint above. From then on, `sign-release` signs with the new key and
   refuses the old one.

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

With the tool (needs `gh` and `cryptography`):

```bash
python3 firmware/esp32-csi-node/tools/release_manifest.py verify-release --tag v0.8.18-esp32
gh attestation verify <downloaded archive> -R ruvnet/RuView
```

`verify-release` downloads the release and prints either **signed and verified**
(with the key fingerprint) or **unsigned (integrity only)**. It exits non-zero on
any mismatch, on a signature that does not verify against the committed public key,
and on a release that is unsigned but stable (stable channel, or not marked
prerelease). Anyone can run the same check in CI with the read-only
`Firmware release verify` workflow (`.github/workflows/firmware-release-verify.yml`,
`workflow_dispatch` with the tag).

To verify files you already downloaded:

```bash
gh release download v0.8.18-esp32 -R ruvnet/RuView -D release
python3 firmware/esp32-csi-node/tools/release_manifest.py verify \
  --manifest release/firmware-manifest.json \
  --sig release/firmware-manifest.json.sig \
  --public-key firmware/esp32-csi-node/release-signing/public-key.ed25519.b64 \
  --assets release
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
