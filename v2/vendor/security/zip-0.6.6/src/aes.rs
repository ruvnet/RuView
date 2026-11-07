//! Implementation of the AES decryption for zip files.
//!
//! This was implemented according to the [WinZip specification](https://www.winzip.com/win/en/aes_info.html).
//! Note that using CRC with AES depends on the used encryption specification, AE-1 or AE-2.
//! If the file is marked as encrypted with AE-2 the CRC field is ignored, even if it isn't set to 0.

use crate::aes_ctr;
use crate::types::AesMode;
use constant_time_eq::constant_time_eq;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::io::{self, Read};

/// The length of the password verifcation value in bytes
const PWD_VERIFY_LENGTH: usize = 2;
/// The length of the authentication code in bytes
const AUTH_CODE_LENGTH: usize = 10;
/// The number of iterations used with PBKDF2
const ITERATION_COUNT: u32 = 1000;

/// Create a AesCipher depending on the used `AesMode` and the given `key`.
///
/// # Panics
///
/// This panics if `key` doesn't have the correct size for the chosen aes mode.
fn cipher_from_mode(aes_mode: AesMode, key: &[u8]) -> Box<dyn aes_ctr::AesCipher> {
    match aes_mode {
        AesMode::Aes128 => Box::new(aes_ctr::AesCtrZipKeyStream::<aes_ctr::Aes128>::new(key))
            as Box<dyn aes_ctr::AesCipher>,
        AesMode::Aes192 => Box::new(aes_ctr::AesCtrZipKeyStream::<aes_ctr::Aes192>::new(key))
            as Box<dyn aes_ctr::AesCipher>,
        AesMode::Aes256 => Box::new(aes_ctr::AesCtrZipKeyStream::<aes_ctr::Aes256>::new(key))
            as Box<dyn aes_ctr::AesCipher>,
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
    pub fn new(reader: R, aes_mode: AesMode, compressed_size: u64) -> AesReader<R> {
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
    ///
    /// # Returns
    ///
    /// If the password verification failed `Ok(None)` will be returned to match the validate
    /// method of ZipCryptoReader.
    pub fn validate(mut self, password: &[u8]) -> io::Result<Option<AesReaderValid<R>>> {
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
        let mut derived_key: Vec<u8> = vec![0; derived_key_len];

        // use PBKDF2 with HMAC-Sha1 to derive the key
        pbkdf2::pbkdf2::<Hmac<Sha1>>(password, &salt, ITERATION_COUNT, &mut derived_key);
        let decrypt_key = &derived_key[0..key_length];
        let hmac_key = &derived_key[key_length..key_length * 2];
        let pwd_verify = &derived_key[derived_key_len - 2..];

        // the last 2 bytes should equal the password verification value
        if pwd_verification_value != pwd_verify {
            // wrong password
            return Ok(None);
        }

        let cipher = cipher_from_mode(self.aes_mode, decrypt_key);
        let hmac = Hmac::<Sha1>::new_from_slice(hmac_key).unwrap();

        Ok(Some(AesReaderValid {
            reader: self.reader,
            data_remaining: self.data_length,
            cipher,
            hmac,
            finalized: false,
        }))
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
    cipher: Box<dyn aes_ctr::AesCipher>,
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
                    io::Error::new(
                        io::ErrorKind::InvalidData,
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
