# Managed C5 L-LTF8 format contract v1

This software contract does not qualify hardware, sensing accuracy or phase.
The existing C5 producer zero-initializes `lltf_bit_mode`; ESP-IDF6.0.2 defines
that as12-bit components. An old106-byte callback is therefore not evidence
of53 signed-int8 complex samples. Bare53-slot ADR-018 input stays rejected.

Primary references: [SDKv6.0.2 CSI fields](https://github.com/espressif/esp-idf/blob/v6.0.2/components/esp_wifi/include/esp_wifi_he_types.h),
[SDKvendor features](https://github.com/espressif/esp-idf/blob/v6.0.2/docs/en/api-guides/wifi-driver/wifi-vendor-features.rst),
and [Espressif packing clarification](https://github.com/espressif/esp-idf/issues/18982#issuecomment-5420118269).

## Producer claim and exact wire extent

The separate local producer opts into8-bit legacy/forcedL-LTF only and checks
actual CSI-format fields/zero scaling through SDK configuration readback.
STA bandwidth is explicitly set and read back as20MHz; it is a configured
interface/acquisition constraint, not a measurement of full PPDU occupied
bandwidth. Only original non-HT OFDMBB1 (11g/11a), secondary-none,
valid-estimate and106-byte callbacks carry this format claim. Other
metadata is withheld and reported without raw samples. The producer and its
physical callback packing/shape must be validated on one board before
managed capture is attempted; a software test or marker alone is not that
validation. The claim does not raise the existing provenance/trust grade.

A valid datagram has20bytesADR-018 header +106bytesdeclaredIQ +16bytesexact
suffix. The suffix fields are: ASCII C5L8; version1; componentbits8;
SDK-documented imaginary/real byte-order ID1; LLTF53tone-order ID1;
matched-format-readback1; originalBB1; originalsecondary0; STAbandwidth
readback20MHz; originalestimate-valid1; first-word-invalid0/1;
originalbyte-count106little-endian. Unsupported version, order, packing,
readback, PHY, length, flag or trailing byte is refused before readiness.
The first-word flag must agree with ADR-018 bit5 and with the actual zeroed
four-byte prefix. No HE-invalid flag reinterpretation occurs; HE/HT/VHT and
unproven variants are outside this new profile. Existing64/256 paths retain
their old grid IDs, complex-component convention and magnitude mathematics.

Newv1 only admits IEEE20MHz5GHz primary channels36–64 (step4),100–144
(step4, including112),149–177 (step4). This is a bounded format/channel set,
not physical qualification of every channel or regulatory authorization.
The local firmware driver/AP remains responsible for lawful association.
2.4/6GHz and off-grid5MHz values are not silently broadened into this profile.

## Native representation and fixed derived features

Original received IQ bytes are retained privately/losslessly; native shape
remains[1,53,1]. The explicit SDK-documented signed-int8 imaginary/real
interpretation uses complexre=byte1,im=byte0 for this new format only.
Raw phase remains unqualified and is not consumed. Native carrier indices
remain `[0..26,-26..-1]`; they are stored explicitly, not represented as53
uniform FFT bins or padded into64. DC slot0 is always invalid; SDK's invalid
first four bytes additionally invalidate slot1. No slots are deleted or
renumbered, and sanitized wire bytes stay as received.

The derived amplitude-only feature recipe is intentionally conservative:
exclude native positions0and1 on every packet, then pass those51magnitudes
in original SDK order through the existing length-only56feature resampler.
The discontinuity from+26to-26 is not a uniform physical-frequency axis.
This interpolation creates features, not additional measured carriers;
there is no gain/DC normalization or invented padded IQ. Its separate ID is
`ant1-bins53-ppdu0-c5lltf8-keep53-drop2-linear-v3`. Existing64/256v2IDs and
fixtures remain unchanged. The validated claim binds the original header's
node, sequence and frequency to the canonicalizer input. Per-source cache
keys bind format, native grid, PPDU, frequency and bandwidth; normal existing
freshness/cadence/room/model/selection gates remain authoritative.

## Read-only rejection diagnostics

Only an explicit read-only `include_native_diagnostics=1` query on the
existing room-learner status adds `native_source_diagnostics` (with or
without its existing temporal schema query). Default status, `=0` and mutation
ACKs omit that key entirely, preserving older clients' exact-key schemas.
Values other than0/1 are refused; authorization and mutation semantics are
unchanged. A newer client may fall back once to ordinary status on an older
helper's unsupported-query400/422, never on authentication or parse failure.
The optional field is
for the last rejected packet, never a source-ready or admission verdict.
Storage is bounded by256u8nodeIDs; replies cap32fresh records, prioritize
selected sources, expose total/truncation and expire by receiver-monotonic
age after5seconds. Reasons are static whitelist strings; no IQ, amplitudes,
absolute timestamps or credentials are stored or returned. A rare bad
packet cannot evict/invalidate an otherwise ready cohort, while normal
exact source/grid/frequency withdrawal rules are unchanged. Old app readers
may ignore this new optional status field.

## Verification boundary

Synthetic regressions cover exact decoder metadata mutants, lossless native
bytes/order/masks, no unclaimed53admission, original64/256 behavior,
three independent20/21/22sources through actual UDP parser and ingress,
malformed/unsupportedPHY refusal, changedfrequency/grid/model bindings,
and bounded diagnostic expiry/privacy. Physical source/callback and the
separate producer commit/artifact receipts must be recorded independently.
No source changes are applied to the frozen server#2513b8review branch.

## Existing ADR-018 advisory sync metadata

The first metadata-only Node23 wire probe observed290 packets, all142bytes,
53slots, PPDU0,5240MHz, with the exact C5L8/v1 suffix and header flag0x10.
The strict8930 decoder rejected this known flag. The producer serializer sets
bit4 when its ESP-NOW or802.15.4 cross-node synchronization helper reports valid;
the historical host field name is `ieee802154_sync_valid`. This bit does not
select IQ packing, bandwidth, sanitation or calibrated timing.

The corrected C5 decoder accepts exactly flags0x00/0x10/0x20/0x30, with0x20
still required to agree with suffix sanitation and zeroed prefix bytes. All
other flag bits reject. The complete parsed flag metadata is bound to the
validated token and retained in the private canonical packet. Flipping sync
between the validated header and canonicalization input rejects. The bit does
not change grid/projection/IQ/masks, receiver-only clock, unknown source sample
rate, gain/phase qualification, capture authority or provenance.

`tests/fixtures/c5-node23-wire-metadata-20261110.json` contains the actual
metadata-only receipt and no IQ. The corresponding native test combines those
observed header/suffix facts with explicitly **synthetic IQ**, not a captured
wire replay. Exhaustive256-byte flag controls and real ingress/readiness
regressions are synthetic contract evidence; physical admission requires a
new wire probe against the corrected helper. Legacy64/256 math and IDs stay
unchanged. No firmware change is required for this existing metadata bit.
