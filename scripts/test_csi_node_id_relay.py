"""
Tests for csi_node_id_relay.py — verifies node_id rewrite logic.

Covers fix for issues #374 / #386:
  All ESP32 nodes ship node_id=1 due to firmware clobber bug.
  Relay must rewrite byte[4] to the correct node_id based on source IP.
"""
import sys
import os
sys.path.insert(0, os.path.dirname(__file__))

from csi_node_id_relay import rewrite_packet, IP_TO_NODE_ID, NODE_ID_MAGICS

# CSI magic (0xC5110001 LE)
CSI_MAGIC = bytes.fromhex("010011c5")
# Vitals magic (0xC5110002 LE)
VITALS_MAGIC = bytes.fromhex("020011c5")

IP_NODE1 = "192.168.13.222"
IP_NODE2 = "192.168.3.246"
IP_UNKNOWN = "10.0.0.99"


def make_packet(magic: bytes, node_id: int, payload_len: int = 20) -> bytes:
    return magic + bytes([node_id]) + bytes(payload_len)


# ── node_id rewrite ───────────────────────────────────────────────────────────

def test_node1_ip_rewrites_to_1_when_clobbered():
    pkt = make_packet(CSI_MAGIC, node_id=1, payload_len=20)  # already correct
    result = rewrite_packet(pkt, IP_NODE1)
    assert result[4] == 1
    assert result is pkt  # no copy when already correct

def test_node2_ip_rewrites_from_1_to_2():
    # firmware clobber: node 2 sends node_id=1
    pkt = make_packet(CSI_MAGIC, node_id=1, payload_len=20)
    result = rewrite_packet(pkt, IP_NODE2)
    assert result[4] == 2, f"expected node_id=2, got {result[4]}"

def test_node2_vitals_rewritten_too():
    pkt = make_packet(VITALS_MAGIC, node_id=1, payload_len=10)
    result = rewrite_packet(pkt, IP_NODE2)
    assert result[4] == 2

def test_payload_unchanged_after_rewrite():
    payload = bytes(range(20))
    pkt = CSI_MAGIC + bytes([1]) + payload
    result = rewrite_packet(pkt, IP_NODE2)
    assert result[5:] == payload, "payload must not be modified"

def test_magic_preserved_after_rewrite():
    pkt = make_packet(CSI_MAGIC, node_id=1)
    result = rewrite_packet(pkt, IP_NODE2)
    assert result[:4] == CSI_MAGIC

# ── passthrough cases ─────────────────────────────────────────────────────────

def test_unknown_ip_passes_through_unchanged():
    pkt = make_packet(CSI_MAGIC, node_id=1)
    result = rewrite_packet(pkt, IP_UNKNOWN)
    assert result is pkt  # identity — not copied

def test_wrong_magic_passes_through_unchanged():
    bad_magic = bytes([0xDE, 0xAD, 0xBE, 0xEF])
    pkt = bad_magic + bytes([1]) + bytes(10)
    result = rewrite_packet(pkt, IP_NODE2)
    assert result is pkt

def test_packet_too_short_passes_through_unchanged():
    pkt = CSI_MAGIC  # only 4 bytes, no node_id byte
    result = rewrite_packet(pkt, IP_NODE2)
    assert result is pkt

def test_already_correct_node_id_not_copied():
    # node 2 already has node_id=2 (won't happen in practice but must not corrupt)
    pkt = make_packet(CSI_MAGIC, node_id=2)
    result = rewrite_packet(pkt, IP_NODE2)
    assert result is pkt  # no copy needed

# ── all three magics are covered ──────────────────────────────────────────────

def test_all_three_magics_are_rewritten():
    wasm_magic = bytes.fromhex("040011c5")  # ADR-040: 0xC5110004
    for magic in [CSI_MAGIC, VITALS_MAGIC, wasm_magic]:
        pkt = magic + bytes([1]) + bytes(10)
        result = rewrite_packet(pkt, IP_NODE2)
        assert result[4] == 2, f"magic {magic.hex()} not rewritten"

# ── ip map contents ───────────────────────────────────────────────────────────

def test_both_nodes_in_default_map():
    assert IP_NODE1 in IP_TO_NODE_ID
    assert IP_NODE2 in IP_TO_NODE_ID
    assert IP_TO_NODE_ID[IP_NODE1] == 1
    assert IP_TO_NODE_ID[IP_NODE2] == 2

def test_custom_ip_map():
    custom_map = {"10.0.0.1": 3}
    pkt = make_packet(CSI_MAGIC, node_id=1)
    result = rewrite_packet(pkt, "10.0.0.1", ip_map=custom_map)
    assert result[4] == 3
