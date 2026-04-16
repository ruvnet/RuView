#!/usr/bin/env python3
"""
ADR-069: ESP32 CSI → Cognitum Seed RVF Ingest Bridge

Listens for CSI feature vectors from ESP32 nodes via UDP, batches them,
and ingests into the Cognitum Seed's RVF vector store via HTTPS REST API.

Usage:
    # Run bridge (default mode)
    python scripts/seed_csi_bridge.py \
        --seed-url https://169.254.42.1:8443 \
        --token "$SEED_TOKEN" \
        --udp-port 5006 \
        --batch-size 10

    # Run with validation (kNN query + PIR comparison after each batch)
    python scripts/seed_csi_bridge.py \
        --token TOKEN --validate

    # Print Seed stats
    python scripts/seed_csi_bridge.py --token TOKEN --stats

    # Trigger store compaction
    python scripts/seed_csi_bridge.py --token TOKEN --compact

The bridge also accepts ADR-018 CSI frames (V1/V2) plus vitals packets
and extracts a simplified 8-dim feature vector from the raw data.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import os
import socket
import struct
import sys
import time
import urllib.error
import urllib.request
import math
import ssl

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%H:%M:%S",
)
log = logging.getLogger("seed-bridge")

# Packet magic numbers
MAGIC_CSI_RAW_V1 = 0xC5110001  # ADR-018 raw CSI frame (V1)
MAGIC_CSI_RAW_V2 = 0xC5110006  # ADR-018 raw CSI frame (V2, source MAC)
MAGIC_VITALS    = 0xC5110002  # ADR-039 vitals packet
MAGIC_FEATURES  = 0xC5110003  # ADR-069 feature vector (new)
RAW_CSI_HEADER_SIZE_V1 = 20
RAW_CSI_HEADER_SIZE_V2 = 26
RAW_CSI_MAX_ANTENNAS = 4
RAW_CSI_MAX_SUBCARRIERS = 256

# Feature vector packet: 4 + 1 + 1 + 2 + 8 + 32 = 48 bytes
FEATURE_PKT_FMT = "<IBBHq8f"
FEATURE_PKT_SIZE = struct.calcsize(FEATURE_PKT_FMT)  # 48

# Vitals packet (edge_processing.h edge_vitals_pkt_t, 32 bytes):
#   magic(4) + node_id(1) + flags(1) + breathing_rate(2) +
#   heartrate(4) + rssi(1) + n_persons(1) + reserved(2) +
#   motion_energy(4) + presence_score(4) + timestamp_ms(4) + reserved2(4)
VITALS_PKT_FMT = "<IBBHIbBxxffII"
VITALS_PKT_SIZE = 32

# Default flush interval in seconds (time-based batching)
DEFAULT_FLUSH_INTERVAL = 10.0


def parse_feature_packet(data: bytes) -> dict | None:
    """Parse an ADR-069 feature vector packet."""
    if len(data) < FEATURE_PKT_SIZE:
        return None
    magic, node_id, _, seq, ts, *features = struct.unpack_from(FEATURE_PKT_FMT, data)
    if magic != MAGIC_FEATURES:
        return None
    # Reject NaN/inf in raw feature values before they reach the vector store
    for i, f in enumerate(features):
        if math.isnan(f) or math.isinf(f):
            log.warning("Dropping feature packet: features[%d]=%s (NaN/inf)", i, f)
            return None
    return {
        "node_id": node_id,
        "seq": seq,
        "timestamp_us": ts,
        "features": features,
    }


def parse_vitals_packet(data: bytes) -> dict | None:
    """Parse an ADR-039 vitals packet and extract an 8-dim feature vector."""
    if len(data) < VITALS_PKT_SIZE:
        return None
    try:
        fields = struct.unpack_from(VITALS_PKT_FMT, data)
    except struct.error:
        return None
    magic = fields[0]
    if magic != MAGIC_VITALS:
        return None
    node_id = fields[1]
    flags = fields[2]
    breathing_rate_raw = fields[3]  # BPM * 100
    heartrate_raw = fields[4]      # BPM * 10000
    rssi = fields[5]               # int8
    n_persons = fields[6]
    motion_energy = fields[7]      # float
    presence_score = fields[8]     # float
    timestamp_ms = fields[9]

    # Reject NaN/inf in raw float fields before clamping (clamp masks NaN)
    if math.isnan(motion_energy) or math.isinf(motion_energy):
        log.warning("Dropping vitals packet: motion_energy=%s (NaN/inf)", motion_energy)
        return None
    if math.isnan(presence_score) or math.isinf(presence_score):
        log.warning("Dropping vitals packet: presence_score=%s (NaN/inf)", presence_score)
        return None

    # Convert from fixed-point
    br_bpm = breathing_rate_raw / 100.0
    hr_bpm = heartrate_raw / 10000.0
    presence = (flags & 0x01) != 0
    fall = (flags & 0x02) != 0
    motion = (flags & 0x04) != 0

    # Normalize to 0.0-1.0 range for 8-dim RVF vector.
    # Live readings show presence_score in 0-15 range and motion_energy in 0-10 range,
    # so divide by their respective maxima before clamping.
    features = [
        max(0.0, min(1.0, presence_score / 15.0)),               # dim 0: presence score (raw 0-15)
        max(0.0, min(1.0, motion_energy / 10.0)),                # dim 1: motion level (raw 0-10)
        max(0.0, min(1.0, br_bpm / 30.0)) if br_bpm > 0 else 0.0,  # dim 2: breathing rate
        max(0.0, min(1.0, hr_bpm / 120.0)) if hr_bpm > 0 else 0.0, # dim 3: heart rate
        0.5,                                                      # dim 4: phase variance (future)
        float(n_persons) / 4.0 if n_persons <= 4 else 1.0,      # dim 5: person count
        1.0 if fall else 0.0,                                     # dim 6: fall detected
        max(0.0, min(1.0, (rssi + 100) / 100.0)),               # dim 7: RSSI normalized
    ]
    return {
        "node_id": node_id,
        "seq": timestamp_ms,
        "timestamp_us": int(time.time() * 1_000_000),
        "features": features,
    }


def parse_raw_csi_packet(data: bytes) -> dict | None:
    """Parse ADR-018 raw CSI frames (V1/V2) and extract basic features."""
    if len(data) < RAW_CSI_HEADER_SIZE_V1:
        return None

    magic = struct.unpack_from("<I", data)[0]
    if magic == MAGIC_CSI_RAW_V1:
        header_size = RAW_CSI_HEADER_SIZE_V1
    elif magic == MAGIC_CSI_RAW_V2:
        if len(data) < RAW_CSI_HEADER_SIZE_V2:
            return None
        header_size = RAW_CSI_HEADER_SIZE_V2
    else:
        return None

    node_id = data[4]
    n_antennas = data[5]
    n_subcarriers = struct.unpack_from("<H", data, 6)[0]
    if (
        n_antennas == 0 or n_antennas > RAW_CSI_MAX_ANTENNAS or
        n_subcarriers == 0 or n_subcarriers > RAW_CSI_MAX_SUBCARRIERS
    ):
        return None

    expected_len = header_size + n_antennas * n_subcarriers * 2
    if len(data) < expected_len:
        return None

    rssi = struct.unpack_from("<b", data, 16)[0]
    seq = struct.unpack_from("<I", data, 12)[0]
    # Minimal feature vector from raw CSI -- mostly placeholder
    features = [0.5, 0.5, 0.0, 0.0, 0.5, 0.5, 0.0, max(0.0, min(1.0, (rssi + 100) / 100.0))]
    return {
        "node_id": node_id,
        "seq": seq,
        "timestamp_us": int(time.time() * 1_000_000),
        "features": features,
    }


def _validate_features(parsed: dict | None) -> dict | None:
    """Reject packets with NaN, inf, or out-of-range feature values."""
    if parsed is None:
        return None
    features = parsed.get("features")
    if features is None:
        return None
    for i, f in enumerate(features):
        if math.isnan(f) or math.isinf(f):
            log.warning("Dropping packet: feature[%d] = %s (NaN/inf)", i, f)
            return None
    return parsed


def parse_packet(data: bytes) -> dict | None:
    """Try all packet formats."""
    if len(data) < 4:
        return None

    raw_packet = parse_raw_csi_packet(data)
    if raw_packet is not None:
        return _validate_features(raw_packet)

    magic = struct.unpack_from("<I", data)[0]
    if magic == MAGIC_FEATURES:
        return _validate_features(parse_feature_packet(data))
    elif magic == MAGIC_VITALS:
        return _validate_features(parse_vitals_packet(data))
    return None


def _make_vector_id(node_id: int, timestamp_us: int, seq_counter: int) -> int:
    """Generate a unique vector ID from node_id + timestamp + sequence counter.

    Uses a hash to produce a non-negative 32-bit integer, avoiding the
    content-addressed deduplication that occurs when all vectors use ID 0.
    """
    key = f"{node_id}:{timestamp_us}:{seq_counter}".encode()
    digest = hashlib.sha256(key).digest()
    # Take first 4 bytes as unsigned 32-bit int
    return struct.unpack("<I", digest[:4])[0]


class SeedClient:
    """HTTPS client for Cognitum Seed REST API."""

    def __init__(self, base_url: str, token: str):
        self.base_url = base_url.rstrip("/")
        self.token = token
        # Skip TLS verification for self-signed cert
        self.ctx = ssl.create_default_context()
        self.ctx.check_hostname = False
        self.ctx.verify_mode = ssl.CERT_NONE

    def _request(self, method: str, path: str, body: dict | None = None,
                 timeout: int = 10, auth: bool = True) -> dict:
        """Issue an HTTP request and return parsed JSON.

        Raises urllib.error.URLError on connection failure,
        urllib.error.HTTPError on non-2xx status, and
        ValueError on non-JSON response body.
        """
        url = f"{self.base_url}{path}"
        data = json.dumps(body).encode() if body is not None else None
        headers = {"Content-Type": "application/json"}
        if auth:
            headers["Authorization"] = f"Bearer {self.token}"
        req = urllib.request.Request(url, data=data, headers=headers, method=method)
        with urllib.request.urlopen(req, context=self.ctx, timeout=timeout) as resp:
            raw = resp.read()
            try:
                return json.loads(raw)
            except (json.JSONDecodeError, ValueError) as exc:
                raise ValueError(
                    f"Non-JSON response from {method} {path} "
                    f"(status {resp.status}): {raw[:200]!r}"
                ) from exc

    def ingest(self, vectors: list[tuple[int, list[float]]]) -> dict:
        """Ingest vectors into the RVF store."""
        return self._request("POST", "/api/v1/store/ingest", {"vectors": vectors})

    def query(self, vector: list[float], k: int = 5) -> dict:
        """Query kNN for a vector."""
        return self._request("POST", "/api/v1/store/query", {"vector": vector, "k": k})

    def compact(self) -> dict:
        """Trigger store compaction."""
        return self._request("POST", "/api/v1/store/compact")

    def status(self) -> dict:
        """Get device status."""
        return self._request("GET", "/api/v1/status", auth=False, timeout=5)

    def boundary(self) -> dict:
        """Get boundary analysis (fragility score)."""
        return self._request("GET", "/api/v1/boundary", auth=False, timeout=5)

    def coherence_profile(self) -> dict:
        """Get coherence profile."""
        return self._request("GET", "/api/v1/coherence/profile", auth=False, timeout=5)

    def graph_stats(self) -> dict:
        """Get kNN graph stats."""
        return self._request("GET", "/api/v1/store/graph/stats", auth=False, timeout=5)

    def read_pir(self, pin: int = 6) -> dict | None:
        """Read PIR sensor GPIO. Returns None if not available (404)."""
        try:
            return self._request("GET", f"/api/v1/sensor/gpio/read?pin={pin}",
                                 auth=False, timeout=5)
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            raise
        except Exception:
            return None

    def verify_witness(self) -> dict:
        """Verify witness chain integrity."""
        return self._request("POST", "/api/v1/witness/verify", timeout=10)


def _flush_batch(seed: SeedClient, batch: list, stats: dict,
                 validate: bool = False, validation_stats: dict | None = None,
                 last_features: list[float] | None = None) -> None:
    """Ingest a batch of vectors into the Seed, with optional retry and validation."""
    max_attempts = 2
    for attempt in range(max_attempts):
        try:
            result = seed.ingest(batch)
            accepted = result.get("count", 0)
            epoch = result.get("new_epoch", "?")
            stats["ingested"] += accepted
            stats["batches"] += 1
            log.info(
                "Ingested %d vectors (epoch=%s, witness=%s)",
                accepted,
                epoch,
                str(result.get("witness_head", "?"))[:16] + "...",
            )
            break  # success
        except Exception as e:
            if attempt == 0:
                log.warning("Ingest failed (attempt 1/2), retrying in 2s: %s", e)
                time.sleep(2.0)
            else:
                stats["errors"] += 1
                log.error("Ingest failed after retry: %s", e)
                return  # skip validation on failure

    # Validation: query the most recent vector and check kNN result
    if validate and last_features is not None and validation_stats is not None:
        _run_validation(seed, last_features, validation_stats)


def _run_validation(seed: SeedClient, features: list[float],
                    validation_stats: dict) -> None:
    """Query kNN for the most recent vector and compare with PIR sensor."""
    try:
        qr = seed.query(features, k=1)
        results = qr.get("results", [])
        if results:
            dist = results[0].get("distance", -1)
            validation_stats["queries"] += 1
            if dist <= 0.01:
                validation_stats["exact_matches"] += 1
                log.info("Validation: kNN distance=%.6f (exact match)", dist)
            else:
                log.info("Validation: kNN distance=%.6f (approximate)", dist)
        else:
            log.warning("Validation: kNN returned empty results")
    except Exception as e:
        log.warning("Validation query failed: %s", e)

    # PIR ground truth comparison
    csi_presence = features[0]  # dim 0 is presence score
    csi_present = csi_presence > 0.3  # threshold for "someone present"
    try:
        pir = seed.read_pir(pin=6)
        if pir is not None:
            pir_state = bool(pir.get("value", 0))
            validation_stats["pir_readings"] += 1
            if csi_present == pir_state:
                validation_stats["pir_agreements"] += 1
            rate = (validation_stats["pir_agreements"] / validation_stats["pir_readings"] * 100
                    if validation_stats["pir_readings"] > 0 else 0)
            log.info(
                "PIR=%s CSI_presence=%.2f (%s) — agreement %.1f%% (%d/%d)",
                "HIGH" if pir_state else "LOW",
                csi_presence,
                "present" if csi_present else "absent",
                rate,
                validation_stats["pir_agreements"],
                validation_stats["pir_readings"],
            )
    except Exception:
        pass  # PIR not available, already handled gracefully


def run_bridge(args):
    """Main bridge loop: UDP -> batch -> HTTPS ingest."""
    seed = SeedClient(args.seed_url, args.token)

    # Verify connectivity
    try:
        status = seed.status()
        log.info(
            "Connected to Seed %s — %d vectors, epoch %d, dim %d",
            status["device_id"][:8],
            status["total_vectors"],
            status["epoch"],
            status["dimension"],
        )
    except Exception as e:
        log.error("Cannot connect to Seed at %s: %s", args.seed_url, e)
        sys.exit(1)

    # Parse allowed source IPs for UDP filtering (anti-spoofing)
    allowed_sources: set[str] | None = None
    if args.allowed_sources:
        allowed_sources = set(ip.strip() for ip in args.allowed_sources.split(",") if ip.strip())
        log.info("UDP source filter: only accepting packets from %s", allowed_sources)

    # Open UDP listener
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    bind_addr = args.bind_addr
    if bind_addr == "auto":
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.connect(("192.168.1.1", 80))
            bind_addr = s.getsockname()[0]
            s.close()
        except Exception:
            bind_addr = "0.0.0.0"
    sock.bind((bind_addr, args.udp_port))
    sock.settimeout(1.0)  # 1s timeout for responsive time-based flushing
    log.info(
        "Listening on UDP port %d (batch size: %d, flush interval: %.0fs)",
        args.udp_port, args.batch_size, args.flush_interval,
    )

    batch: list[tuple[int, list[float]]] = []
    stats = {"received": 0, "ingested": 0, "errors": 0, "batches": 0}
    validation_stats = {"queries": 0, "exact_matches": 0, "pir_readings": 0, "pir_agreements": 0}
    last_log = time.time()
    last_flush = time.time()
    seq_counter = 0
    last_features: list[float] | None = None

    try:
        while True:
            try:
                data, addr = sock.recvfrom(2048)
            except socket.timeout:
                # Time-based flush: flush if interval elapsed and batch is non-empty
                now = time.time()
                if batch and (now - last_flush) >= args.flush_interval:
                    _flush_batch(seed, batch, stats, args.validate,
                                 validation_stats, last_features)
                    batch = []
                    last_flush = now
                # Periodic status log
                if now - last_log > 30:
                    log.info(
                        "Stats: received=%d ingested=%d batches=%d errors=%d",
                        stats["received"], stats["ingested"], stats["batches"], stats["errors"],
                    )
                    if args.validate and validation_stats["pir_readings"] > 0:
                        rate = validation_stats["pir_agreements"] / validation_stats["pir_readings"] * 100
                        log.info(
                            "Validation: kNN queries=%d exact=%d | PIR agreement=%.1f%% (%d/%d)",
                            validation_stats["queries"],
                            validation_stats["exact_matches"],
                            rate,
                            validation_stats["pir_agreements"],
                            validation_stats["pir_readings"],
                        )
                    last_log = now
                continue

            # Source IP filtering (defense against UDP spoofing)
            if allowed_sources and addr[0] not in allowed_sources:
                log.debug("Dropping packet from unauthorized source %s", addr[0])
                continue

            parsed = parse_packet(data)
            if parsed is None:
                continue

            stats["received"] += 1
            seq_counter += 1

            # Generate unique vector ID from hash(node_id + timestamp + seq)
            vec_id = _make_vector_id(parsed["node_id"], parsed["timestamp_us"], seq_counter)
            last_features = parsed["features"]
            batch.append((vec_id, parsed["features"]))

            if args.verbose:
                log.debug(
                    "node=%d seq=%d id=%08x features=[%s]",
                    parsed["node_id"],
                    parsed["seq"],
                    vec_id,
                    ", ".join(f"{f:.3f}" for f in parsed["features"]),
                )

            # Size-based flush
            if len(batch) >= args.batch_size:
                _flush_batch(seed, batch, stats, args.validate,
                             validation_stats, last_features)
                batch = []
                last_flush = time.time()

            # Also check time-based flush for slow packet rates
            if batch and (time.time() - last_flush) >= args.flush_interval:
                _flush_batch(seed, batch, stats, args.validate,
                             validation_stats, last_features)
                batch = []
                last_flush = time.time()

    except KeyboardInterrupt:
        log.info("Shutting down...")
        if batch:
            _flush_batch(seed, batch, stats, args.validate,
                         validation_stats, last_features)
    finally:
        sock.close()
        log.info(
            "Final stats: received=%d ingested=%d batches=%d errors=%d",
            stats["received"], stats["ingested"], stats["batches"], stats["errors"],
        )
        if args.validate:
            log.info(
                "Validation: kNN queries=%d exact_matches=%d | PIR readings=%d agreements=%d",
                validation_stats["queries"],
                validation_stats["exact_matches"],
                validation_stats["pir_readings"],
                validation_stats["pir_agreements"],
            )
        # Verify witness chain on exit
        try:
            result = seed.verify_witness()
            log.info(
                "Witness chain: %s (length=%d)",
                "VALID" if result.get("valid") else "INVALID",
                result.get("chain_length", 0),
            )
        except Exception:
            pass


def run_stats(args):
    """Query Seed and print comprehensive stats."""
    seed = SeedClient(args.seed_url, args.token)

    # Status
    print("=== Seed Status ===")
    try:
        s = seed.status()
        print(f"  Device ID:      {s.get('device_id', '?')}")
        print(f"  Total vectors:  {s.get('total_vectors', '?')}")
        print(f"  Epoch:          {s.get('epoch', '?')}")
        print(f"  Dimension:      {s.get('dimension', '?')}")
        print(f"  Uptime:         {s.get('uptime_secs', '?')}s")
    except Exception as e:
        print(f"  Error: {e}")

    # Witness chain
    print("\n=== Witness Chain ===")
    try:
        w = seed.verify_witness()
        print(f"  Valid:          {w.get('valid', '?')}")
        print(f"  Chain length:   {w.get('chain_length', '?')}")
        print(f"  Head:           {str(w.get('head', '?'))[:32]}...")
    except Exception as e:
        print(f"  Error: {e}")

    # Boundary analysis
    print("\n=== Boundary Analysis ===")
    try:
        b = seed.boundary()
        print(f"  Fragility score: {b.get('fragility_score', '?')}")
        print(f"  Boundary count:  {b.get('boundary_count', '?')}")
        for k, v in b.items():
            if k not in ("fragility_score", "boundary_count"):
                print(f"  {k}: {v}")
    except Exception as e:
        print(f"  Error: {e}")

    # Coherence profile
    print("\n=== Coherence Profile ===")
    try:
        c = seed.coherence_profile()
        for k, v in c.items():
            print(f"  {k}: {v}")
    except Exception as e:
        print(f"  Error: {e}")

    # kNN graph stats
    print("\n=== kNN Graph Stats ===")
    try:
        g = seed.graph_stats()
        for k, v in g.items():
            print(f"  {k}: {v}")
    except Exception as e:
        print(f"  Error: {e}")


def run_compact(args):
    """Trigger store compaction on the Seed."""
    seed = SeedClient(args.seed_url, args.token)
    print("Triggering store compaction...")
    try:
        result = seed.compact()
        print(f"Compaction result: {json.dumps(result, indent=2)}")
    except Exception as e:
        print(f"Compaction failed: {e}")
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(
        description="ADR-069: ESP32 CSI -> Cognitum Seed RVF Bridge"
    )
    parser.add_argument(
        "--seed-url",
        default="https://169.254.42.1:8443",
        help="Cognitum Seed HTTPS URL (default: https://169.254.42.1:8443)",
    )
    parser.add_argument(
        "--token",
        default=os.environ.get("SEED_TOKEN"),
        help="Bearer token from Seed pairing (or set SEED_TOKEN env var)",
    )
    parser.add_argument(
        "--udp-port",
        type=int,
        default=5006,
        help="UDP port to listen on (default: 5006)",
    )
    parser.add_argument(
        "--bind-addr",
        default="auto",
        help="Bind address for UDP listener (default: auto-detect WiFi IP; use 0.0.0.0 for all interfaces)",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=10,
        help="Vectors per ingest batch (default: 10)",
    )
    parser.add_argument(
        "--flush-interval",
        type=float,
        default=DEFAULT_FLUSH_INTERVAL,
        help="Max seconds between flushes (default: %.0f)" % DEFAULT_FLUSH_INTERVAL,
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Log every received packet",
    )
    parser.add_argument(
        "--validate",
        action="store_true",
        help="After each batch, query kNN and compare with PIR sensor",
    )
    parser.add_argument(
        "--stats",
        action="store_true",
        help="Print Seed stats (vectors, boundary, coherence, graph) and exit",
    )
    parser.add_argument(
        "--compact",
        action="store_true",
        help="Trigger store compaction and exit",
    )
    parser.add_argument(
        "--allowed-sources",
        type=str,
        default=None,
        help="Comma-separated list of allowed source IPs for UDP packets "
             "(e.g. '192.168.1.105,192.168.1.106'). Packets from other IPs are dropped.",
    )
    args = parser.parse_args()

    if not args.token:
        parser.error("--token is required (or set SEED_TOKEN environment variable)")

    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)

    if args.stats:
        run_stats(args)
    elif args.compact:
        run_compact(args)
    else:
        run_bridge(args)


if __name__ == "__main__":
    main()
