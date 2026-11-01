#!/usr/bin/env python3
"""ADR-018 header parsing in the UDP capture scripts.

Builds real ADR-018 frames with the layout used by
v2/crates/wifi-densepose-hardware/src/esp32_parser.rs (build_test_frame) and
firmware/esp32-csi-node/main/csi_collector.c, then checks the fields that
scripts/collect-training-data.py and scripts/record-csi-udp.py decode.

Run:  python -m pytest scripts/tests/test_capture_adr018.py -q
"""

from __future__ import annotations

import importlib.util
import json
import math
import struct
import time
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[1]

ADR018_MAGIC = 0xC5110001
FEATURE_MAGIC = 0xC5110003


def load_script(filename: str):
    """Import a hyphenated script from scripts/ as a module."""
    name = filename.replace("-", "_").removesuffix(".py")
    spec = importlib.util.spec_from_file_location(name, SCRIPTS_DIR / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


collect = load_script("collect-training-data.py")
record = load_script("record-csi-udp.py")


def build_frame(
    node_id: int,
    n_antennas: int,
    pairs: list[tuple[int, int]],
    *,
    freq_mhz: int = 2437,
    seq: int = 1,
    rssi: int = -50,
    noise: int = -95,
    ppdu: int = 0,
    flags: int = 0,
    magic: int = ADR018_MAGIC,
) -> bytes:
    """ADR-018 frame, same byte layout as esp32_parser.rs build_test_frame_with_he.

    0 magic u32 | 4 node_id | 5 n_antennas | 6 n_subcarriers u16 | 8 freq_mhz u32
    | 12 sequence u32 | 16 rssi i8 | 17 noise i8 | 18 ppdu | 19 flags | 20 I/Q i8 pairs
    """
    n_sub = len(pairs) // n_antennas if n_antennas else len(pairs)
    buf = bytearray()
    buf += struct.pack("<I", magic)
    buf.append(node_id)
    buf.append(n_antennas)
    buf += struct.pack("<H", n_sub)
    buf += struct.pack("<I", freq_mhz)
    buf += struct.pack("<I", seq)
    buf += struct.pack("<b", rssi)
    buf += struct.pack("<b", noise)
    buf.append(ppdu)
    buf.append(flags)
    for i, q in pairs:
        buf += struct.pack("<bb", i, q)
    return bytes(buf)


# 64 subcarriers, first pair (3, 4) -> amplitude 5, last pair (-6, 8) -> 10.
PAIRS_64 = [(3, 4)] + [(1, -1)] * 62 + [(-6, 8)]
# Sequence chosen so bytes 14/15 (seq high bytes) differ from rssi/noise.
SEQ = 0x01020304


def test_builder_matches_rust_header_size():
    frame = build_frame(1, 1, [(0, 0)] * 64)
    assert len(frame) == 20 + 64 * 2  # 148 bytes, the HT20 frame size in #1005


# ── collect-training-data.py ────────────────────────────────────────────────

class TestCollectTrainingData:
    def test_single_antenna_header_fields(self):
        frame = build_frame(7, 1, PAIRS_64, seq=SEQ, rssi=-50, noise=-95,
                            ppdu=1, flags=0x01)
        out = collect.parse_packet(frame)
        assert out is not None
        assert out["type"] == "raw_csi"
        assert out["node_id"] == 7
        assert out["antenna_config"] == 1
        assert out["n_antennas"] == 1
        assert out["n_subcarriers"] == 64
        assert out["freq_mhz"] == 2437
        assert out["channel"] == 6
        assert out["seq"] == SEQ
        assert out["rssi"] == -50.0
        assert out["noise_floor"] == -95.0
        assert out["ppdu_type"] == 1
        assert out["flags"] == 0x01

    def test_single_antenna_amplitudes_from_int8_pairs(self):
        out = collect.parse_packet(build_frame(7, 1, PAIRS_64))
        amps = out["subcarriers"]
        assert len(amps) == 64
        assert amps[0] == pytest.approx(5.0)
        assert amps[1] == pytest.approx(math.sqrt(2))
        assert amps[-1] == pytest.approx(10.0)

    def test_multi_antenna_reads_all_pairs(self):
        pairs = [(3, 4)] * 4 + [(6, 8)] * 4  # antenna 0, then antenna 1
        out = collect.parse_packet(build_frame(2, 2, pairs))
        assert out["n_antennas"] == 2
        assert out["n_subcarriers"] == 4
        assert out["subcarriers"] == pytest.approx([5.0] * 4 + [10.0] * 4)

    def test_5ghz_channel(self):
        out = collect.parse_packet(build_frame(1, 1, PAIRS_64, freq_mhz=5180))
        assert out["freq_mhz"] == 5180
        assert out["channel"] == 36

    def test_no_header_timestamp_so_recorder_uses_receive_time(self, tmp_path):
        out = collect.parse_packet(build_frame(3, 1, PAIRS_64, seq=SEQ))
        assert "timestamp" not in out
        rec = collect.CsiRecorder(str(tmp_path), "t", "walking")
        rec.open()
        before = time.time()
        rec.write_frame(out)
        rec.close()
        line = json.loads(rec.file_path.read_text().splitlines()[0])
        assert before - 1 <= line["timestamp"] <= time.time() + 1
        assert line["rssi"] == -50.0
        assert line["features"]["seq"] == SEQ
        assert len(line["subcarriers"]) == 64

    def test_trailing_bytes_ignored(self):
        out = collect.parse_packet(build_frame(1, 1, PAIRS_64) + b"\xff" * 8)
        assert len(out["subcarriers"]) == 64

    def test_truncated_iq_rejected(self):
        assert collect.parse_packet(build_frame(1, 1, PAIRS_64)[:-1]) is None

    def test_short_header_rejected(self):
        assert collect.parse_packet(build_frame(1, 1, PAIRS_64)[:19]) is None

    def test_zero_antennas_rejected(self):
        frame = bytearray(build_frame(1, 1, PAIRS_64))
        frame[5] = 0
        assert collect.parse_packet(bytes(frame)) is None

    def test_feature_packet_still_parses(self):
        feats = [0.1 * i for i in range(8)]
        pkt = struct.pack("<IBBHq8f", FEATURE_MAGIC, 4, 0, 9, 123456, *feats)
        out = collect.parse_packet(pkt)
        assert out["type"] == "features"
        assert out["node_id"] == 4
        assert out["seq"] == 9
        assert out["features"] == pytest.approx(feats)


# ── record-csi-udp.py ───────────────────────────────────────────────────────

class TestRecordCsiUdp:
    def test_single_antenna_header_fields(self):
        frame = build_frame(7, 1, PAIRS_64, seq=SEQ, rssi=-50, noise=-95)
        out = record.parse_csi_packet(frame)
        assert out is not None
        assert out["node_id"] == 7
        assert out["rssi"] == -50
        assert out["noise_floor"] == -95
        assert out["freq_mhz"] == 2437
        assert out["channel"] == 6
        assert out["seq"] == SEQ
        assert out["n_antennas"] == 1
        assert out["n_subcarriers"] == 64

    def test_iq_payload_starts_at_offset_20(self):
        frame = build_frame(7, 1, PAIRS_64)
        out = record.parse_csi_packet(frame)
        assert out["subcarriers"] == 64
        assert out["iq_hex"] == frame[20:].hex()
        assert len(out["amplitudes"]) == 64
        assert out["amplitudes"][0] == 5.0
        assert out["amplitudes"][-1] == 10.0

    def test_multi_antenna(self):
        pairs = [(3, 4)] * 4 + [(6, 8)] * 4
        out = record.parse_csi_packet(build_frame(2, 2, pairs))
        assert out["n_antennas"] == 2
        assert out["n_subcarriers"] == 4
        assert out["subcarriers"] == 8
        assert out["amplitudes"] == [5.0] * 4 + [10.0] * 4

    def test_trailing_bytes_not_treated_as_iq(self):
        frame = build_frame(1, 1, PAIRS_64)
        out = record.parse_csi_packet(frame + b"\x7f" * 6)
        assert out["iq_hex"] == frame[20:].hex()

    def test_other_magic_rejected(self):
        feats = [0.0] * 8
        pkt = struct.pack("<IBBHq8f", FEATURE_MAGIC, 4, 0, 9, 123456, *feats)
        assert record.parse_csi_packet(pkt) is None

    def test_truncated_iq_rejected(self):
        assert record.parse_csi_packet(build_frame(1, 1, PAIRS_64)[:-1]) is None

    def test_short_header_rejected(self):
        assert record.parse_csi_packet(build_frame(1, 1, PAIRS_64)[:19]) is None

    def test_timestamps_present(self):
        out = record.parse_csi_packet(build_frame(1, 1, PAIRS_64))
        assert out["timestamp"].endswith("Z")
        assert isinstance(out["ts_ns"], int)
