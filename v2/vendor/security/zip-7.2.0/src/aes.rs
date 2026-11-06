//! Implementation of the AES decryption for zip files.
//!
//! This was implemented according to the [WinZip specification](https://www.winzip.com/win/en/aes_info.html).
//! Note that using CRC with AES depends on the used encryption specification, AE-1 or AE-2.
//! If the file is marked as encrypted with AE-2 the CRC field is ignored, even if it isn't set to 0.

use crate::aes_ctr::AesCipher;
use crate::result::ZipResult;
use crate::types::AesMode;
use crate::{aes_ctr, result::ZipError};
use constant_time_eq::constant_time_eq;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::io::{self, Error, ErrorKind, Read, Write};
use zeroize::{Zeroize, Zeroizing};

/// The length of the password verification value in bytes
pub const PWD_VERIFY_LENGTH: usize = 2;
/// The length of the authentication code in bytes
const AUTH_CODE_LENGTH: usize = 10;
/// The number of iterations used with PBKDF2
const ITERATION_COUNT: u32 = 1000;

enum Cipher {
    Aes128(Box<aes_ctr::AesCtrZipKeyStream<aes_ctr::Aes128>>),
    Aes192(Box<aes_ctr::AesCtrZipKeyStream<aes_ctr::Aes192>>),
    Aes256(Box<aes_ctr::AesCtrZipKeyStream<aes_ctr::Aes256>>),
}

impl Cipher {
    /// Create a `Cipher` depending on the used `AesMode` and the given `key`.
    ///
    /// Returns `ZipError::InvalidPassword` if `key` doesn't have the correct size for the chosen
    /// AES mode.
    fn from_mode(aes_mode: AesMode, key: &[u8]) -> ZipResult<Self> {
        Ok(match aes_mode {
            AesMode::Aes128 => Cipher::Aes128(Box::new(aes_ctr::AesCtrZipKeyStream::<
                aes_ctr::Aes128,
            >::new(key)?)),
            AesMode::Aes192 => Cipher::Aes192(Box::new(aes_ctr::AesCtrZipKeyStream::<
                aes_ctr::Aes192,
            >::new(key)?)),
            AesMode::Aes256 => Cipher::Aes256(Box::new(aes_ctr::AesCtrZipKeyStream::<
                aes_ctr::Aes256,
            >::new(key)?)),
        })
    }

    fn crypt_in_place(&mut self, target: &mut [u8]) {
        match self {
            Self::Aes128(cipher) => cipher.crypt_in_place(target),
            Self::Aes192(cipher) => cipher.crypt_in_place(target),
            Self::Aes256(cipher) => cipher.crypt_in_place(target),
        }
    }
}

// An aes encrypted file starts with a salt, whose length depends on the used aes mode
// followed by a 2 byte password verification value
// then the variable length encrypted data
// and lastly a 10 byte authentication code
pub struct AesReader<R> {
    reader: R,
    aes_mode: AesMode,
    data_length: u64,
}

impl<R: Read> AesReader<R> {
    pub const fn new(reader: R, aes_mode: AesMode, compressed_size: u64) -> AesReader<R> {
        let data_length = compressed_size
            - (PWD_VERIFY_LENGTH + AUTH_CODE_LENGTH + aes_mode.salt_length()) as u64;

        Self {
            reader,
            aes_mode,
            data_length,
        }
    }

    /// Read the AES header bytes and validate the password.
    ///
    /// Even if the validation succeeds, there is still a 1 in 65536 chance that an incorrect
    /// password was provided.
    /// It isn't possible to check the authentication code in this step. This will be done after
    /// reading and decrypting the file.
    pub fn validate(mut self, password: &[u8]) -> Result<AesReaderValid<R>, ZipError> {
        let salt_length = self.aes_mode.salt_length();
        let key_length = self.aes_mode.key_length();

        let mut salt = vec![0; salt_length];
        self.reader.read_exact(&mut salt)?;

        // next are 2 bytes used for password verification
        let mut pwd_verification_value = vec![0; PWD_VERIFY_LENGTH];
        self.reader.read_exact(&mut pwd_verification_value)?;

        // derive a key from the password and salt
        // the length depends on the aes key length
        let derived_key_len = 2 * key_length + PWD_VERIFY_LENGTH;
        let mut derived_key: Box<[u8]> = vec![0; derived_key_len].into_boxed_slice();

        // use PBKDF2 with HMAC-Sha1 to derive the key
        pbkdf2::pbkdf2::<Hmac<Sha1>>(password, &salt, ITERATION_COUNT, &mut derived_key)
            .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
        let decrypt_key = &derived_key[0..key_length];
        let hmac_key = &derived_key[key_length..key_length * 2];
        let pwd_verify = &derived_key[derived_key_len - 2..];

        // the last 2 bytes should equal the password verification value
        if pwd_verification_value != pwd_verify {
            // wrong password
            return Err(ZipError::InvalidPassword);
        }

        let cipher = Cipher::from_mode(self.aes_mode, decrypt_key)?;
        let hmac = Hmac::<Sha1>::new_from_slice(hmac_key).unwrap();

        Ok(AesReaderValid {
            reader: self.reader,
            data_remaining: self.data_length,
            cipher,
            hmac,
            finalized: false,
        })
    }

    /// Read the AES header bytes and returns the verification value and salt.
    ///
    /// # Returns
    ///
    /// the verification value and the salt
    pub fn get_verification_value_and_salt(
        mut self,
    ) -> io::Result<([u8; PWD_VERIFY_LENGTH], Vec<u8>)> {
        let salt_length = self.aes_mode.salt_length();

        let mut salt = vec![0; salt_length];
        self.reader.read_exact(&mut salt)?;

        // next are 2 bytes used for password verification
        let mut pwd_verification_value = [0; PWD_VERIFY_LENGTH];
        self.reader.read_exact(&mut pwd_verification_value)?;
        Ok((pwd_verification_value, salt))
    }
}

/// A reader for aes encrypted files, which has already passed the first password check.
///
/// There is a 1 in 65536 chance that an invalid password passes that check.
/// After the data has been read and decrypted an HMAC will be checked and provide a final means
/// to check if either the password is invalid or if the data has been changed.
pub struct AesReaderValid<R: Read> {
    reader: R,
    data_remaining: u64,
    cipher: Cipher,
    hmac: Hmac<Sha1>,
    finalized: bool,
}

impl<R: Read> Read for AesReaderValid<R> {
    /// This implementation does not fulfill all requirements set in the trait documentation.
    ///
    /// ```txt
    /// "If an error is returned then it must be guaranteed that no bytes were read."
    /// ```
    ///
    /// Whether this applies to errors that occur while reading the encrypted data depends on the
    /// underlying reader. If the error occurs while verifying the HMAC, the reader might become
    /// practically unusable, since its position after the error is not known.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.data_remaining == 0 {
            return Ok(0);
        }

        // get the number of bytes to read, compare as u64 to make sure we can read more than
        // 2^32 bytes even on 32 bit systems.
        let bytes_to_read = self.data_remaining.min(buf.len() as u64) as usize;
        let read = self.reader.read(&mut buf[0..bytes_to_read])?;
        self.data_remaining -= read as u64;

        // Update the hmac with the encrypted data
        self.hmac.update(&buf[0..read]);

        // decrypt the data
        self.cipher.crypt_in_place(&mut buf[0..read]);

        // if there is no data left to read, check the integrity of the data
        if self.data_remaining == 0 {
            assert!(
                !self.finalized,
                "Tried to use an already finalized HMAC. This is a bug!"
            );
            self.finalized = true;

            // Zip uses HMAC-Sha1-80, which only uses the first half of the hash
            // see https://www.winzip.com/win/en/aes_info.html#auth-faq
            let mut read_auth_code = [0; AUTH_CODE_LENGTH];
            self.reader.read_exact(&mut read_auth_code)?;
            let computed_auth_code = &self.hmac.finalize_reset().into_bytes()[0..AUTH_CODE_LENGTH];

            // use constant time comparison to mitigate timing attacks
            if !constant_time_eq(computed_auth_code, &read_auth_code) {
                return Err(
                    Error::new(
                        ErrorKind::InvalidData,
                        "Invalid authentication code, this could be due to an invalid password or errors in the data"
                    )
                );
            }
        }

        Ok(read)
    }
}

impl<R: Read> AesReaderValid<R> {
    /// Consumes this decoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

pub struct AesWriter<W> {
    writer: W,
    cipher: Cipher,
    hmac: Hmac<Sha1>,
    buffer: Zeroizing<Vec<u8>>,
    encrypted_file_header: Option<Vec<u8>>,
}

impl<W: Write> AesWriter<W> {
    pub fn new(writer: W, aes_mode: AesMode, password: &[u8]) -> ZipResult<Self> {
        let salt_length = aes_mode.salt_length();
        let key_length = aes_mode.key_length();

        let mut encrypted_file_header = Vec::with_capacity(salt_length + 2);

        let mut salt = vec![0; salt_length];
        getrandom::fill(&mut salt).map_err(|e| ZipError::Io(e.into()))?;
        encrypted_file_header.write_all(&salt)?;

        // Derive a key from the password and salt.  The length depends on the aes key length
        let derived_key_len = 2 * key_length + PWD_VERIFY_LENGTH;
        let mut derived_key: Zeroizing<Vec<u8>> = Zeroizing::new(vec![0; derived_key_len]);

        // Use PBKDF2 with HMAC-Sha1 to derive the key.
        pbkdf2::pbkdf2::<Hmac<Sha1>>(password, &salt, ITERATION_COUNT, &mut derived_key)
            .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
        let encryption_key = &derived_key[0..key_length];
        let hmac_key = &derived_key[key_length..key_length * 2];

        let pwd_verify = derived_key[derived_key_len - 2..].to_vec();
        encrypted_file_header.write_all(&pwd_verify)?;

        let cipher = Cipher::from_mode(aes_mode, encryption_key)?;
        let hmac = Hmac::<Sha1>::new_from_slice(hmac_key).unwrap();

        Ok(Self {
            writer,
            cipher,
            hmac,
            buffer: Default::default(),
            encrypted_file_header: Some(encrypted_file_header),
        })
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.write_encrypted_file_header()?;

        // Zip uses HMAC-Sha1-80, which only uses the first half of the hash
        // see https://www.winzip.com/win/en/aes_info.html#auth-faq
        let computed_auth_code = &self.hmac.finalize_reset().into_bytes()[0..AUTH_CODE_LENGTH];
        self.writer.write_all(computed_auth_code)?;

        Ok(self.writer)
    }

    /// The AES encryption specification requires some metadata being written at the start of the
    /// file data section, but this can only be done once the extra data writing has been finished
    /// so we can't do it when the writer is constructed.
    fn write_encrypted_file_header(&mut self) -> io::Result<()> {
        if let Some(header) = self.encrypted_file_header.take() {
            self.writer.write_all(&header)?;
        }

        Ok(())
    }
}

impl<W: Write> Write for AesWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_encrypted_file_header()?;

        // Fill the internal buffer and encrypt it in-place.
        self.buffer.extend_from_slice(buf);
        self.cipher.crypt_in_place(&mut self.buffer[..]);

        // Update the hmac with the encrypted data.
        self.hmac.update(&self.buffer[..]);

        // Write the encrypted buffer to the inner writer.  We need to use `write_all` here as if
        // we only write parts of the data we can't easily reverse the keystream in the cipher
        // implementation.
        self.writer.write_all(&self.buffer[..])?;

        // Zeroize the backing memory before clearing the buffer to prevent cleartext data from
        // being left in memory.
        self.buffer.zeroize();
        self.buffer.clear();

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(all(test, feature = "aes-crypto"))]
mod tests {
    use std::io::{self, Read, Write};

    use crate::{
        aes::{AesReader, AesWriter},
        result::ZipError,
        types::AesMode,
    };

    /// Checks whether `AesReader` can successfully decrypt what `AesWriter` produces.
    fn roundtrip(aes_mode: AesMode, password: &[u8], plaintext: &[u8]) -> Result<bool, ZipError> {
        let mut buf = io::Cursor::new(vec![]);
        let mut read_buffer = vec![];

        {
            let mut writer = AesWriter::new(&mut buf, aes_mode, password)?;
            writer.write_all(plaintext)?;
            writer.finish()?;
        }

        // Reset cursor position to the beginning.
        buf.set_position(0);

        {
            let compressed_length = buf.get_ref().len() as u64;
            let mut reader =
                AesReader::new(&mut buf, aes_mode, compressed_length).validate(password)?;
            reader.read_to_end(&mut read_buffer)?;
        }

        Ok(plaintext == read_buffer)
    }

    #[test]
    fn crypt_aes_256_0_byte() {
        let plaintext = &[];
        let password = b"some super secret password";
        assert!(roundtrip(AesMode::Aes256, password, plaintext).expect("could encrypt and decrypt"));
    }

    #[test]
    fn crypt_aes_128_5_byte() {
        let plaintext = b"asdf\n";
        let password = b"some super secret password";

        assert!(roundtrip(AesMode::Aes128, password, plaintext).expect("could encrypt and decrypt"));
    }

    #[test]
    fn crypt_aes_192_5_byte() {
        let plaintext = b"asdf\n";
        let password = b"some super secret password";

        assert!(roundtrip(AesMode::Aes192, password, plaintext).expect("could encrypt and decrypt"));
    }

    #[test]
    fn crypt_aes_256_5_byte() {
        let plaintext = b"asdf\n";
        let password = b"some super secret password";

        assert!(roundtrip(AesMode::Aes256, password, plaintext).expect("could encrypt and decrypt"));
    }

    #[test]
    fn crypt_aes_128_40_byte() {
        let plaintext = b"Lorem ipsum dolor sit amet, consectetur\n";
        let password = b"some super secret password";

        assert!(roundtrip(AesMode::Aes128, password, plaintext).expect("could encrypt and decrypt"));
    }

    #[test]
    fn crypt_aes_192_40_byte() {
        let plaintext = b"Lorem ipsum dolor sit amet, consectetur\n";
        let password = b"some super secret password";

        assert!(roundtrip(AesMode::Aes192, password, plaintext).expect("could encrypt and decrypt"));
    }

    #[test]
    fn crypt_aes_256_40_byte() {
        let plaintext = b"Lorem ipsum dolor sit amet, consectetur\n";
        let password = b"some super secret password";

        assert!(roundtrip(AesMode::Aes256, password, plaintext).expect("could encrypt and decrypt"));
    }
}
