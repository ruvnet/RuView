use crate::result::invalid;
use crate::result::{ZipError, ZipResult};
use crate::unstable::LittleEndianReadExt;
use std::io::Read;

/// extended timestamp, as described in <https://libzip.org/specifications/extrafld.txt>

#[derive(Debug, Clone)]
pub struct ExtendedTimestamp {
    mod_time: Option<u32>,
    ac_time: Option<u32>,
    cr_time: Option<u32>,
}

impl ExtendedTimestamp {
    /// creates an extended timestamp struct by reading the required bytes from the reader.
    ///
    /// This method assumes that the length has already been read, therefore
    /// it must be passed as an argument
    pub fn try_from_reader<R>(reader: &mut R, len: u16) -> ZipResult<Self>
    where
        R: Read,
    {
        if len == 0 {
            return Err(invalid!("Extended timestamp field is empty"));
        }
        let mut flags = [0u8];
        let mut bytes_to_read = len as usize;
        reader.read_exact(&mut flags)?;
        bytes_to_read -= flags.len();
        let flags = flags[0];

        // the `flags` field refers to the local headers and might not correspond
        // to the len field. If the length field is 1+4, we assume that only
        // the modification time has been set

        // > Those times that are present will appear in the order indicated, but
        // > any combination of times may be omitted.  (Creation time may be
        // > present without access time, for example.)  TSize should equal
        // > (1 + 4*(number of set bits in Flags)), as the block is currently
        // > defined.
        if len != 5 && len as u32 != 1 + 4 * flags.count_ones() {
            //panic!("found len {len} and flags {flags:08b}");
            return Err(ZipError::UnsupportedArchive(
                "flags and len don't match in extended timestamp field",
            ));
        }

        // allow unsupported/undocumented flags

        let mod_time = if (flags & 0b00000001u8 == 0b00000001u8) || len == 5 {
            bytes_to_read -= size_of::<u32>();
            Some(reader.read_u32_le()?)
        } else {
            None
        };

        let ac_time = if flags & 0b00000010u8 == 0b00000010u8 && len > 5 {
            bytes_to_read -= size_of::<u32>();
            Some(reader.read_u32_le()?)
        } else {
            None
        };

        let cr_time = if flags & 0b00000100u8 == 0b00000100u8 && len > 5 {
            bytes_to_read -= size_of::<u32>();
            Some(reader.read_u32_le()?)
        } else {
            None
        };

        if bytes_to_read > 0 {
            // ignore undocumented bytes
            reader.read_exact(&mut vec![0; bytes_to_read])?
        }

        Ok(Self {
            mod_time,
            ac_time,
            cr_time,
        })
    }

    /// returns the last modification timestamp, if defined, as UNIX epoch seconds
    pub fn mod_time(&self) -> Option<u32> {
        self.mod_time
    }

    /// returns the last access timestamp, if defined, as UNIX epoch seconds
    pub fn ac_time(&self) -> Option<u32> {
        self.ac_time
    }

    /// returns the creation timestamp, if defined, as UNIX epoch seconds
    pub fn cr_time(&self) -> Option<u32> {
        self.cr_time
    }
}

#[test]
/// Ensure we don't panic or read garbage data if the field body is empty
pub fn test_bad_extended_timestamp() -> ZipResult<()> {
    use crate::ZipArchive;
    use std::io::Cursor;

    assert!(ZipArchive::new(Cursor::new(include_bytes!(
        "../../tests/data/extended_timestamp_bad.zip"
    )))
    .is_err());
    Ok(())
}
