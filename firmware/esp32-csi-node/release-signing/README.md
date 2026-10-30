# Firmware release manifest and signing

Every `v*-esp32` release publishes `firmware-manifest.json` next to the variant
archives and `SHA256SUMS.txt` (RuView#2394). The manifest is generated in the
`publish-release-assets` job of `.github/workflows/firmware-ci.yml` from the
exact-run build artifacts by `firmware/esp32-csi-node/tools/release_manifest.py`,
and the same job verifies it before upload.

Checksums prove integrity. Only the signature proves origin: someone who replaces
an archive can replace `SHA256SUMS.txt` too, but cannot re-sign the manifest
without the private key.

## CI signing with Cloud KMS (recommended)

The main path signs in CI without any private key or GitHub secret:

- The Ed25519 private key is **generated inside Google Cloud KMS** (algorithm
  `EC_SIGN_ED25519`) and never leaves it. Nobody, including CI, can export it.
- On a `v*-esp32` tag, the `publish-release-assets` job gets a short-lived Google
  token through **Workload Identity Federation** (GitHub OIDC, no stored
  credential) with `google-github-actions/auth` pinned by SHA, and calls
  `release_manifest.py sign --kms-key <key version>`.
- The tool fetches the key version's public key, requires `EC_SIGN_ED25519`, and
  refuses to sign unless it equals the committed
  `release-signing/public-key.ed25519.b64`. It calls `asymmetricSign` over the raw
  manifest bytes (Ed25519 signs data, not a digest) with CRC32C integrity checks in
  both directions, verifies the signature locally with the pinned key, and only then
  writes `firmware-manifest.json.sig`. The job then runs `verify` with the signature
  before anything is uploaded. The token is never printed.
- The release is published as a full release only when the channel is `stable`
  **and** CI signed it. Anything else stays a prerelease.
- If signing is configured but fails (missing WIF provider variable, missing
  committed public key, IAM denial, key mismatch, CRC error, bad signature), the job
  fails and nothing is published.

Configuration is through **repository variables** (Settings → Secrets and
variables → Actions → Variables), not secrets, because none of them is secret:

| Variable | Value |
|---|---|
| `FIRMWARE_SIGNING_KMS_KEY` | Key **version**: `projects/<project>/locations/<location>/keyRings/<ring>/cryptoKeys/<key>/cryptoKeyVersions/<n>`. Unset = no CI signing (fallback behaviour). |
| `FIRMWARE_SIGNING_WIF_PROVIDER` | `projects/<project NUMBER>/locations/global/workloadIdentityPools/<pool>/providers/<provider>` |
| `FIRMWARE_SIGNING_SERVICE_ACCOUNT` | Optional. Empty = direct Workload Identity Federation (the pool identity holds the KMS role). Set = federation through that service account. |

How the auth step is configured (`google-github-actions/auth@c200f3691d83b41bf9bbd8638997a462592937ed`, v2.1.13):

- Direct WIF (`FIRMWARE_SIGNING_SERVICE_ACCOUNT` empty): no `token_format`; the
  federated token from the `auth_token` output (maximum lifetime 10 minutes) is
  used for the KMS call. The action cannot mint an `access_token` without a
  service account.
- Through a service account: `token_format: access_token`,
  `access_token_lifetime: 300s`; the `access_token` output is used.
- Both: `create_credentials_file: false` and `export_environment_variables: false`,
  so no credentials file or ambient credential is left for later steps. The action
  masks both outputs.

Use **one** signing key: this KMS key. The fallback below signs with the same key.

### One-time setup (maintainer)

Run these with an account that administers the GCP project. Replace the
placeholders. Anything marked *verify* should be checked in the GCP console or the
current GCP documentation before relying on it.

```bash
PROJECT=<gcp-project-id>
LOCATION=<location, for example us-central1 or global>
RING=ruview-firmware
KEY=manifest-signing
POOL=github-firmware
PROVIDER=ruview-firmware-tags
REPO_ID="$(gh api repos/ruvnet/RuView --jq .id)"   # immutable numeric repository id
```

1. Create the key ring and the Ed25519 signing key:

   ```bash
   gcloud kms keyrings create "$RING" --project="$PROJECT" --location="$LOCATION"
   gcloud kms keys create "$KEY" --project="$PROJECT" --location="$LOCATION" --keyring="$RING" \
     --purpose=asymmetric-signing --default-algorithm=ec-sign-ed25519 --protection-level=hsm
   ```

   *Verify* that `EC_SIGN_ED25519` is offered at the HSM protection level in your
   location; if `hsm` is rejected, use `--protection-level=software`. The key
   version resource is
   `projects/$PROJECT/locations/$LOCATION/keyRings/$RING/cryptoKeys/$KEY/cryptoKeyVersions/1`.

2. Create the workload identity pool and a GitHub OIDC provider that admits only
   this repository's `firmware-ci.yml` running for a `v*-esp32` tag:

   ```bash
   gcloud iam workload-identity-pools create "$POOL" --project="$PROJECT" --location=global \
     --display-name="RuView firmware release signing"
   gcloud iam workload-identity-pools providers create-oidc "$PROVIDER" \
     --project="$PROJECT" --location=global --workload-identity-pool="$POOL" \
     --issuer-uri="https://token.actions.githubusercontent.com" \
     --attribute-mapping="google.subject=assertion.sub,attribute.repository=assertion.repository,attribute.repository_id=assertion.repository_id,attribute.ref=assertion.ref,attribute.ref_type=assertion.ref_type,attribute.workflow_ref=assertion.workflow_ref" \
     --attribute-condition="assertion.repository == 'ruvnet/RuView' && assertion.repository_id == '${REPO_ID}' && assertion.ref_type == 'tag' && assertion.ref.startsWith('refs/tags/v') && assertion.ref.endsWith('-esp32') && assertion.workflow_ref.startsWith('ruvnet/RuView/.github/workflows/firmware-ci.yml@refs/tags/')"
   gcloud iam workload-identity-pools providers describe "$PROVIDER" --project="$PROJECT" \
     --location=global --workload-identity-pool="$POOL" --format='value(name)'
   ```

   The last command prints the provider name (with the project **number**) for
   `FIRMWARE_SIGNING_WIF_PROVIDER`.

3. Grant `roles/cloudkms.signerVerifier` on **that one key** (not the key ring, not
   the project). It includes signing and viewing the public key. Either directly to
   the federated identity:

   ```bash
   POOL_NAME="$(gcloud iam workload-identity-pools describe "$POOL" --project="$PROJECT" \
     --location=global --format='value(name)')"
   gcloud kms keys add-iam-policy-binding "$KEY" --project="$PROJECT" --location="$LOCATION" \
     --keyring="$RING" --role=roles/cloudkms.signerVerifier \
     --member="principalSet://iam.googleapis.com/${POOL_NAME}/attribute.repository/ruvnet/RuView"
   ```

   or, through a service account the federated identity may impersonate (then set
   `FIRMWARE_SIGNING_SERVICE_ACCOUNT`):

   ```bash
   SA="ruview-firmware-signer@${PROJECT}.iam.gserviceaccount.com"
   gcloud iam service-accounts create ruview-firmware-signer --project="$PROJECT"
   gcloud kms keys add-iam-policy-binding "$KEY" --project="$PROJECT" --location="$LOCATION" \
     --keyring="$RING" --role=roles/cloudkms.signerVerifier --member="serviceAccount:${SA}"
   gcloud iam service-accounts add-iam-policy-binding "$SA" --project="$PROJECT" \
     --role=roles/iam.workloadIdentityUser \
     --member="principalSet://iam.googleapis.com/${POOL_NAME}/attribute.repository/ruvnet/RuView"
   ```

   *Verify* that Cloud KMS accepts the `principalSet://` member for direct
   federation in your organization (org policies can restrict external
   identities); if not, use the service-account form.

4. Export the public key and commit it:

   ```bash
   python3 firmware/esp32-csi-node/tools/release_manifest.py kms-public-key \
     --kms-key "projects/$PROJECT/locations/$LOCATION/keyRings/$RING/cryptoKeys/$KEY/cryptoKeyVersions/1" \
     --out firmware/esp32-csi-node/release-signing/public-key.ed25519.b64
   ```

   (it uses `gcloud auth print-access-token`), or offline from a PEM export:

   ```bash
   gcloud kms keys versions get-public-key 1 --key="$KEY" --keyring="$RING" \
     --location="$LOCATION" --project="$PROJECT" --output-file=kms-public.pem
   python3 firmware/esp32-csi-node/tools/release_manifest.py public-key-from-pem \
     --pem kms-public.pem --out firmware/esp32-csi-node/release-signing/public-key.ed25519.b64
   ```

   Both print the fingerprint (`SHA256:` plus the hex SHA-256 of the raw 32-byte
   public key). Record it here and commit the file in a reviewed PR:

   **Current key fingerprint:** `SHA256:<to be filled in when the key is created>`

5. Set the repository variables:

   ```bash
   gh variable set FIRMWARE_SIGNING_KMS_KEY --repo ruvnet/RuView \
     --body "projects/$PROJECT/locations/$LOCATION/keyRings/$RING/cryptoKeys/$KEY/cryptoKeyVersions/1"
   gh variable set FIRMWARE_SIGNING_WIF_PROVIDER --repo ruvnet/RuView --body "<provider name from step 2>"
   gh variable set FIRMWARE_SIGNING_SERVICE_ACCOUNT --repo ruvnet/RuView --body "$SA"   # service-account form only
   ```

6. Restrict who can create release tags with a **tag ruleset** (Settings → Rules →
   Rulesets → New tag ruleset): target `refs/tags/v*-esp32`, enforcement Active,
   rules *Restrict creations*, *Restrict updates* and *Restrict deletions*, and a
   bypass list containing only the release owners (for example the repository
   admin role or a release team). The same through the API (*verify* the actor id of
   the role you choose in the ruleset UI; `5` is the built-in admin role):

   ```bash
   gh api -X POST repos/ruvnet/RuView/rulesets --input - <<'JSON'
   {"name": "ESP32 firmware release tags", "target": "tag", "enforcement": "active",
    "conditions": {"ref_name": {"include": ["refs/tags/v*-esp32"], "exclude": []}},
    "rules": [{"type": "creation"}, {"type": "update"}, {"type": "deletion"}],
    "bypass_actors": [{"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always"}]}
   JSON
   ```

7. Recommended: enable Cloud KMS *Data Access* audit logs for the project so every
   `AsymmetricSign` call is recorded with the calling identity (*verify* the log
   type in the console).

### Why the tag ruleset is required

The provider's attribute condition proves *which repository, which workflow file
path and which tag name* asked for a token. It cannot prove *what that workflow
file contains*: `workflow_ref` names `firmware-ci.yml` **as committed at the tag**.
Without the ruleset, anyone with write access could push a commit with a modified
`firmware-ci.yml` (or modified tooling), tag it `v9.9.9-esp32`, and receive a token
that can sign arbitrary bytes with the project key. The ruleset makes tag creation
the release owners' decision, so a valid signature means a release owner tagged
that commit. The ruleset, the attribute condition and the single-key IAM grant
together are the security boundary; review changes to any of them like a key
handover.

### Each release (KMS configured)

1. Bump `version.txt` on `main`, then a release owner tags:
   `git tag v0.8.18-esp32 && git push origin v0.8.18-esp32`.
2. `publish-release-assets` generates and verifies the manifest, signs it with
   Cloud KMS, verifies the signature, attests provenance, and publishes the
   archives, `SHA256SUMS.txt`, `firmware-manifest.json` and
   `firmware-manifest.json.sig`. A `stable` channel release is published as a full
   release; `prerelease` stays a prerelease.
3. Check: `python3 firmware/esp32-csi-node/tools/release_manifest.py verify-release --tag v0.8.18-esp32`.

## Fallback: maintainer-run sign-release

When `FIRMWARE_SIGNING_KMS_KEY` is not set, CI publishes an **unsigned prerelease**
(exactly as before KMS) and a maintainer signs afterwards with
`release_manifest.py sign-release`. It downloads the published release, re-verifies
everything, signs, uploads `firmware-manifest.json.sig` and, for the stable channel
only, clears the prerelease flag. This mirrors ruflo's helper-manifest model
(`v3/@claude-flow/cli/scripts/sign-helpers.mjs`, verified by
`helpers-manifest-guard.yml` / `verify-helpers.mjs`). Use it too when a CI signing
run failed and the release must be signed by hand.

### With the same KMS key (recommended fallback)

A maintainer holding `roles/cloudkms.signerVerifier` on the key signs with the same
key, so clients keep a single trusted public key:

```bash
gcloud auth login   # the tool calls `gcloud auth print-access-token`
python3 firmware/esp32-csi-node/tools/release_manifest.py sign-release --tag v0.8.18-esp32 \
  --kms-key "projects/<project>/locations/<location>/keyRings/<ring>/cryptoKeys/<key>/cryptoKeyVersions/<n>"
```

`--kms-key` cannot be combined with `--stdin-key` or `--key-file`. The access token
may also come from `GOOGLE_OAUTH_ACCESS_TOKEN`; it is never printed.

### Without KMS: Secret Manager or a local key

For setups without Cloud KMS, the private key lives outside GitHub in GCP Secret
Manager (or a maintainer's local file). It is not a repository or environment
secret and no workflow can read it; CI only verifies. The public key is still
committed at `firmware/esp32-csi-node/release-signing/public-key.ed25519.b64`.
A signing key that a workflow could read is a key that a compromised workflow, a
malicious action update or a poisoned dependency in that job could use; that is
why this path keeps the key off GitHub and why the KMS path never exposes it at all.

#### One-time setup without KMS

On a trusted machine:

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
   fingerprint in the KMS section above.

   The fingerprint is `SHA256:` plus the hex SHA-256 of the raw 32-byte public key.
   `keygen`, `sign`, `sign-release` and `verify-release` all print it.

Until the public key is committed, `sign` and `sign-release` refuse to run unless
you pass `--allow-unpinned`, which prints a loud warning. Use it only to test the
first key before committing it.

Never commit a private key. This directory holds only the public key and this README.

#### Each release without KMS

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

#### Private key sources

Without `--kms-key`, `sign` and `sign-release` take the first source that is set, in this order:

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

## What each release contains

| Asset | Purpose |
|---|---|
| `esp32-csi-node-firmware-<variant>.tar.gz` | Flat bundle: images, `version.txt`, `flasher_args.json`, inner `SHA256SUMS.txt` |
| `SHA256SUMS.txt` | SHA-256 of each archive |
| `firmware-manifest.json` | Variants, images, digests, flash offsets, channel, revocations |
| `firmware-manifest.json.sig` | Base64 Ed25519 signature over the exact bytes of the manifest. Produced in CI with Cloud KMS when configured, otherwise added by the maintainer's `sign-release` |
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
the tagged commit. **CI publishes a full (non-prerelease) GitHub release only when
the channel is `stable` and CI signed the manifest with Cloud KMS**; otherwise it
publishes a prerelease.

- `prerelease`: always a prerelease, signed in CI or by `sign-release`.
- `stable`: a full release when CI signed it. Without CI signing, `sign-release`
  marks the release non-prerelease **only after** the signature upload succeeds.
  An unsigned stable-channel manifest fails `verify-release`.

To promote, open a PR that changes `RELEASE_CHANNEL` to `stable` (with the
qualification evidence in the PR), merge it, bump `version.txt`, tag
`v<semver>-esp32` and wait for the publish job (and, without CI signing, run
`sign-release`). Put
`RELEASE_CHANNEL` back to `prerelease` in a later PR for releases that should not
be stable. `make_latest` stays `false`, so firmware releases never become the
repository's "latest" release.

## Rotation

Clients embed the public key, not a release version. To rotate:

1. Generate the next key pair as above.
2. Cut a release whose manifest is still signed by the **current** key and carries
   the next public key: add
   `--next-public-key <next base64 or file>` to the `generate` step of that
   release. Clients that verified the signature with the current key may then
   trust `nextPublicKey`.
3. With KMS: create a new key version (`gcloud kms keys versions create`), export
   its public key with `kms-public-key`, replace the committed
   `public-key.ed25519.b64`, update the fingerprint above and point
   `FIRMWARE_SIGNING_KMS_KEY` at the new version in the same change window. Without
   KMS: add the next private key as a new version of the Secret Manager secret (or a
   new secret) and replace the committed public key. From then on, signing with the
   old key is refused because it no longer matches the pin.

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
