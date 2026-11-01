# ADR-365: Public BFI dataset and decoder contract

- **Status**: proposed
- **Date**: 2026-09-29
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: bfi, datasets, decoding, provenance, benchmarks

## Context

Public activity recordings offer a separate route to BFI model development
while local capture continuity is insufficient. They introduce different frame
formats, collection assumptions and licenses. Public HAR activity labels do
not reproduce KIT's person-identification experiment.

**MEASURED:** the pinned CSI-BFI-HAR train/validation download passed exact
size/hash checks for 120 captures plus its license card. A separate body decoder
matched 13,104 quantized angles across 14 sample bodies against tshark 4.2.2 in
two independent executions. Whole-packet admission and activity training remain
incomplete. See [results](../../tools/bfi_learning/RESULTS.md) and the
[decoder audit and reproducer](../../tools/bfi_learning/PUBLIC-BFI-DECODE.md).

## Decision

### Pin provenance and partitions

Use the [author dataset](https://huggingface.co/datasets/foysalhaque/CSI-BFI-HAR-Dataset/tree/c3013b3edc5a563a9cf6759f5d464a9d9b6f456a)
at revision `c3013b3edc5a563a9cf6759f5d464a9d9b6f456a`. The
[manifest](../../tools/bfi_learning/manifests/csi-bfi-har-c3013b3-subset.json)
pins URLs, lengths, SHA-256, labels and split membership. Preserve the pinned
dataset card's GPL-3.0 declaration and attribution separately from code licenses.

| Partition | Author directory | Domain |
| --- | --- | --- |
| Training | HAR-1/BFI/M1 | Day 1, kitchen |
| Validation | HAR-3/BFI/M1 | Day 3, classroom |
| Held out | HAR-5/BFI/M1 and M2 | Day 5, living room; two views |

Keep all views of one event in the same partition. Day and room are confounded;
M1/M2 labels do not establish heterogeneous-chipset transfer. The selected
filenames cover A–T activities and P1–P3 participants.

Default downloads exclude held-out views. The bounded
[downloader](../../tools/bfi_learning/download_public_bfi.py) verifies exact
bytes before atomic completion, rehashes resumed files, retains failed partials
and enforces transfer deadlines and byte limits. Data stays outside Git;
downloaded code is never executed by the downloader.

### Keep body decoding separate from packet admission

[public_bfi_decode.py](../../tools/bfi_learning/public_bfi_decode.py) accepts
only the audited VHT MU 3x1, 80 MHz, Ng=1, codebook-1 complete action body:
1,003 bytes containing one SNR byte, 936 angle bytes and 61 opaque MU-exclusive
bytes. It exposes quantized/radian angles and tone ordinals. Frequency indices,
exclusive-field interpretation and validated matrix reconstruction remain
separate acceptance work.

Twelve sample frames have nonzero fragment-number bits. Successful tshark
decoding and valid CRCs do not resolve their semantics. Retain live MAC guards;
quarantine these frames until an explicit, independently reviewed dataset
policy can justify admission. Require lossless pcapng conversion evidence for
integer timestamps, packet lengths, link types and all original packet bytes.
Do not silently strip headers or rewrite fragment fields to gain admission.

The natural circular representation needs 1,404 features, exceeding the current
trainer's 1,024-feature limit. Approve a bounded representation or revised limit
with allocation tests before integration; do not truncate tones implicitly.

### Admit recording quality before scoring models

Inventory every manifest file, including tiny or unusable traces. Freeze a
five-second observed-interval rule: at least 20 valid unique reports, with no
inter-report gap above one second and no interpolation. Require at least two
independent recordings per activity in each partition/view group that supply
an eligible interval. Missing A–T coverage fails benchmark admission.
Before implementing the census, also freeze interval anchoring, stride and
leading/trailing gap rules: an inter-report gap bound alone does not bound
silence at interval edges. These details remain required acceptance work.

Public files lack trusted recorder start/end manifests. Observed packet spans
cannot establish unobserved leading/trailing coverage. Report rejection reasons,
eligible recording/time coverage and accuracy together. Freeze event/file groups
before generating windows; inspect held-out quality without model selection
against held-out outcomes.

## Alternatives

Reusing the HE/SU parser without format admission, weakening live fragment
guards, or silently dropping sparse classes would change the task while hiding
the change. Keep this adapter explicit and fail incomplete benchmark coverage.

## Consequences

### Positive

Data provenance, protocol support and score coverage remain independently
auditable. New public formats cannot silently weaken live capture validation.

### Negative

Sparse files or unresolved framing may prevent a benchmark despite a successful
download. Format and representation work must precede model optimization.

### Neutral

This is offline research/reference tooling. Production integration must reuse
the Rust protocols in [ADR-291](ADR-291-public-benchmark-evaluation-harness.md)
and scorecards in [ADR-317](ADR-317-benchmark-multi-domain-scorecard.md); this ADR
does not establish a second production trainer or an integrated benchmark score.

## Validation and links

Use `test_download_public_bfi.py` and `test_public_bfi_decode.py` under
`tools/bfi_learning`, then the full packet oracle and quality census specified
in the [decode audit](../../tools/bfi_learning/PUBLIC-BFI-DECODE.md) and
[benchmark protocol](../../tools/bfi_learning/RESEARCH-NEXT.md).

- Depends on [ADR-364](ADR-364-bfi-capture-admission-and-continuity.md)'s distinction
  between packet validity and coverage.
- Preserves [ADR-120](ADR-120-bfld-privacy-class-and-hash-rotation.md).
- [ADR-119](ADR-119-bfld-frame-format-and-wire-protocol.md) defines a BFLD envelope;
  that envelope does not replace raw 802.11 decoding.
