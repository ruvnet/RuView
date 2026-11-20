//! A counter mode (CTR) for AES to work with the encryption used in zip files.
//!
//! This was implemented since the zip specification requires the mode to not use a nonce and uses a
//! different byte order (little endian) than NIST (big endian).
//! See [AesCtrZipKeyStream] for more information.

use aes::cipher::generic_array::GenericArray;
// use aes::{BlockEncrypt, NewBlockCipher};
use aes::cipher::{BlockEncrypt, KeyInit};
use byteorder::WriteBytesExt;
use std::{any, fmt};

/// Internal block size of an AES cipher.
const AES_BLOCK_SIZE: usize = 16;

/// AES-128.
#[derive(Debug)]
pub struct Aes128;
/// AES-192
#[derive(Debug)]
pub struct Aes192;
/// AES-256.
#[derive(Debug)]
pub struct Aes256;

/// An AES cipher kind.
pub trait AesKind {
    /// Key type.
    type Key: AsRef<[u8]>;
    /// Cipher used to decrypt.
    type Cipher;
}

impl AesKind for Aes128 {
    type Key = [u8; 16];
    type Cipher = aes::Aes128;
}

impl AesKind for Aes192 {
    type Key = [u8; 24];
    type Cipher = aes::Aes192;
}

impl AesKind for Aes256 {
    type Key = [u8; 32];
    type Cipher = aes::Aes256;
}

/// An AES-CTR key stream generator.
///
/// Implements the slightly non-standard AES-CTR variant used by WinZip AES encryption.
///
/// Typical AES-CTR implementations combine a nonce with a 64 bit counter. WinZIP AES instead uses
/// no nonce and also uses a different byte order (little endian) than NIST (big endian).
///
/// The stream implements the `Read` trait; encryption or decryption is performed by XOR-ing the
/// bytes from the key stream with the ciphertext/plaintext.
pub struct AesCtrZipKeyStream<C: AesKind> {
    /// Current AES counter.
    counter: u128,
    /// AES cipher instance.
    cipher: C::Cipher,
    /// Stores the currently available keystream bytes.
    buffer: [u8; AES_BLOCK_SIZE],
    /// Number of bytes already used up from `buffer`.
    pos: usize,
}

impl<C> fmt::Debug for AesCtrZipKeyStream<C>
where
    C: AesKind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AesCtrZipKeyStream<{}>(counter: {})",
            any::type_name::<C>(),
            self.counter
        )
    }
}

impl<C> AesCtrZipKeyStream<C>
where
    C: AesKind,
    C::Cipher: KeyInit,
{
    /// Creates a new zip variant AES-CTR key stream.
    ///
    /// # Panics
    ///
    /// This panics if `key` doesn't have the correct size for cipher `C`.
    pub fn new(key: &[u8]) -> AesCtrZipKeyStream<C> {
        AesCtrZipKeyStream {
            counter: 1,
            cipher: C::Cipher::new(GenericArray::from_slice(key)),
            buffer: [0u8; AES_BLOCK_SIZE],
            pos: AES_BLOCK_SIZE,
        }
    }
}

impl<C> AesCipher for AesCtrZipKeyStream<C>
where
    C: AesKind,
    C::Cipher: BlockEncrypt,
{
    /// Decrypt or encrypt `target`.
    #[inline]
    fn crypt_in_place(&mut self, mut target: &mut [u8]) {
        while !target.is_empty() {
            if self.pos == AES_BLOCK_SIZE {
                // Note: AES block size is always 16 bytes, same as u128.
                self.buffer
                    .as_mut()
                    .write_u128::<byteorder::LittleEndian>(self.counter)
                    .expect("did not expect u128 le conversion to fail");
                self.cipher
                    .encrypt_block(GenericArray::from_mut_slice(&mut self.buffer));
                self.counter += 1;
                self.pos = 0;
            }

            let target_len = target.len().min(AES_BLOCK_SIZE - self.pos);

            xor(
                &mut target[0..target_len],
                &self.buffer[self.pos..(self.pos + target_len)],
            );
            target = &mut target[target_len..];
            self.pos += target_len;
        }
    }
}

/// This trait allows using generic AES ciphers with different key sizes.
pub trait AesCipher {
    fn crypt_in_place(&mut self, target: &mut [u8]);
}

/// XORs a slice in place with another slice.
#[inline]
fn xor(dest: &mut [u8], src: &[u8]) {
    assert_eq!(dest.len(), src.len());

    for (lhs, rhs) in dest.iter_mut().zip(src.iter()) {
        *lhs ^= *rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::{Aes128, Aes192, Aes256, AesCipher, AesCtrZipKeyStream, AesKind};
    use aes::cipher::{BlockEncrypt, KeyInit};

    /// Checks whether `crypt_in_place` produces the correct plaintext after one use and yields the
    /// cipertext again after applying it again.
    fn roundtrip<Aes>(key: &[u8], ciphertext: &mut [u8], expected_plaintext: &[u8])
    where
        Aes: AesKind,
        Aes::Cipher: KeyInit + BlockEncrypt,
    {
        let mut key_stream = AesCtrZipKeyStream::<Aes>::new(key);

        let mut plaintext: Vec<u8> = ciphertext.to_vec();
        key_stream.crypt_in_place(plaintext.as_mut_slice());
        assert_eq!(plaintext, expected_plaintext.to_vec());

        // Round-tripping should yield the ciphertext again.
        let mut key_stream = AesCtrZipKeyStream::<Aes>::new(key);
        key_stream.crypt_in_place(&mut plaintext);
        assert_eq!(plaintext, ciphertext.to_vec());
    }

    #[test]
    #[should_panic]
    fn new_with_wrong_key_size() {
        AesCtrZipKeyStream::<Aes128>::new(&[1, 2, 3, 4, 5]);
    }

    // The data used in these tests was generated with p7zip without any compression.
    // It's not possible to recreate the exact same data, since a random salt is used for encryption.
    // `7z a -phelloworld -mem=AES256 -mx=0 aes256_40byte.zip 40byte_data.txt`
    #[test]
    fn crypt_aes_256_0_byte() {
        let mut ciphertext = [];
        let expected_plaintext = &[];
        let key = [
            0x0b, 0xec, 0x2e, 0xf2, 0x46, 0xf0, 0x7e, 0x35, 0x16, 0x54, 0xe0, 0x98, 0x10, 0xb3,
            0x18, 0x55, 0x24, 0xa3, 0x9e, 0x0e, 0x40, 0xe7, 0x92, 0xad, 0xb2, 0x8a, 0x48, 0xf4,
            0x5c, 0xd0, 0xc0, 0x54,
        ];

        roundtrip::<Aes256>(&key, &mut ciphertext, expected_plaintext);
    }

    #[test]
    fn crypt_aes_128_5_byte() {
        let mut ciphertext = [0x98, 0xa9, 0x8c, 0x26, 0x0e];
        let expected_plaintext = b"asdf\n";
        let key = [
            0xe0, 0x25, 0x7b, 0x57, 0x97, 0x6a, 0xa4, 0x23, 0xab, 0x94, 0xaa, 0x44, 0xfd, 0x47,
            0x4f, 0xa5,
        ];

        roundtrip::<Aes128>(&key, &mut ciphertext, expected_plaintext);
    }

    #[test]
    fn crypt_aes_192_5_byte() {
        let mut ciphertext = [0x36, 0x55, 0x5c, 0x61, 0x3c];
        let expected_plaintext = b"asdf\n";
        let key = [
            0xe4, 0x4a, 0x88, 0x52, 0x8f, 0xf7, 0x0b, 0x81, 0x7b, 0x75, 0xf1, 0x74, 0x21, 0x37,
            0x8c, 0x90, 0xad, 0xbe, 0x4a, 0x65, 0xa8, 0x96, 0x0e, 0xcc,
        ];

        roundtrip::<Aes192>(&key, &mut ciphertext, expected_plaintext);
    }

    #[test]
    fn crypt_aes_256_5_byte() {
        let mut ciphertext = [0xc2, 0x47, 0xc0, 0xdc, 0x56];
        let expected_plaintext = b"asdf\n";
        let key = [
            0x79, 0x5e, 0x17, 0xf2, 0xc6, 0x3d, 0x28, 0x9b, 0x4b, 0x4b, 0xbb, 0xa9, 0xba, 0xc9,
            0xa5, 0xee, 0x3a, 0x4f, 0x0f, 0x4b, 0x29, 0xbd, 0xe9, 0xb8, 0x41, 0x9c, 0x41, 0xa5,
            0x15, 0xb2, 0x86, 0xab,
        ];

        roundtrip::<Aes256>(&key, &mut ciphertext, expected_plaintext);
    }

    #[test]
    fn crypt_aes_128_40_byte() {
        let mut ciphertext = [
            0xcf, 0x72, 0x6b, 0xa1, 0xb2, 0x0f, 0xdf, 0xaa, 0x10, 0xad, 0x9c, 0x7f, 0x6d, 0x1c,
            0x8d, 0xb5, 0x16, 0x7e, 0xbb, 0x11, 0x69, 0x52, 0x8c, 0x89, 0x80, 0x32, 0xaa, 0x76,
            0xa6, 0x18, 0x31, 0x98, 0xee, 0xdd, 0x22, 0x68, 0xb7, 0xe6, 0x77, 0xd2,
        ];
        let expected_plaintext = b"Lorem ipsum dolor sit amet, consectetur\n";
        let key = [
            0x43, 0x2b, 0x6d, 0xbe, 0x05, 0x76, 0x6c, 0x9e, 0xde, 0xca, 0x3b, 0xf8, 0xaf, 0x5d,
            0x81, 0xb6,
        ];

        roundtrip::<Aes128>(&key, &mut ciphertext, expected_plaintext);
    }

    #[test]
    fn crypt_aes_192_40_byte() {
        let mut ciphertext = [
            0xa6, 0xfc, 0x52, 0x79, 0x2c, 0x6c, 0xfe, 0x68, 0xb1, 0xa8, 0xb3, 0x07, 0x52, 0x8b,
            0x82, 0xa6, 0x87, 0x9c, 0x72, 0x42, 0x3a, 0xf8, 0xc6, 0xa9, 0xc9, 0xfb, 0x61, 0x19,
            0x37, 0xb9, 0x56, 0x62, 0xf4, 0xfc, 0x5e, 0x7a, 0xdd, 0x55, 0x0a, 0x48,
        ];
        let expected_plaintext = b"Lorem ipsum dolor sit amet, consectetur\n";
        let key = [
            0xac, 0x92, 0x41, 0xba, 0xde, 0xd9, 0x02, 0xfe, 0x40, 0x92, 0x20, 0xf6, 0x56, 0x03,
            0xfe, 0xae, 0x1b, 0xba, 0x01, 0x97, 0x97, 0x79, 0xbb, 0xa6,
        ];

        roundtrip::<Aes192>(&key, &mut ciphertext, expected_plaintext);
    }

    #[test]
    fn crypt_aes_256_40_byte() {
        let mut ciphertext = [
            0xa9, 0x99, 0xbd, 0xea, 0x82, 0x9b, 0x8f, 0x2f, 0xb7, 0x52, 0x2f, 0x6b, 0xd8, 0xf6,
            0xab, 0x0e, 0x24, 0x51, 0x9e, 0x18, 0x0f, 0xc0, 0x8f, 0x54, 0x15, 0x80, 0xae, 0xbc,
            0xa0, 0x5c, 0x8a, 0x11, 0x8d, 0x14, 0x7e, 0xc5, 0xb4, 0xae, 0xd3, 0x37,
        ];
        let expected_plaintext = b"Lorem ipsum dolor sit amet, consectetur\n";
        let key = [
            0x64, 0x7c, 0x7a, 0xde, 0xf0, 0xf2, 0x61, 0x49, 0x1c, 0xf1, 0xf1, 0xe3, 0x37, 0xfc,
            0xe1, 0x4d, 0x4a, 0x77, 0xd4, 0xeb, 0x9e, 0x3d, 0x75, 0xce, 0x9a, 0x3e, 0x10, 0x50,
            0xc2, 0x07, 0x36, 0xb6,
        ];

        roundtrip::<Aes256>(&key, &mut ciphertext, expected_plaintext);
    }
}
