//! Pure no_std SHA-256 (FIPS 180-4) and HMAC-SHA256 (RFC 2104).
//!
//! Zero external dependencies. Verified against NIST test vectors.

/// SHA-256 digest size in bytes.
pub const DIGEST_SIZE: usize = 32;

/// SHA-256 block size in bytes.
pub const BLOCK_SIZE: usize = 64;

/// Round constants: first 32 bits of fractional parts of cube roots of first 64 primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Initial hash values: first 32 bits of fractional parts of square roots of first 8 primes.
const H_INIT: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Streaming SHA-256 hasher.
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Sha256 {
    /// Create a new SHA-256 hasher.
    pub fn new() -> Self {
        Self {
            state: H_INIT,
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    /// Feed data into the hasher.
    pub fn update(&mut self, data: &[u8]) {
        self.total_len += data.len() as u64;
        let mut offset = 0;

        // Fill partial buffer.
        if self.buffer_len > 0 {
            let need = 64 - self.buffer_len;
            let take = if need < data.len() { need } else { data.len() };
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
            self.buffer_len += take;
            offset = take;
            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }

        // Process full blocks directly.
        while offset + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[offset..offset + 64]);
            self.compress(&block);
            offset += 64;
        }

        // Buffer remaining.
        let remaining = data.len() - offset;
        if remaining > 0 {
            self.buffer[..remaining].copy_from_slice(&data[offset..]);
            self.buffer_len = remaining;
        }
    }

    /// Finalize and return the 32-byte digest.
    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;

        // Append 0x80 padding byte.
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        // If no room for 8-byte length, process block and start new one.
        if self.buffer_len > 56 {
            while self.buffer_len < 64 {
                self.buffer[self.buffer_len] = 0;
                self.buffer_len += 1;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffer = [0u8; 64];
            self.buffer_len = 0;
        }

        // Zero-fill up to byte 56.
        while self.buffer_len < 56 {
            self.buffer[self.buffer_len] = 0;
            self.buffer_len += 1;
        }

        // Append bit length as big-endian u64.
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        // Produce output.
        let mut out = [0u8; 32];
        for i in 0..8 {
            out[i * 4..(i + 1) * 4].copy_from_slice(&self.state[i].to_be_bytes());
        }
        out
    }

    /// Process a single 64-byte block.
    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];

        // First 16 words from the block (big-endian).
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }

        // Extend to 64 words.
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        // Initialize working variables.
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;

        // 64 rounds.
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        // Update state.
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// One-shot SHA-256 hash.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
}

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    // If key > block size, hash it.
    let key_hash: [u8; 32];
    let k: &[u8] = if key.len() > BLOCK_SIZE {
        key_hash = sha256(key);
        &key_hash
    } else {
        key
    };

    // Pad key to block size.
    let mut k_pad = [0u8; BLOCK_SIZE];
    k_pad[..k.len()].copy_from_slice(k);

    // Inner hash: H((K ⊕ ipad) || message)
    let mut inner_key = [0u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        inner_key[i] = k_pad[i] ^ 0x36;
    }
    let mut hasher = Sha256::new();
    hasher.update(&inner_key);
    hasher.update(message);
    let inner_hash = hasher.finalize();

    // Outer hash: H((K ⊕ opad) || inner_hash)
    let mut outer_key = [0u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        outer_key[i] = k_pad[i] ^ 0x5c;
    }
    let mut hasher = Sha256::new();
    hasher.update(&outer_key);
    hasher.update(&inner_hash);
    hasher.finalize()
}

/// Constant-time comparison of two 32-byte digests.
pub fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a hex string to a 32-byte array.
    fn hex32(hex: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }

    // --- NIST SHA-256 Test Vectors (FIPS 180-4 examples) ---

    #[test]
    fn sha256_empty() {
        let expected = hex32("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256(b""), expected);
    }

    #[test]
    fn sha256_abc() {
        let expected = hex32("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(sha256(b"abc"), expected);
    }

    #[test]
    fn sha256_two_block() {
        // "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq" — 56 bytes, spans two blocks.
        let expected = hex32("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            expected
        );
    }

    #[test]
    fn sha256_streaming_matches_oneshot() {
        let data = b"The quick brown fox jumps over the lazy dog";
        let expected = sha256(data);

        // Feed in small chunks.
        let mut h = Sha256::new();
        h.update(&data[..10]);
        h.update(&data[10..30]);
        h.update(&data[30..]);
        assert_eq!(h.finalize(), expected);
    }

    #[test]
    fn sha256_exactly_64_bytes() {
        let data = [0x42u8; 64]; // Exactly one block.
        let result = sha256(&data);
        // Just verify it produces a valid digest (no panic).
        assert_ne!(result, [0u8; 32]);
    }

    #[test]
    fn sha256_128_bytes() {
        let data = [0xAB; 128]; // Exactly two blocks.
        let result = sha256(&data);
        assert_ne!(result, [0u8; 32]);
    }

    // --- HMAC-SHA256 Test Vector (RFC 4231 Test Case 2) ---

    #[test]
    fn hmac_sha256_rfc4231_case2() {
        // Key = "Jefe", Data = "what do ya want for nothing?"
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let expected = hex32("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        assert_eq!(hmac_sha256(key, data), expected);
    }

    #[test]
    fn hmac_sha256_rfc4231_case1() {
        // Key = 20 bytes of 0x0b, Data = "Hi There"
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let expected = hex32("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
        assert_eq!(hmac_sha256(&key, data), expected);
    }

    #[test]
    fn hmac_sha256_long_key() {
        // Key longer than block size (131 bytes of 0xaa).
        let key = [0xAAu8; 131];
        let data = b"Test Using Larger Than Block-Size Key - Hash Key First";
        let expected = hex32("60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
        assert_eq!(hmac_sha256(&key, data), expected);
    }

    #[test]
    fn ct_eq_same() {
        let a = sha256(b"test");
        assert!(ct_eq(&a, &a));
    }

    #[test]
    fn ct_eq_different() {
        let a = sha256(b"test1");
        let b = sha256(b"test2");
        assert!(!ct_eq(&a, &b));
    }
}
