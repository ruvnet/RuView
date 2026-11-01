"""Bounded offline VHT MU 3x1 action BODY decoding, standard library only.

This does not admit packets: callers must separately validate MAC headers,
fragment/Sequence Control semantics, radiotap and FCS. In particular, decoding
a body does not authorize ignoring a nonzero MAC fragment number.
Only complete 80 MHz Ng1 codebook1 reports are supported. Tone ordinals are
provided, with no frequency-index claim. Exclusive-report bytes stay opaque.
Control/angle reference: Wireshark packet-ieee80211.c revision 9209acd8,
add_ff_vht_compressed_beamforming_report (802.11ac tables 8-53c/g/j).
No external implementation code is incorporated.
"""
import math
import struct

BODY_BYTES = 1003
TONE_COUNT = 234
ANGLE_NAMES = ('phi11', 'phi21', 'psi21', 'psi31')


def decode_body(body):
    """Return immutable numeric angle rows and exact opaque trailing bytes.

    avg_snr_code is the signed int8 code, not an inferred dB measurement.
    Angles use quantization-bin centers in radians. Allocation is fixed-size.
    """
    if not isinstance(body, bytes) or len(body) != BODY_BYTES:
        raise ValueError('expected exactly 1003 bytes of action body')
    if body[:2] != b'\x15\x00':
        raise ValueError('unsupported category/action')
    control = int.from_bytes(body[2:5], 'little')
    fields = dict(nc=(control & 7) + 1, nr=((control >> 3) & 7) + 1,
                  width_mhz=(20, 40, 80, 160)[(control >> 6) & 3],
                  grouping_index=(control >> 8) & 3,
                  codebook=(control >> 10) & 1, feedback_type=(control >> 11) & 1,
                  remaining_segments=(control >> 12) & 7,
                  first_segment=(control >> 15) & 1,
                  reserved=(control >> 16) & 3, sounding_token=(control >> 18) & 63)
    expected = dict(nc=1, nr=3, width_mhz=80, grouping_index=0, codebook=1,
                    feedback_type=1, remaining_segments=0, first_segment=1, reserved=0)
    if any(fields[key] != value for key, value in expected.items()):
        raise ValueError('unsupported VHT MU tuple, reserved bits or segmentation')
    quantized = tuple((word & 511, (word >> 9) & 511,
                       (word >> 18) & 127, (word >> 25) & 127)
                      for (word,) in struct.iter_unpack('<I', body[6:942]))
    # phi denominator 2**9 and psi denominator 2**(7+2) both equal 512.
    radians = tuple(tuple((2 * value + 1) * math.pi / 512
                          for value in row) for row in quantized)
    return dict(standard='VHT', control=fields, tone_ordinal=tuple(range(TONE_COUNT)),
                frequency_indices=None, angle_names=ANGLE_NAMES,
                quantized=quantized, radians=radians,
                avg_snr_code=struct.unpack('b', body[5:6])[0],
                exclusive_report=body[942:])
