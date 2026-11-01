# Public VHT MU 3x1 BFI decoding audit

Status: **MEASURED framing audit and independently checked body decoder.**
The separate [public_bfi_decode.py](public_bfi_decode.py) accepts only the exact
VHT MU 3x1 action-body tuple below. Existing capture parsers are unchanged.
Whole-packet admission, dataset import and activity training remain incomplete.

The agent and parent independently ran the pinned sample through tshark 4.2.2
and this decoder. All 14 bodies matched all 13,104 quantized angle integers,
ten control fields per body and signed average-SNR codes. Separate CRC checks
passed for all frames. Radian conversion is covered by synthetic fixtures;
MU-exclusive bytes remain opaque and tone positions are ordinals without a
frequency-index claim. This result does not resolve MAC fragment semantics.

The private reproducer and source snapshot are in
`public-bfi-oracle-v1/` under the local overnight evidence directory; the decoder
SHA-256 is `dfd882502e8a02002e9dd544010b74bd6aef21b5725a5623353a50b6ee7fa03e`.
No raw public capture bytes are committed.

## Bounded sample and result

The publisher-hosted sample is
[HAR-3/BFI/M1/L_3_M1_P3.pcapng](https://huggingface.co/datasets/foysalhaque/CSI-BFI-HAR-Dataset/resolve/c3013b3edc5a563a9cf6759f5d464a9d9b6f456a/HAR-3/BFI/M1/L_3_M1_P3.pcapng),
at repository revision `c3013b3edc5a563a9cf6759f5d464a9d9b6f456a`.
It is 16,052 bytes, SHA256
`ff5f30d4abc4be9796a24eced82fb29be3a3978a4388ca1dcb8a1a957b6cd5f6`.
A second independent in-memory read reproduced its hash and checked block
lengths, report lengths and every MAC FCS. No raw sample bytes are committed.

Observed facts for this sample only:

- One radiotap interface, link type 127, pcapng `if_tsresol=9` (nanoseconds).
- Fourteen Action No Ack frames, frame control `0x00e0`, category 21, action 0.
- Nr=3, Nc=1, 80 MHz, Ng=1, MU feedback, codebook 1, first segment set and zero
  remaining feedback segments. Other dimensions/formats are not established
  by this small sample.
- Each radiotap header is 64 bytes; each MAC frame is 1,031 bytes including its
  four-byte FCS. Each action body is 1,003 bytes. All fourteen CRCs matched.
- Sequence Control's low four bits are nonzero in twelve frames. Only sample
  frames 8 and 14 have zero there. Every More Fragments bit is zero.
- The collaborating auditor's tshark 4.2.2 inspection found 234 subcarrier
  entries per report, with angle labels phi11, phi21, psi21 and psi31, and no
  malformed-frame reports. That is an independent dissector observation, not
  proof that every header field conforms to the standard.

The auditor used `tshark -n -r - -T pdml`, with no preference overrides;
the default reassembly behavior was therefore active. Its separate read-only
`editcap -F` format listing confirmed that the installed version supports
`nsecpcap`. No conversion was performed during this review.

The observed packet timestamps span approximately 196.223 seconds. That span
is between the first and last retained packet, not a verified recorder start/end
window; do not reuse it as full observation coverage without capture metadata.
Public recording names describe activity/trial organization, not person identity.

## Action No Ack Sequence Control: unresolved normative point

Do **not** globally ignore the low four Sequence Control bits just because the
subtype is Action No Ack.

The inspected primary implementation is
[Wireshark packet-ieee80211.c, pinned revision 9209acd8](https://github.com/wireshark/wireshark/blob/9209acd8/epan/dissectors/packet-ieee80211.c).
Its management-frame path reads the two bytes at offset 22 and derives both a
fragment number and a sequence number for all management subtypes. No Action
No Ack exception appears in that path. Later reassembly comments describe a
capture-interface behavior: a complete reassembled packet can retain a nonzero
fragment number while More Fragments is clear. The dissector can process that
case through reassembly. This provides a possible explanation for a successful
tshark decode despite nonzero fragment bits; it does not establish that those
bits are normatively reserved or that the sender is conformant.

IEEE 802.11-2012 clause 8.2.4.4 is the relevant Sequence Control definition;
the 802.11ac-2013 amendment and consolidated later editions also need checking
for a subtype-specific exception. The complete primary IEEE clause was **not
retrieved successfully in this bounded review**: the browser rejected a full
standard PDF as too large, and the draft mirror was inaccessible. No verified
primary IEEE text found here says Action No Ack makes Sequence Control reserved.
Consequently that assertion remains unverified, not a basis for relaxing the
existing guard.

Initial acceptance should retain the existing zero-fragment requirement and
quarantine the other twelve sample frames. Alternatively, a later separately
reviewed dataset-specific adapter may explicitly accept already-complete
reassembled MPDUs, with provenance explaining the capture behavior and an oracle
for the exact original bytes. That must not silently change live capture
semantics. Preserve raw Sequence Control as diagnostic data; neither its high
bits nor the sounding token establish a BFI packet-loss counter. Never zero
those bytes to make an FCS or parser check pass.

## Minimal MU decoding contract

The same pinned Wireshark source's `add_ff_vht_compressed_beamforming_report`
and `dissect_he_feedback_matrix` give the implementation cross-check. Its
comments identify IEEE 802.11ac-2013 Tables 8-53c, 8-53g and 8-53j as the control,
compressed-tone and MU-exclusive-tone references. Restrict a first extension
to the observed tuple:

`VHT, MU, Nr=3, Nc=1, 80 MHz, Ng=1, codebook=1, one complete segment`.

For that tuple:

| Component | Required size/meaning |
|---|---|
| Category and action | 2 bytes, values 21 and 0 |
| MIMO control | 3 little-endian bytes; reject reserved bits, unsupported tuples and segmentation |
| Average SNR | One signed byte for Nc=1; keep endpoint saturation explicit |
| Quantized angles | 234 tones × (9+9+7+7) bits = 936 bytes |
| Angle order per tone | phi11, phi21, psi21, psi31; little-endian bit packing |
| MU-exclusive report | 122 subcarriers × one stream × 4 bits = 61 bytes |
| Complete report payload after control | 1 + 936 + 61 = 998 bytes |
| Whole action body | 2 + 3 + 998 = 1,003 bytes |

These lengths agree with every observed body. Codebook 1 in MU uses phi9/psi7;
reusing the current SU phi6/psi4 branch would misinterpret bytes. Codebook 0 MU
uses different widths and should remain unsupported until independently tested.
Retain the exclusive bytes even if the first representation only uses angles.
Do not accept an arbitrary suffix or drop the 61 bytes as padding.

For 80 MHz Ng1 the compressed matrix grid has 234 reported tones, distinct from
the 122-tone MU-exclusive grid. The exact signed subcarrier index order must be
copied into a reviewed specification from the reference table and checked
against every tshark `scidx` entry; matching only the tone count is inadequate.
For angle dequantization use the established codebook-bin centers, separately
for phi and psi. Matrix reconstruction must use the full 3x1 Givens sequence;
the existing 2-row formula cannot be reused. Norm-one checks are necessary but
insufficient, because an incorrect angle order can still yield a unit vector.

The pinned Wireshark exclusive report field is presented as a label rather
than a numeric per-tone delta-SNR oracle in that source. Its code establishes
the byte span, but do not claim numeric delta-SNR agreement merely because the
section appears in a tree. Signed four-bit values, nibble order, units and
saturation semantics need a separately verified normative definition and
literal-byte fixtures before exposing decoded values.

## Radiotap, FCS and pcapng conversion

[The current offline decoder](../bfi/bfi_decode.py) accepts classic radiotap PCAP,
not pcapng. Prefer a bounded external conversion with the installed Wireshark
`editcap`, retaining nanosecond precision, rather than adding a broad pcapng
parser to the existing tool. The [official editcap manual](https://www.wireshark.org/docs/man-pages/editcap.html)
documents explicit output-format selection; inspect the installed format list
before selecting `nsecpcap`.

Proposed conversion (execute only in a private working directory):

```bash
editcap -F nsecpcap sample.pcapng sample-ns.pcap
```

Before conversion, inventory section endianness, interfaces/link types,
timestamp resolution/offset options, packet captured/original lengths, packet
counts and unknown critical options. Reject mixed link types or ambiguous
interface clocks in the initial adapter. After conversion, require equality of
packet count, exact integer timestamps, captured/original lengths and every
packet's radiotap+MAC byte digest. Hash both source and derivative files and
record editcap version/argv. Do not use ordinary microsecond PCAP: it can erase
timestamp distinctions and would not reproduce this source faithfully.

Honor the radiotap FCS-present flag before removing exactly four trailing
bytes; honor bad-FCS flags and validate CRC over the unmodified MAC bytes.
Absent-FCS is a separate recorded state, never an assumed four-byte suffix.
The existing `strip_radiotap` checks these cases and should remain authoritative.
In this sample the independent CRC pass confirms integrity of all fourteen
frames, including their unusual Sequence Control bytes; CRC is not proof of
protocol conformance or source authenticity.

## Required independent acceptance before a larger import

1. Freeze the sample hash and conversion receipt. Keep original frames intact.
   Record tshark version, preferences, reassembly behavior and exact command.
   A default successful decode alone does not resolve the fragment ambiguity.
2. On the initially admissible frames, compare all control fields, signed
   average SNR, **every** subcarrier index and all 936 quantized angle values per
   report against tshark output. Require a nonempty match and exact consumed
   byte lengths. Include all four angle labels in the comparison schema.
3. Independently check dequantized angles and 3x1 matrix elements on literal,
   nonzero cross-byte fixtures with different phi and psi values. Include
   first/last tones, quantizer edges, exclusive-report boundaries and payload
   truncation/extra-byte cases. Test FCS corruption, partial capture, invalid
   reserved control bits, segmented feedback, 160 MHz and wrong MU codebooks.
4. If accepting the nonzero-fragment subset is later justified, compare the
   original bytes under a named, explicit dataset policy. Keep that policy
   separate from ordinary live Action-frame fragmentation checks and prove
   that it cannot admit a partial body by exact-length and oracle tests.
5. Only then inventory the larger collection: hashes, variants, usable/rejected
   counts per file, monotonicity, gaps, class/trial metadata and content
   duplicates. Do not assume all 240 files share this sample's tuple.

Existing [test_bfi_decode.py](../bfi/test_bfi_decode.py) and
[test_compare_tshark.py](../bfi/test_compare_tshark.py) cover the current SU
contract and must retain their behavior. Generalizing [prepare_bfi.py](prepare_bfi.py)
is a separate step: two circular phi values plus two psi values need six
features per tone, or 1,404 features at 234 tones. That exceeds the current
trainer's 1,024-feature bound. Do not silently drop tones or reuse the 2x2
feature encoder. First define a justified bounded representation or explicitly
review a new input limit. Public HAR labels also do not supply human identity
labels.

## Decision

Proceed with a narrowly scoped MU angle-decoder prototype only after the
fragment-zero subset receives a complete independent angle comparison.
Keep full public-file import blocked on explicit Sequence Control policy,
conversion fidelity and representation bounds. This is a protocol/evidence
boundary, not a reason to relax existing live-capture validation.
