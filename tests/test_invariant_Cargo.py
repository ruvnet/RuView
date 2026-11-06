import pytest
import re
import os


ADVERSARIAL_PAYLOADS = [
    # Null bytes and binary data
    b"\x00" * 100,
    b"\xff\xfe\xfd",
    b"\x00\x01\x02\x03",
    # Oversized inputs
    b"A" * 65536,
    b"B" * 1048576,
    # Format string attacks
    b"%s%s%s%s%s%s%s%s%s%s",
    b"%x%x%x%x%x%x%x%x",
    b"%n%n%n%n",
    # SQL injection patterns
    b"' OR '1'='1",
    b"'; DROP TABLE users; --",
    b"1; SELECT * FROM secrets",
    # Path traversal
    b"../../../etc/passwd",
    b"..\\..\\..\\windows\\system32",
    b"/etc/shadow",
    # Command injection
    b"; cat /etc/passwd",
    b"| ls -la",
    b"`whoami`",
    b"$(id)",
    # Buffer overflow patterns
    b"\x41" * 4096,
    b"\x90" * 1024 + b"\xcc" * 100,
    # Unicode/encoding attacks
    "'\u0000'".encode("utf-8"),
    "\uFFFD\uFFFE\uFFFF".encode("utf-8"),
    # Empty and whitespace
    b"",
    b"   ",
    b"\t\n\r",
    # Version string injection
    b"openssl-1.0.1e",
    b"openssl 1.0.1f",
    b"1.0.1g",
    # Malformed version strings
    b"999.999.999",
    b"-1.-1.-1",
    b"0.0.0",
    # Special characters
    b"!@#$%^&*()",
    b"<script>alert(1)</script>",
    b"<?xml version='1.0'?><!DOCTYPE foo [<!ENTITY xxe SYSTEM 'file:///etc/passwd'>]>",
]


def parse_cargo_lock_openssl_version(content: str) -> list:
    """Extract openssl-related package versions from Cargo.lock content."""
    versions = []
    lines = content.split('\n')
    in_openssl_package = False
    current_name = None
    
    for line in lines:
        line = line.strip()
        if line.startswith('name = '):
            current_name = line.split('=', 1)[1].strip().strip('"')
            in_openssl_package = 'openssl' in current_name.lower()
        elif in_openssl_package and line.startswith('version = '):
            version_str = line.split('=', 1)[1].strip().strip('"')
            versions.append((current_name, version_str))
    
    return versions


def is_safe_version_string(version_str: str) -> bool:
    """Check that a version string only contains safe characters."""
    safe_pattern = re.compile(r'^[0-9]+\.[0-9]+\.[0-9]+([.\-][a-zA-Z0-9]+)*$')
    return bool(safe_pattern.match(version_str))


def simulate_version_comparison(version_str: str) -> bool:
    """Simulate version comparison without executing arbitrary code."""
    try:
        parts = version_str.split('.')
        if len(parts) < 2:
            return False
        for part in parts[:3]:
            base = part.split('-')[0].split('+')[0]
            if base:
                int(base)
        return True
    except (ValueError, AttributeError):
        return False


@pytest.mark.parametrize("payload", ADVERSARIAL_PAYLOADS, ids=[f"p{i}" for i in range(len(ADVERSARIAL_PAYLOADS))])
def test_openssl_version_handling_security_invariant(payload):
    """Invariant: Adversarial inputs must not cause unsafe behavior when processed
    as version strings or package metadata. Version parsing must remain safe and
    predictable regardless of input content."""
    
    # Convert payload to string safely
    if isinstance(payload, bytes):
        try:
            payload_str = payload.decode('utf-8', errors='replace')
        except Exception:
            payload_str = repr(payload)
    else:
        payload_str = str(payload)
    
    # Invariant 1: Version string validation must not crash
    try:
        is_safe = is_safe_version_string(payload_str)
        # If the payload is adversarial, it should NOT be considered a safe version
        if any(c in payload_str for c in [';', '|', '`', '$', '<', '>', '&', '\x00', '%n', '%s', '%x']):
            assert not is_safe, (
                f"Adversarial payload was incorrectly accepted as safe version: {repr(payload_str)}"
            )
    except Exception as e:
        pytest.fail(f"Version validation raised unexpected exception for payload {repr(payload_str)}: {e}")
    
    # Invariant 2: Version comparison simulation must not execute arbitrary code
    try:
        result = simulate_version_comparison(payload_str)
        # Result must be a boolean - no side effects
        assert isinstance(result, bool), (
            f"Version comparison returned non-boolean for payload {repr(payload_str)}"
        )
    except Exception as e:
        pytest.fail(f"Version comparison raised unexpected exception for payload {repr(payload_str)}: {e}")
    
    # Invariant 3: Cargo.lock-like content with adversarial version must be parseable safely
    fake_cargo_lock = f'''
[[package]]
name = "openssl"
version = "{payload_str}"
source = "registry+https://github.com/rust-lang/crates.io-index"
'''
    try:
        versions = parse_cargo_lock_openssl_version(fake_cargo_lock)
        # Must return a list (even if empty or with the injected value)
        assert isinstance(versions, list), (
            f"Parser returned non-list for payload {repr(payload_str)}"
        )
        # The parser must not execute any code from the payload
        for name, ver in versions:
            assert isinstance(name, str), "Package name must be a string"
            assert isinstance(ver, str), "Version must be a string"
    except Exception as e:
        pytest.fail(f"Cargo.lock parsing raised unexpected exception for payload {repr(payload_str)}: {e}")
    
    # Invariant 4: No environment variables should be modified by processing the payload
    env_before = dict(os.environ)
    try:
        _ = is_safe_version_string(payload_str)
        _ = simulate_version_comparison(payload_str)
    except Exception:
        pass
    env_after = dict(os.environ)
    assert env_before == env_after, (
        f"Environment was modified while processing payload {repr(payload_str)}"
    )