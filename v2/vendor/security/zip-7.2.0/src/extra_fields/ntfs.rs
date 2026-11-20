use std::io::Read;

use crate::{
    result::{ZipError, ZipResult},
    unstable::LittleEndianReadExt,
};

/// The NTFS extra field as described in [PKWARE's APPNOTE.TXT v6.3.9].
///
/// This field stores [Windows file times], which are 64-bit unsigned integer
/// values that represents the number of 100-nanosecond intervals that have
/// elapsed since "1601-01-01 00:00:00 UTC".
///
/// [PKWARE's APPNOTE.TXT v6.3.9]: https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
/// [Windows file times]: https://docs.microsoft.com/en-us/windows/win32/sysinfo/file-times
#[derive(Clone, Debug)]
pub struct Ntfs {
    mtime: u64,
    atime: u64,
    ctime: u64,
}

impl Ntfs {
    /// Creates a NTFS extra field struct by reading the required bytes from the
    /// reader.
    ///
    /// This method assumes that the length has already been read, therefore it
    /// must be passed as an argument.
    pub fn try_from_reader<R>(reader: &mut R, len: u16) -> ZipResult<Self>
    where
        R: Read,
    {
        if len != 32 {
            return Err(ZipError::UnsupportedArchive(
                "NTFS extra field has an unsupported length",
            ));
        }

        // Read reserved for future use.
        let _ = reader.read_u32_le()?;

        let tag = reader.read_u16_le()?;
        if tag != 0x0001 {
            return Err(ZipError::UnsupportedArchive(
                "NTFS extra field has an unsupported attribute tag",
            ));
        }
        let size = reader.read_u16_le()?;
        if size != 24 {
            return Err(ZipError::UnsupportedArchive(
                "NTFS extra field has an unsupported attribute size",
            ));
        }

        let mtime = reader.read_u64_le()?;
        let atime = reader.read_u64_le()?;
        let ctime = reader.read_u64_le()?;
        Ok(Self {
            mtime,
            atime,
            ctime,
        })
    }

    /// Returns the file last modification time as a file time.
    pub fn mtime(&self) -> u64 {
        self.mtime
    }

    /// Returns the file last modification time as a file time.
    #[cfg(feature = "nt-time")]
    pub fn modified_file_time(&self) -> nt_time::FileTime {
        nt_time::FileTime::new(self.mtime)
    }

    /// Returns the file last access time as a file time.
    pub fn atime(&self) -> u64 {
        self.atime
    }

    /// Returns the file last access time as a file time.
    #[cfg(feature = "nt-time")]
    pub fn accessed_file_time(&self) -> nt_time::FileTime {
        nt_time::FileTime::new(self.atime)
    }

    /// Returns the file creation time as a file time.
    pub fn ctime(&self) -> u64 {
        self.ctime
    }

    /// Returns the file creation time as a file time.
    #[cfg(feature = "nt-time")]
    pub fn created_file_time(&self) -> nt_time::FileTime {
        nt_time::FileTime::new(self.ctime)
    }
}
