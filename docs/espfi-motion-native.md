# Native centered-feature inference experiment

Protocol frozen on 2026-10-13 after the separately preserved NumPy reduction
experiment missed every 10% speed gate. This is an adaptive implementation
followup with unchanged numerical and timing thresholds, not an independent
model-accuracy experiment.

## Boundary and hypothesis

The [NumPy experiment](espfi-motion-reduced.md) removes redundant work but
retains multiple Python and NumPy dispatches. A small C kernel can fuse the
remaining per-carrier reductions while retaining sensitive amplitude floor,
logarithm and mean centering exactly in the existing NumPy float64 path.

The optional `NativeMotionBundle` inherits the existing model loader, bounded
prediction API, float64 classifier, lock and owned output behavior. The C
kernel receives only a centered 52 by 190 float64 window and writes 468
float64 features. It computes unfloored carrier energies, global RMS, six
normalized autocorrelations and three normalized mean absolute differences.
The ABI fixes dimensions, checks pointers/lengths and rejects nonfinite input
or output. There is no mutable native global state or heap allocation.

Compile repository-owned source explicitly with the system C compiler,
optimization enabled, fast math disabled and floating point contraction
disabled. Record exact source hash, compiler version, flags and library hash.
Compilation is never automatic during model loading. The new runtime requires
an explicit kernel path and expected SHA-256. Check bounded regular library
bytes and load those exact bytes from a private temporary directory retained
for the runtime lifetime. Model metadata cannot select an executable library.

Loading a shared library executes native code. The digest establishes code
identity and integrity, not safety of arbitrary third-party code. Only use a
trusted build of the reviewed repository source. Current validation targets
Linux; it does not establish compatibility or performance on another system.

## Frozen acceptance

Require float64 output, maximum absolute logit error 1e-10 and identical
classes against both retained buffered and original explicit references on
all 837 training and 70 reused development records at batch sizes one and 32.
The final test was opened once in the first pilot and is not reopened here.

Three fixed rounds each use 100 warmups and 1,000 alternating paired full
prediction calls, rotating development inputs with seeds 42, 43 and 44.
Both median and p95 must improve at least 10% in every round against the
buffered float64 runtime. Include Python validation, NumPy preprocessing,
native call overhead, feature kernel and classifier. Exclude compilation,
library loading, model loading, acquisition, pooling and I/O.

Bind input, model, library, build receipt and source hashes before and after
measurement. Reserve receipts exclusively, preserve failures, and do not
repeat unchanged configurations to seek favorable timing. Test numerical
edge cases, concurrency, output ownership, invalid inputs, library integrity,
ABI mismatch and failed kernel status. Run an instrumented native bounds
probe if the existing compiler supports it.

No production default, artifact, training split or weight changes. Retained
array counts exclude the library, executable pages, native stack and Python
overhead, so they do not prove total process memory improvement. Existing
workspace retains transformed samples until overwritten.

Decision rule: retain an explicit native research option only if all parity,
integrity and every-round speed gates pass. Next action is the same test on
the intended device. Key risks are native memory safety, compiler-dependent
rounding and unmeasured target latency. Acceptance requires the same numerical
and speed thresholds with the prior NumPy runtime available for rollback.

## Measured outcome, 2026-10-13

Retain the explicit native research option. All three paired rounds pass
both speed gates. Relative to the buffered float64 runtime, median latency
falls 29.01% to 30.57% and p95 falls 25.78% to 29.81%.

| Round | Native p50 ms | Buffered p50 ms | Native p95 ms | Buffered p95 ms |
|---|---:|---:|---:|---:|
| 1 | 0.270125 | 0.381550 | 0.405321 | 0.577428 |
| 2 | 0.245240 | 0.353240 | 0.371675 | 0.525422 |
| 3 | 0.265500 | 0.374005 | 0.402225 | 0.541913 |

Every admitted training and development class prediction is unchanged against
both references at batch sizes one and 32. Maximum absolute logit error is
4.773960e-15, below the unchanged 1e-10 limit. The shared ruOS host runs Linux
x86_64, Python 3.12.3 and NumPy 2.4.4, with one configured OpenBLAS/OMP thread.
This does not measure latency from physical sensor acquisition to a decision.

The GCC 13.3.0 build produces a 15,736-byte shared library with SHA-256
`ae3da3fbc6d71dbdb9c605f1ac515d22e3fe108aee7b62ffd22d9751443626ad`.
The unchanged compact weights remain 16,088 bytes. Model arrays remain
487,480 bytes and workspace remains 163,736 bytes. Native stack, mapped
library pages and temporary loading copies are additional costs outside those
array counts. No total memory reduction is claimed. The preceding mixed
precision option remains separate; this experiment does not combine their
benefits or change a production default.

All 113 relevant Python tests pass on ruOS: 85 ESPFi tests and 28 original
training tests. The eight focused native tests pass. The ASan and UBSan
native probe passes on ruOS without disabling checks; CI now runs that probe.
The local executor initially prevented LeakSanitizer from inspecting `/proc`,
so the local attempt is not counted as passing sanitizer evidence. Python
reports automatic temporary kernel directory cleanup at garbage collection
as ResourceWarnings during tests; all test processes exit successfully.

Review found and fixed a benchmark intake issue before measurement: kernel
and build receipt hashing must use bounded, nonblocking regular-file reads.
Tests confirm FIFO rejection before model loading. The kernel path never
comes from model metadata, and executable hashes remain integrity evidence
rather than permission to load untrusted code.

The [reviewed aggregate](validation/espfi-motion-native-20261013.json) includes
every round, gates, exact source/input/build hashes and validation scope.
Full private receipt SHA-256:
`cd9ac18713ca430b01fd0d2887f58b216a11ed78d344cb426047c3887acac556`.
Raw signals, per-record results, native binaries and model weights remain
outside Git. The failed NumPy experiment is preserved without reinterpretation
in [its own report](espfi-motion-reduced.md).

## Build and reproduce

Build the reviewed source explicitly on Linux into a new directory:

```bash
PYTHONPATH=scripts python3 -m csi_training.espfi_motion_native_build \
  --output /path/to/new-kernel-build
```

Record and review `build.json`, including the compiler, flags and library
digest. Pass its `library_sha256` value explicitly to a fresh benchmark:

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
  python3 -m csi_training.espfi_motion_native_benchmark \
  --manifest /path/to/espfi-prepared/manifest.json \
  --model /path/to/espfi-motion-compact-001 \
  --baseline /path/to/espfi-motion-002-worst \
  --kernel /path/to/new-kernel-build/kernel.so \
  --kernel-sha256 REVIEWED_LIBRARY_SHA256 \
  --build-receipt /path/to/new-kernel-build/build.json \
  --output /path/to/new-native-receipt.json
```

For explicit application use after trusting that local build:

```python
from csi_training.espfi_motion_native import NativeMotionBundle

runtime = NativeMotionBundle(
    "/path/to/espfi-motion-compact-001",
    "/path/to/new-kernel-build/kernel.so",
    "REVIEWED_LIBRARY_SHA256",
)
logits = runtime.predict(amplitudes)  # [N, 52, 190] to owned [N, 7] float64
```

Validation commands from the repository root:

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
  python3 -m unittest discover -s scripts/tests -p 'test_espfi*.py' -q
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
  python3 -m unittest discover -s scripts/tests -p 'test_csi_training*.py' -q
cc -std=c11 -O1 -g -fno-fast-math -ffp-contract=off \
  -fsanitize=address,undefined -fno-omit-frame-pointer \
  scripts/csi_training/espfi_motion_native.c \
  scripts/csi_training/espfi_motion_native_probe.c -lm -o /tmp/espfi-native-probe
/tmp/espfi-native-probe
```

Decision: retain the optional native path for the measured shared-CPU speed
gain. Next action: build and validate on the intended device before adoption.
Key risk: native code execution and compiler/platform differences. Acceptance:
the same exact classes, <=1e-10 logits, passing sanitizer checks and >=10%
median and p95 gain in every declared paired round on that device.
Activity accuracy remains 39/70 on reused development data; no new accuracy
or SOTA result follows from this inference optimization.
