use crate::result::{invalid, ZipResult};
use crate::unstable::LittleEndianReadExt;
use core::mem::size_of;
use std::io::Read;

/// Info-ZIP Unicode Path Extra Field (0x7075) or Unicode Comment Extra Field (0x6375), as
/// specified in APPNOTE 4.6.8 and 4.6.9
#[derive(Clone, Debug)]
pub struct UnicodeExtraField {
    crc32: u32,
    content: Box<[u8]>,
}

impl UnicodeExtraField {
    /// Verifies the checksum and returns the content.
    pub fn unwrap_valid(self, ascii_field: &[u8]) -> ZipResult<Box<[u8]>> {
        let mut crc32 = crc32fast::Hasher::new();
        crc32.update(ascii_field);
        let actual_crc32 = crc32.finalize();
        if self.crc32 != actual_crc32 {
            return Err(invalid!("CRC32 checksum failed on Unicode extra field"));
        }
        Ok(self.content)
    }
}

impl UnicodeExtraField {
    pub(crate) fn try_from_reader<R: Read>(reader: &mut R, len: u16) -> ZipResult<Self> {
        // Read and discard version byte
        reader.read_exact(&mut [0u8])?;

        let crc32 = reader.read_u32_le()?;
        let content_len = (len as usize)
            .checked_sub(size_of::<u8>() + size_of::<u32>())
            .ok_or(invalid!("Unicode extra field is too small"))?;
        let mut content = vec![0u8; content_len].into_boxed_slice();
        reader.read_exact(&mut content)?;
        Ok(Self { crc32, content })
    }
}
