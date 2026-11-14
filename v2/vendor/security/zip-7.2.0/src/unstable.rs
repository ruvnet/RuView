#![allow(missing_docs)]

use std::borrow::Cow;
use std::io;
use std::io::{Read, Write};
use std::path::{Component, Path, MAIN_SEPARATOR};

/// Provides high level API for reading from a stream.
pub mod stream {
    pub use crate::read::stream::*;
}
/// Types for creating ZIP archives.
pub mod write {
    use crate::write::{FileOptionExtension, FileOptions};
    /// Unstable methods for [`FileOptions`].
    pub trait FileOptionsExt {
        /// Write the file with the given password using the deprecated ZipCrypto algorithm.
        ///
        /// This is not recommended for new archives, as ZipCrypto is not secure.
        fn with_deprecated_encryption(self, password: &[u8]) -> Self;
    }
    impl<T: FileOptionExtension> FileOptionsExt for FileOptions<'_, T> {
        fn with_deprecated_encryption(self, password: &[u8]) -> FileOptions<'static, T> {
            self.with_deprecated_encryption(password)
        }
    }
}

/// Helper methods for writing unsigned integers in little-endian form.
pub trait LittleEndianWriteExt: Write {
    fn write_u16_le(&mut self, input: u16) -> io::Result<()> {
        self.write_all(&input.to_le_bytes())
    }

    fn write_u32_le(&mut self, input: u32) -> io::Result<()> {
        self.write_all(&input.to_le_bytes())
    }

    fn write_u64_le(&mut self, input: u64) -> io::Result<()> {
        self.write_all(&input.to_le_bytes())
    }

    fn write_u128_le(&mut self, input: u128) -> io::Result<()> {
        self.write_all(&input.to_le_bytes())
    }
}

impl<W: Write + ?Sized> LittleEndianWriteExt for W {}

/// Helper methods for reading unsigned integers in little-endian form.
pub trait LittleEndianReadExt: Read {
    fn read_u16_le(&mut self) -> io::Result<u16> {
        let mut out = [0u8; 2];
        self.read_exact(&mut out)?;
        Ok(u16::from_le_bytes(out))
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        let mut out = [0u8; 4];
        self.read_exact(&mut out)?;
        Ok(u32::from_le_bytes(out))
    }

    fn read_u64_le(&mut self) -> io::Result<u64> {
        let mut out = [0u8; 8];
        self.read_exact(&mut out)?;
        Ok(u64::from_le_bytes(out))
    }
}

impl<R: Read> LittleEndianReadExt for R {}

/// Converts a path to the ZIP format (forward-slash-delimited and normalized).
pub fn path_to_string<T: AsRef<Path>>(path: T) -> Box<str> {
    let mut maybe_original = None;
    if let Some(original) = path.as_ref().to_str() {
        if original.is_empty() || original == "." || original == ".." {
            return String::new().into_boxed_str();
        }
        if original.starts_with(MAIN_SEPARATOR) {
            if original.len() == 1 {
                return MAIN_SEPARATOR.to_string().into_boxed_str();
            } else if (MAIN_SEPARATOR == '/' || !original[1..].contains(MAIN_SEPARATOR))
                && !original.ends_with('.')
                && !original.contains([MAIN_SEPARATOR, MAIN_SEPARATOR])
                && !original.contains([MAIN_SEPARATOR, '.', MAIN_SEPARATOR])
                && !original.contains([MAIN_SEPARATOR, '.', '.', MAIN_SEPARATOR])
            {
                maybe_original = Some(&original[1..]);
            }
        } else if !original.contains(MAIN_SEPARATOR) {
            return original.into();
        }
    }
    let mut recreate = maybe_original.is_none();
    let mut normalized_components = Vec::new();

    for component in path.as_ref().components() {
        match component {
            Component::Normal(os_str) => match os_str.to_str() {
                Some(valid_str) => normalized_components.push(Cow::Borrowed(valid_str)),
                None => {
                    recreate = true;
                    normalized_components.push(os_str.to_string_lossy());
                }
            },
            Component::ParentDir => {
                recreate = true;
                normalized_components.pop();
            }
            _ => {
                recreate = true;
            }
        }
    }
    if recreate {
        normalized_components.join("/").into()
    } else {
        maybe_original.unwrap().into()
    }
}
