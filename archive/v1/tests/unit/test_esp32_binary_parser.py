"""Tests for ESP32BinaryParser (ADR-018 binary frame format)."""

import asyncio
import math
import socket
import struct
import threading
import time

import numpy as np
import pytest

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', 'src'))

from hardware.csi_extractor import (
    ESP32BinaryParser,
    CSIExtractor,
    CSIParseError,
    CSIExtractionError,
)

# ADR-018 constants
MAGIC = 0xC5110001
HEADER_FMT = '<IBBHIIBB2x'
HEADER_SIZE = 20


def build_binary_frame(
    node_id: int = 1,
    n_antennas: int = 1,
    n_subcarriers: int = 4,
    freq_mhz: int = 2437,
    sequence: int = 0,
    rssi: int = -50,
    noise_floor: int = -90,
    iq_pairs: list = None,
) -> bytes:
    """Build an ADR-018 binary frame for testing."""
    if iq_pairs is None:
        iq_pairs = [(i % 50, (i * 2) % 50) for i in range(n_antennas * n_subcarriers)]

    rssi_u8 = rssi & 0xFF
    noise_u8 = noise_floor & 0xFF

    header = struct.pack(
        HEADER_FMT,
        MAGIC,
        node_id,
        n_antennas,
        n_subcarriers,
        freq_mhz,
        sequence,
        rssi_u8,
        noise_u8,
    )

    iq_data = b''
    for i_val, q_val in iq_pairs:
        iq_data += struct.pack('<bb', i_val, q_val)

    return header + iq_data


class TestESP32BinaryParser:
    """Tests for ESP32BinaryParser."""

    def setup_method(self):
        self.parser = ESP32BinaryParser()

    def test_parse_valid_binary_frame(self):
        """Parse a well-formed ADR-018 binary frame."""
        iq = [(3, 4), (0, 10), (5, 12), (7, 0)]
        frame_bytes = build_binary_frame(
            node_id=1, n_antennas=1, n_subcarriers=4,
            freq_mhz=2437, sequence=42, rssi=-50, noise_floor=-90,
            iq_pairs=iq,
        )

        result = self.parser.parse(frame_bytes)

        assert result.num_antennas == 1
        assert result.num_subcarriers == 4
        assert result.amplitude.shape == (1, 4)
        assert result.phase.shape == (1, 4)
        assert result.metadata['node_id'] == 1
        assert result.metadata['sequence'] == 42
        assert result.metadata['rssi_dbm'] == -50
        assert result.metadata['noise_floor_dbm'] == -90
        assert result.metadata['channel_freq_mhz'] == 2437

        # Check amplitude for I=3, Q=4 -> sqrt(9+16) = 5.0
        assert abs(result.amplitude[0, 0] - 5.0) < 0.001
        # I=0, Q=10 -> 10.0
        assert abs(result.amplitude[0, 1] - 10.0) < 0.001

    def test_parse_frame_too_short(self):
        """Reject frames shorter than the 20-byte header."""
        with pytest.raises(CSIParseError, match="too short"):
            self.parser.parse(b'\x00' * 10)

    def test_parse_invalid_magic(self):
        """Reject frames with wrong magic number."""
        bad_frame = build_binary_frame()
        # Corrupt magic
        bad_frame = b'\xFF\xFF\xFF\xFF' + bad_frame[4:]
        with pytest.raises(CSIParseError, match="Invalid magic"):
            self.parser.parse(bad_frame)

    def test_parse_multi_antenna_frame(self):
        """Parse a frame with 3 antennas and 4 subcarriers."""
        n_ant = 3
        n_sc = 4
        iq = [(i + 1, i + 2) for i in range(n_ant * n_sc)]

        frame_bytes = build_binary_frame(
            node_id=5, n_antennas=n_ant, n_subcarriers=n_sc,
            iq_pairs=iq,
        )

        result = self.parser.parse(frame_bytes)

        assert result.num_antennas == 3
        assert result.num_subcarriers == 4
        assert result.amplitude.shape == (3, 4)
        assert result.phase.shape == (3, 4)

    def test_udp_read_with_mock_server(self):
        """Send a frame via UDP and verify CSIExtractor receives it."""
        # Find a free port
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        sock.close()

        frame_bytes = build_binary_frame(
            node_id=3, n_antennas=1, n_subcarriers=4,
            freq_mhz=2412, sequence=99,
        )

        config = {
            'hardware_type': 'esp32',
            'parser_format': 'binary',
            'sampling_rate': 100,
            'buffer_size': 2048,
            'timeout': 2,
            'aggregator_host': '127.0.0.1',
            'aggregator_port': port,
        }

        extractor = CSIExtractor(config)

        async def run_test():
            # Connect
            await extractor.connect()

            # Send frame after a short delay from a background thread
            def send():
                time.sleep(0.2)
                s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                s.sendto(frame_bytes, ('127.0.0.1', port))
                s.close()

            sender = threading.Thread(target=send, daemon=True)
            sender.start()

            result = await extractor.extract_csi()
            sender.join(timeout=2)

            assert result.metadata['node_id'] == 3
            assert result.metadata['sequence'] == 99
            assert result.num_subcarriers == 4

            await extractor.disconnect()

        asyncio.run(run_test())

    def test_udp_timeout(self):
        """Verify timeout when no UDP server is sending data."""
        # Find a free port (nothing will send to it)
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        sock.close()

        config = {
            'hardware_type': 'esp32',
            'parser_format': 'binary',
            'sampling_rate': 100,
            'buffer_size': 2048,
            'timeout': 0.5,
            'retry_attempts': 1,
            'aggregator_host': '127.0.0.1',
            'aggregator_port': port,
        }

        extractor = CSIExtractor(config)

        async def run_test():
            await extractor.connect()
            with pytest.raises(CSIExtractionError, match="timed out"):
                await extractor.extract_csi()
            await extractor.disconnect()

        asyncio.run(run_test())
