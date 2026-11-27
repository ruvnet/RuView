# Reproducible CSI encoder training

This harness trains a small temporal embedding model from explicit eight-value
CSI features. It is not a pose, presence, or vital-sign classifier. It produces
local candidates only, with no automatic publishing or promotion.

## Set up

Use Python 3.11 or 3.12 and an isolated environment. For the tested CPU setup:

```sh
python3 -m venv .venv-csi
. .venv-csi/bin/activate
python -m pip install torch==2.11.0 --index-url https://download.pytorch.org/whl/cpu
python -m pip install -r scripts/requirements-csi-training.txt
PYTHONPATH=scripts python -m unittest discover -s scripts/tests -p 'test_csi_training*.py' -v
```

Install an appropriate CUDA build of the same PyTorch version for an authorized
GPU host. Record the resolved environment with `python -m pip freeze` alongside
the private run. CPU and GPU results need not be bit-identical. A requested CUDA
run fails when CUDA is unavailable instead of silently falling back to CPU.

## Prepare private data

Create a private JSON manifest with `schema_version: 1` and a `files` array.
Each entry has the following fields:

| Field | Contract |
| --- | --- |
| `path` | Local regular file, relative to the manifest or absolute |
| `sha256` | Exact lowercase SHA256 of the complete file |
| `captured_at` | Actual declared capture time in UTC ISO 8601, not file modification or model upload time |
| `room_id` | Stable pseudonymous room identifier |
| `session_id` | Stable capture session identifier |
| `split` | `train`, `validation`, or `test` |
| `evidence` | `real` or `synthetic` |
| `use_permitted` | Explicit boolean `true`, asserted by the authorized operator |

Keep producer revision, sensor configuration, consent, permitted execution sites,
label provenance and schema documentation with the private source manifest.
The CLI validates structural evidence; it cannot prove those operator assertions.
Never mark a capture usable solely because someone mentioned it in a message.

Canonical JSONL rows contain `timestamp` in seconds, `node_id`, and `features`
as eight finite numbers. The explicit legacy feature stream and the current
collector feature envelope are supported. Raw subcarrier amplitudes are not
silently repurposed as features. Missing labels are never synthesized.

Room separation is the default and requires at least three independent rooms
across training, validation and test. `--group-by session_id` allows distinct
sessions in the same room, but cannot substantiate performance in unseen rooms.
Each partition needs same-node/session positive pairs within two seconds and
negative observations more than thirty seconds apart. The training set alone
determines preprocessing statistics.

## Inspect and train

```sh
python scripts/train-csi-encoder.py inspect --manifest /private/csi/manifest.json
python scripts/train-csi-encoder.py train \
  --manifest /private/csi/manifest.json \
  --output /private/runs/csi-candidate-001 \
  --device cpu --epochs 20 --seed 42 --max-seconds 600
```

For synthetic software experiments, add `--allow-synthetic`; results retain
the `SYNTHETIC` evidence label. Do not use those results as sensor benchmarks.
The output directory must not already exist. No input is changed or uploaded.

For Vast, first provision an authorized isolated host, check its hourly price,
bind only the approved private dataset and pin the code commit and environment.
Then use the same command with `--device cuda`. The CLI does not access provider
keys, create instances, copy private captures, or stop the rental. Its cooperative
runtime limit must be backed by the operator's process and instance deadline.

## Review the candidate

The output includes `model.safetensors`, `preprocessing.json`, `metrics.json`,
and `status.json`. Check completion, manifest digest, implementation source
digests and complete file hashes.
Load preprocessing together with the model; never feed unstandardized features
directly into the network. Validation selects a checkpoint; the final test
comparison uses identical triplets for the candidate and controls.

The loader applies the saved training statistics and verifies completion and
artifact hashes. Hashes detect corruption; they are not a trusted signature.

```python
# Run with PYTHONPATH=scripts. raw_features has shape [N, 8].
from csi_training.model import load_bundle

encoder = load_bundle('/private/runs/csi-candidate-001')
embeddings = encoder.encode(raw_features)
```

`promotionAuthorized` remains false even if the temporal metric improves.
Temporal proximity is a proxy, and shared frame structure can make it easy.
Repeated test-driven tuning also contaminates a test set. Qualify downstream
tasks on fresh independent rooms and sessions before deployment. Quantization,
device RAM, latency, and compatibility with the production runtime remain
separate measured gates.

See [ADR-419](adr/ADR-419-reproducible-csi-encoder-training.md) for boundaries.
