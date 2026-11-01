import math
import struct
import unittest
import public_bfi_decode as decoder


def body(rows=None, token=63, snr=-128):
    control = (2 << 3) | (2 << 6) | (1 << 10) | (1 << 11) | (1 << 15) | (token << 18)
    rows = rows or [(0, 511, 0, 127)] * 234
    angles = b''.join(struct.pack('<I', a | (b << 9) | (c << 18) | (d << 25)) for a,b,c,d in rows)
    return b'\x15\x00' + control.to_bytes(3, 'little') + struct.pack('b', snr) + angles + bytes(range(61))


class DecodeTests(unittest.TestCase):
    def test_endpoints_and_opaque_tail(self):
        result = decoder.decode_body(body())
        self.assertEqual(result['quantized'], ((0,511,0,127),)*234)
        self.assertEqual(result['control']['sounding_token'], 63)
        self.assertEqual(result['avg_snr_code'], -128)
        self.assertEqual(result['exclusive_report'], bytes(range(61)))
        self.assertEqual(result['tone_ordinal'], tuple(range(234)))
        self.assertIsNone(result['frequency_indices'])
        self.assertEqual(result['radians'][0], (math.pi/512,1023*math.pi/512,math.pi/512,255*math.pi/512))

    def test_bitpacking_varies_every_row(self):
        rows = [(i,511-i,i%128,127-i%128) for i in range(234)]
        self.assertEqual(decoder.decode_body(body(rows,0,127))['quantized'], tuple(rows))

    def test_length_and_type_bounds(self):
        original = body()
        for candidate in (original[:-1], original+b'\0', b'', bytearray(original), None):
            with self.assertRaises(ValueError): decoder.decode_body(candidate)

    def test_each_control_tuple_field_rejected(self):
        original=body()
        for bit in (0,3,6,8,9,10,11,12,13,14,15,16,17):
            candidate=bytearray(original); candidate[2+bit//8] ^= 1 << (bit%8)
            with self.subTest(bit=bit), self.assertRaises(ValueError): decoder.decode_body(bytes(candidate))

    def test_category_and_action_rejected(self):
        for prefix in (b'\x1e\0',b'\x15\1'):
            with self.assertRaises(ValueError): decoder.decode_body(prefix+body()[2:])


if __name__ == '__main__': unittest.main()
