# Bounded CSI learning data

These offline tools prepare numeric inputs and train a small baseline. Outputs,
source captures, model weights, and logs belong outside the repository.

## Public NTU-Fi HumanID

The SenseFi author dataset supplies identity labels. This importer uses a custom train/test
split. Download and extract it separately using a reviewed, bounded downloader.
The importer requires `train_amp/<identity>/*.mat` and
`test_amp/<identity>/*.mat`: 14 identities, 294 train recordings and 546 test
recordings in the downloaded author archive. The author code instead trains on test_amp and tests on train_amp. This importer preserves its existing archive-directory partition and does not reproduce that benchmark. It accepts numeric `CSIamp` matrices of shape 342 × 2000 and applies
the author's temporal decimation `x[:, ::4]`. Each result has 500 samples and
342 amplitude features. No MATLAB code or pickle is executed.

```bash
python tools/bfi_learning/prepare.py \
  --ntu-humanid /private/data/NTU-Fi-HumanID \
  --output /private/data/prepared-humanid
```

NumPy and SciPy are required. Output is `dataset-000.npz` plus its JSON manifest.
The NPZ contains only float32 `frames[N,F]`, float64 `timestamps[N]`, and uint32
`sequence[N]`. Use `allow_pickle=False` when reading it. Source and output SHA256
hashes bind the manifest to the files. Source files are read only.

Validation takes the first floor(20%) of train_amp files per identity,
ordered by their SHA256 hashes. All test_amp files stay in the custom test split. Duplicate
source hashes are rejected across all splits. The importer does not normalize
using validation or test data; the trainer fits normalization on train only.

Metadata records `split_protocol=archive_directory_custom`, both author folder mappings, and each recording's source folder. `source_split` names this declared custom partition; it does not assert the author benchmark split. Existing pilot artifacts retain their original provenance.

The metadata's `sessions` entries are **source recording files**. Physical
acquisition sessions are unknown, explicitly recorded as such. File-disjoint
results do not prove session-disjoint generalization. Timestamps are sample
indices 1 through 500, not seconds. No physical sample rate is invented.

Use the trainer's explicit `--identity-protocol published_files` option.
Validation selects checkpoints. During tuning, leave test scoring disabled.
For the final chosen run, use `--evaluate-checkpoint PRIVATE_RUN_DIRECTORY`
with its exact dataset and manifest, a new output directory and an explicit
device. This restores the selected weights without training and writes a
one-use holdout receipt. See `train.py --help` for bounded model/run parameters.
Public identity training does not demonstrate identity recognition using the
local router's BFI stream.

Sources: [SenseFi author repository](https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark)
and [NTU-Fi dataset, DOI 10.17632/dzvgyxkx2f.1](https://data.mendeley.com/datasets/dzvgyxkx2f/1).
The dataset is CC BY 4.0; the reference repository's MIT code license is separate.
Retain attribution when sharing derived artifacts.

## Private RAC1 captures

```bash
python tools/bfi_learning/prepare.py /private/udp_capture.bin \
  --peer-mac 02:00:00:00:00:01 --trigger-mac 02:00:00:00:00:02 \
  --provenance real --output /private/prepared-rac1
```

Use the explicit source MACs from your capture, not the example values. Input
records are `<QI host_timestamp_ns, datagram_length>` followed by RAC1 v1.
The importer checks framing, length, CRC32, metadata and allocation bounds.
It filters the requested link, orders bounded wrapping sequence numbers,
removes exact duplicates and reports sequence gaps without attributing loss
to a particular device or network layer. Conflicting reused sequences and
ambiguous or repeated hardware timestamps fail closed.

Different receivers and feature grids produce separate datasets. Signed IQ
samples are scaled by 128 or 32768. Hardware timestamps are unwrapped after
sequence ordering and anchored to the first host arrival; this does not
synchronize multiple devices. The SYNTHETIC flag cannot be promoted to real.
CRC provides integrity, not source authentication; real provenance remains an
operator declaration. These captures stay explicitly unlabeled. A MAC address
is not an identity label, and no labels are invented.

## Tests

```bash
python -m unittest discover -s tools/bfi_learning -p 'test_*.py' -v
```

The tests use synthetic fixtures and temporary output directories. They do not
access radios, serial ports, networks, or a GPU.

## Training and controls

`train.py` supports `--encoder lstm` (default) and `--encoder gru`. Both use
training-only normalization and validation-loss checkpoint selection. Identity
metrics include one vote per recording and coverage, alongside correlated
window metrics. Numeric checkpoint tensors, model configuration, data hashes,
software versions and GPU allocator usage are recorded outside Git.

`run_batch.py` runs three fixed seeds serially on Linux with GPU capacity checks,
exclusive ownership, hard timeouts, input/output hashes and bounded logs.
Its optional `--encoder gru` keeps the other experimental settings fixed.
`--seeds 1730 1731` can resume a known subset without repeating a completed seed.
Receipts must match the expected seed, identity protocol, experiment settings
and explicit validation-only flags. Linux process groups provide descendant
cleanup; the launcher rejects other hosts before spawning a GPU child.

Optional `--representation raw|demean`, `--temporal-order original|shuffle`
and `--epochs` support the preregistered [temporal controls](CONTROLS.md).
Nondefault transformations apply only to the public NTU custom identity split.
They preserve recording boundaries and use no label-derived randomness. Whole
recording demeaning is an offline diagnostic, not causal streaming inference.

`static_baseline.py` fits nearest-centroid and fixed-alpha ridge classifiers
using one mean amplitude vector per recording. It uses train/validation only.
This control asks whether temporal order is necessary for this split; high
accuracy alone does not distinguish body information from acquisition context.

## Offline BFI import

```bash
python tools/bfi_learning/prepare_bfi.py /private/capture.pcap \
  --manifest /private/manifest.json --ap AP_MAC --client CLIENT_MAC \
  --provenance real --output /private/prepared-bfi
```

The importer reuses the strict decoder and capture-quality checks under
`tools/bfi`; preserve that relative directory layout when copying the tools.
It verifies the PCAP hash and observation interval, partitions feedback grids,
and creates unlabeled circular-angle features with continuity and window counts.
It performs no capture and invents no identity labels. Short or interrupted
datasets may be unsuitable for the trainer's purged partitions. Report this
without filling gaps or weakening the split.

Read [OVERNIGHT.md](OVERNIGHT.md) for the current experiment checkpoint,
[RESEARCH.md](RESEARCH.md) for the public-data comparison contract,
[SOTA-BFI.md](SOTA-BFI.md) for primary research, and
[BFI-INTEGRATION.md](BFI-INTEGRATION.md) for the production integration boundary.

[CAPTURE-CONTINUITY.md](CAPTURE-CONTINUITY.md) reports separate full-capture
and traffic-period coverage from verified private evidence.
[RESEARCH-NEXT.md](RESEARCH-NEXT.md) pins a public BFI activity dataset and its
proposed domain split; [PUBLIC-BFI-DECODE.md](PUBLIC-BFI-DECODE.md) records the
remaining parser and representation requirements before importing it.

## Public BFI download

```bash
python tools/bfi_learning/download_public_bfi.py \
  --output /private/public-bfi-c3013b3
```

The default fetches the pinned training and validation captures plus the dataset
card. It excludes both held-out views. Use `--resume` only with an existing
receipt from this manifest; completed files are rehashed before reuse. Failed
partials remain recorded. URLs, revision, byte limits and hashes are fixed by
the reviewed manifest. Downloading does not decode packets or authorize them
for training. See `RESEARCH-NEXT.md` for the activity labels and quality rules.
