#![macro_use]

use crate::read::magic_finder::{Backwards, Forward, MagicFinder, OptimisticMagicFinder};
use crate::read::ArchiveOffset;
use crate::result::{invalid, ZipError, ZipResult};
use core::any::type_name;
use core::mem;
use core::slice;
use std::io::{self, Read, Seek, Write};

/// "Magic" header values used in the zip spec to locate metadata records.
///
/// These values currently always take up a fixed four bytes, so we can parse and wrap them in this
/// struct to enforce some small amount of type safety.
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub(crate) struct Magic(u32);

impl Magic {
    pub const fn literal(x: u32) -> Self {
        Self(x)
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub const fn from_le_bytes(bytes: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bytes))
    }

    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    #[allow(clippy::wrong_self_convention)]
    #[inline(always)]
    pub fn from_le(self) -> Self {
        Self(u32::from_le(self.0))
    }

    #[allow(clippy::wrong_self_convention)]
    #[inline(always)]
    pub fn to_le(self) -> Self {
        Self(u32::to_le(self.0))
    }

    pub const LOCAL_FILE_HEADER_SIGNATURE: Self = Self::literal(0x04034b50);
    pub const CENTRAL_DIRECTORY_HEADER_SIGNATURE: Self = Self::literal(0x02014b50);
    pub const CENTRAL_DIRECTORY_END_SIGNATURE: Self = Self::literal(0x06054b50);
    pub const ZIP64_CENTRAL_DIRECTORY_END_SIGNATURE: Self = Self::literal(0x06064b50);
    pub const ZIP64_CENTRAL_DIRECTORY_END_LOCATOR_SIGNATURE: Self = Self::literal(0x07064b50);
    pub const DATA_DESCRIPTOR_SIGNATURE: Self = Self::literal(0x08074b50);
}

/// Similar to [`Magic`], but used for extra field tags as per section 4.5.3 of APPNOTE.TXT.
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub(crate) struct ExtraFieldMagic(u16);

/* TODO: maybe try to use this for parsing extra fields as well as writing them? */
#[allow(dead_code)]
impl ExtraFieldMagic {
    pub const fn literal(x: u16) -> Self {
        Self(x)
    }

    #[inline(always)]
    pub const fn from_le_bytes(bytes: [u8; 2]) -> Self {
        Self(u16::from_le_bytes(bytes))
    }

    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }

    #[allow(clippy::wrong_self_convention)]
    #[inline(always)]
    pub fn from_le(self) -> Self {
        Self(u16::from_le(self.0))
    }

    #[allow(clippy::wrong_self_convention)]
    #[inline(always)]
    pub fn to_le(self) -> Self {
        Self(u16::to_le(self.0))
    }

    pub const ZIP64_EXTRA_FIELD_TAG: Self = Self::literal(0x0001);
}

/// The file size at which a ZIP64 record becomes necessary.
///
/// If a file larger than this threshold attempts to be written, compressed or uncompressed, and
/// [`FileOptions::large_file()`](crate::write::FileOptions::large_file) was not true, then [`crate::ZipWriter`] will
/// raise an [`io::Error`] with [`io::ErrorKind::Other`].
///
/// If the zip file itself is larger than this value, then a zip64 central directory record will be
/// written to the end of the file.
///
///```
/// # fn main() -> Result<(), zip::result::ZipError> {
/// # #[cfg(target_pointer_width = "64")]
/// # {
/// use std::io::{self, Cursor, Write};
/// use std::error::Error;
/// use zip::{ZipWriter, write::SimpleFileOptions};
///
/// let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
/// // Writing an extremely large file for this test is faster without compression.
///
/// let big_len: usize = (zip::ZIP64_BYTES_THR as usize) + 1;
/// let big_buf = vec![0u8; big_len];
/// {
///     let options = SimpleFileOptions::default()
///         .compression_method(zip::CompressionMethod::Stored);
///     zip.start_file("zero.dat", options)?;
///     // This is too big!
///     let res = zip.write_all(&big_buf[..]).err().unwrap();
///     assert_eq!(res.kind(), io::ErrorKind::Other);
///     let description = format!("{}", &res);
///     assert_eq!(description, "Large file option has not been set");
///     // Attempting to write anything further to the same zip will still succeed, but the previous
///     // failing entry has been removed.
///     zip.start_file("one.dat", options)?;
///     let zip = zip.finish_into_readable()?;
///     let names: Vec<_> = zip.file_names().collect();
///     assert_eq!(&names, &["one.dat"]);
/// }
///
/// // Create a new zip output.
/// let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
/// // This time, create a zip64 record for the file.
/// let options = SimpleFileOptions::default()
///      .compression_method(zip::CompressionMethod::Stored)
///      .large_file(true);
/// zip.start_file("zero.dat", options)?;
/// // This succeeds because we specified that it could be a large file.
/// assert!(zip.write_all(&big_buf[..]).is_ok());
/// # }
/// # Ok(())
/// # }
///```
pub const ZIP64_BYTES_THR: u64 = u32::MAX as u64;
/// The number of entries within a single zip necessary to allocate a zip64 central
/// directory record.
///
/// If more than this number of entries is written to a [`crate::ZipWriter`], then [`crate::ZipWriter::finish()`]
/// will write out extra zip64 data to the end of the zip file.
pub const ZIP64_ENTRY_THR: usize = u16::MAX as usize;

/// # Safety
///
/// - No padding/uninit bytes
/// - All bytes patterns must be valid
/// - No cell, pointers
///
/// See `bytemuck::Pod` for more details.
pub(crate) unsafe trait Pod: Copy + 'static {
    #[inline]
    fn zeroed() -> Self {
        unsafe { mem::zeroed() }
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const Self as *const u8, mem::size_of::<Self>()) }
    }

    #[inline]
    fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self as *mut Self as *mut u8, mem::size_of::<Self>()) }
    }
}

pub(crate) trait FixedSizeBlock: Pod {
    const MAGIC: Magic;

    fn magic(self) -> Magic;

    const WRONG_MAGIC_ERROR: ZipError;

    #[allow(clippy::wrong_self_convention)]
    fn from_le(self) -> Self;

    fn parse<R: Read + ?Sized>(reader: &mut R) -> ZipResult<Self> {
        let mut block = Self::zeroed();
        if let Err(e) = reader.read_exact(block.as_bytes_mut()) {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                return Err(invalid!("Unexpected end of {}", type_name::<Self>()));
            }
            return Err(e.into());
        }
        let block = Self::from_le(block);

        if block.magic() != Self::MAGIC {
            return Err(Self::WRONG_MAGIC_ERROR);
        }
        Ok(block)
    }

    fn to_le(self) -> Self;

    fn write<T: Write>(self, writer: &mut T) -> ZipResult<()> {
        let block = self.to_le();
        writer.write_all(block.as_bytes())?;
        Ok(())
    }
}

/// Convert all the fields of a struct *from* little-endian representations.
macro_rules! from_le {
    ($obj:ident, $field:ident, $type:ty) => {
        $obj.$field = <$type>::from_le($obj.$field);
    };
    ($obj:ident, [($field:ident, $type:ty) $(,)?]) => {
        from_le![$obj, $field, $type];
    };
    ($obj:ident, [($field:ident, $type:ty), $($rest:tt),+ $(,)?]) => {
        from_le![$obj, $field, $type];
        from_le!($obj, [$($rest),+]);
    };
}

/// Convert all the fields of a struct *into* little-endian representations.
macro_rules! to_le {
    ($obj:ident, $field:ident, $type:ty) => {
        $obj.$field = <$type>::to_le($obj.$field);
    };
    ($obj:ident, [($field:ident, $type:ty) $(,)?]) => {
        to_le![$obj, $field, $type];
    };
    ($obj:ident, [($field:ident, $type:ty), $($rest:tt),+ $(,)?]) => {
        to_le![$obj, $field, $type];
        to_le!($obj, [$($rest),+]);
    };
}

/* TODO: derive macro to generate these fields? */
/// Implement `from_le()` and `to_le()`, providing the field specification to both macros
/// and methods.
macro_rules! to_and_from_le {
    ($($args:tt),+ $(,)?) => {
        #[inline(always)]
        fn from_le(mut self) -> Self {
            from_le![self, [$($args),+]];
            self
        }
        #[inline(always)]
        fn to_le(mut self) -> Self {
            to_le![self, [$($args),+]];
            self
        }
    };
}

#[derive(Copy, Clone, Debug)]
#[repr(packed, C)]
pub(crate) struct Zip32CDEBlock {
    magic: Magic,
    pub disk_number: u16,
    pub disk_with_central_directory: u16,
    pub number_of_files_on_this_disk: u16,
    pub number_of_files: u16,
    pub central_directory_size: u32,
    pub central_directory_offset: u32,
    pub zip_file_comment_length: u16,
}

unsafe impl Pod for Zip32CDEBlock {}

impl FixedSizeBlock for Zip32CDEBlock {
    const MAGIC: Magic = Magic::CENTRAL_DIRECTORY_END_SIGNATURE;

    #[inline(always)]
    fn magic(self) -> Magic {
        self.magic
    }

    const WRONG_MAGIC_ERROR: ZipError = invalid!("Invalid digital signature header");

    to_and_from_le![
        (magic, Magic),
        (disk_number, u16),
        (disk_with_central_directory, u16),
        (number_of_files_on_this_disk, u16),
        (number_of_files, u16),
        (central_directory_size, u32),
        (central_directory_offset, u32),
        (zip_file_comment_length, u16)
    ];
}

#[derive(Debug)]
pub(crate) struct Zip32CentralDirectoryEnd {
    pub disk_number: u16,
    pub disk_with_central_directory: u16,
    pub number_of_files_on_this_disk: u16,
    pub number_of_files: u16,
    pub central_directory_size: u32,
    pub central_directory_offset: u32,
    pub zip_file_comment: Box<[u8]>,
}

impl Zip32CentralDirectoryEnd {
    fn into_block_and_comment(self) -> (Zip32CDEBlock, Box<[u8]>) {
        let Self {
            disk_number,
            disk_with_central_directory,
            number_of_files_on_this_disk,
            number_of_files,
            central_directory_size,
            central_directory_offset,
            zip_file_comment,
        } = self;
        let block = Zip32CDEBlock {
            magic: Zip32CDEBlock::MAGIC,
            disk_number,
            disk_with_central_directory,
            number_of_files_on_this_disk,
            number_of_files,
            central_directory_size,
            central_directory_offset,
            zip_file_comment_length: zip_file_comment.len() as u16,
        };

        (block, zip_file_comment)
    }

    pub fn parse<T: Read + ?Sized>(reader: &mut T) -> ZipResult<Zip32CentralDirectoryEnd> {
        let Zip32CDEBlock {
            // magic,
            disk_number,
            disk_with_central_directory,
            number_of_files_on_this_disk,
            number_of_files,
            central_directory_size,
            central_directory_offset,
            zip_file_comment_length,
            ..
        } = Zip32CDEBlock::parse(reader)?;

        let mut zip_file_comment = vec![0u8; zip_file_comment_length as usize].into_boxed_slice();
        if let Err(e) = reader.read_exact(&mut zip_file_comment) {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                return Err(invalid!("EOCD comment exceeds file boundary"));
            }

            return Err(e.into());
        }

        Ok(Zip32CentralDirectoryEnd {
            disk_number,
            disk_with_central_directory,
            number_of_files_on_this_disk,
            number_of_files,
            central_directory_size,
            central_directory_offset,
            zip_file_comment,
        })
    }

    pub fn write<T: Write>(self, writer: &mut T) -> ZipResult<()> {
        let (block, comment) = self.into_block_and_comment();

        if comment.len() > u16::MAX as usize {
            return Err(invalid!("EOCD comment length exceeds u16::MAX"));
        }

        block.write(writer)?;
        writer.write_all(&comment)?;
        Ok(())
    }

    pub fn may_be_zip64(&self) -> bool {
        self.number_of_files == u16::MAX || self.central_directory_offset == u32::MAX
    }
}

#[derive(Copy, Clone)]
#[repr(packed, C)]
pub(crate) struct Zip64CDELocatorBlock {
    magic: Magic,
    pub disk_with_central_directory: u32,
    pub end_of_central_directory_offset: u64,
    pub number_of_disks: u32,
}

unsafe impl Pod for Zip64CDELocatorBlock {}

impl FixedSizeBlock for Zip64CDELocatorBlock {
    const MAGIC: Magic = Magic::ZIP64_CENTRAL_DIRECTORY_END_LOCATOR_SIGNATURE;

    #[inline(always)]
    fn magic(self) -> Magic {
        self.magic
    }

    const WRONG_MAGIC_ERROR: ZipError = invalid!("Invalid zip64 locator digital signature header");

    to_and_from_le![
        (magic, Magic),
        (disk_with_central_directory, u32),
        (end_of_central_directory_offset, u64),
        (number_of_disks, u32),
    ];
}

pub(crate) struct Zip64CentralDirectoryEndLocator {
    pub disk_with_central_directory: u32,
    pub end_of_central_directory_offset: u64,
    pub number_of_disks: u32,
}

impl Zip64CentralDirectoryEndLocator {
    pub fn parse<T: Read + ?Sized>(reader: &mut T) -> ZipResult<Zip64CentralDirectoryEndLocator> {
        let Zip64CDELocatorBlock {
            // magic,
            disk_with_central_directory,
            end_of_central_directory_offset,
            number_of_disks,
            ..
        } = Zip64CDELocatorBlock::parse(reader)?;

        Ok(Zip64CentralDirectoryEndLocator {
            disk_with_central_directory,
            end_of_central_directory_offset,
            number_of_disks,
        })
    }

    pub fn block(self) -> Zip64CDELocatorBlock {
        let Self {
            disk_with_central_directory,
            end_of_central_directory_offset,
            number_of_disks,
        } = self;
        Zip64CDELocatorBlock {
            magic: Zip64CDELocatorBlock::MAGIC,
            disk_with_central_directory,
            end_of_central_directory_offset,
            number_of_disks,
        }
    }

    pub fn write<T: Write>(self, writer: &mut T) -> ZipResult<()> {
        self.block().write(writer)
    }
}

#[derive(Copy, Clone)]
#[repr(packed, C)]
pub(crate) struct Zip64CDEBlock {
    magic: Magic,
    pub record_size: u64,
    pub version_made_by: u16,
    pub version_needed_to_extract: u16,
    pub disk_number: u32,
    pub disk_with_central_directory: u32,
    pub number_of_files_on_this_disk: u64,
    pub number_of_files: u64,
    pub central_directory_size: u64,
    pub central_directory_offset: u64,
}

unsafe impl Pod for Zip64CDEBlock {}

impl FixedSizeBlock for Zip64CDEBlock {
    const MAGIC: Magic = Magic::ZIP64_CENTRAL_DIRECTORY_END_SIGNATURE;

    fn magic(self) -> Magic {
        self.magic
    }

    const WRONG_MAGIC_ERROR: ZipError = invalid!("Invalid digital signature header");

    to_and_from_le![
        (magic, Magic),
        (record_size, u64),
        (version_made_by, u16),
        (version_needed_to_extract, u16),
        (disk_number, u32),
        (disk_with_central_directory, u32),
        (number_of_files_on_this_disk, u64),
        (number_of_files, u64),
        (central_directory_size, u64),
        (central_directory_offset, u64),
    ];
}

pub(crate) struct Zip64CentralDirectoryEnd {
    pub record_size: u64,
    pub version_made_by: u16,
    pub version_needed_to_extract: u16,
    pub disk_number: u32,
    pub disk_with_central_directory: u32,
    pub number_of_files_on_this_disk: u64,
    pub number_of_files: u64,
    pub central_directory_size: u64,
    pub central_directory_offset: u64,
    pub extensible_data_sector: Box<[u8]>,
}

impl Zip64CentralDirectoryEnd {
    pub fn parse<T: Read + ?Sized>(
        reader: &mut T,
        max_size: u64,
    ) -> ZipResult<Zip64CentralDirectoryEnd> {
        let Zip64CDEBlock {
            record_size,
            version_made_by,
            version_needed_to_extract,
            disk_number,
            disk_with_central_directory,
            number_of_files_on_this_disk,
            number_of_files,
            central_directory_size,
            central_directory_offset,
            ..
        } = Zip64CDEBlock::parse(reader)?;

        if record_size < 44 {
            return Err(invalid!("Low EOCD64 record size"));
        } else if record_size.saturating_add(12) > max_size {
            return Err(invalid!("EOCD64 extends beyond EOCD64 locator"));
        }

        let mut zip_file_comment = vec![0u8; record_size as usize - 44].into_boxed_slice();
        if let Err(e) = reader.read_exact(&mut zip_file_comment) {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                return Err(invalid!(
                    "EOCD64 extensible data sector exceeds file boundary"
                ));
            }
            return Err(e.into());
        }

        Ok(Self {
            record_size,
            version_made_by,
            version_needed_to_extract,
            disk_number,
            disk_with_central_directory,
            number_of_files_on_this_disk,
            number_of_files,
            central_directory_size,
            central_directory_offset,
            extensible_data_sector: zip_file_comment,
        })
    }

    pub fn into_block_and_comment(self) -> (Zip64CDEBlock, Box<[u8]>) {
        let Self {
            record_size,
            version_made_by,
            version_needed_to_extract,
            disk_number,
            disk_with_central_directory,
            number_of_files_on_this_disk,
            number_of_files,
            central_directory_size,
            central_directory_offset,
            extensible_data_sector,
        } = self;

        (
            Zip64CDEBlock {
                magic: Zip64CDEBlock::MAGIC,
                record_size,
                version_made_by,
                version_needed_to_extract,
                disk_number,
                disk_with_central_directory,
                number_of_files_on_this_disk,
                number_of_files,
                central_directory_size,
                central_directory_offset,
            },
            extensible_data_sector,
        )
    }

    pub fn write<T: Write>(self, writer: &mut T) -> ZipResult<()> {
        let (block, comment) = self.into_block_and_comment();
        block.write(writer)?;
        writer.write_all(&comment)?;
        Ok(())
    }
}

pub(crate) struct DataAndPosition<T> {
    pub data: T,
    #[allow(dead_code)]
    pub position: u64,
}

impl<T> From<(T, u64)> for DataAndPosition<T> {
    fn from(value: (T, u64)) -> Self {
        Self {
            data: value.0,
            position: value.1,
        }
    }
}

pub(crate) struct CentralDirectoryEndInfo {
    pub eocd: DataAndPosition<Zip32CentralDirectoryEnd>,
    pub eocd64: Option<DataAndPosition<Zip64CentralDirectoryEnd>>,

    pub archive_offset: u64,
}

/// Finds the EOCD and possibly the EOCD64 block and determines the archive offset.
///
/// In the best case scenario (no prepended junk), this function will not backtrack
/// in the reader.
pub(crate) fn find_central_directory<R: Read + Seek + ?Sized>(
    reader: &mut R,
    archive_offset: ArchiveOffset,
    end_exclusive: u64,
    file_len: u64,
) -> ZipResult<CentralDirectoryEndInfo> {
    const EOCD_SIG_BYTES: [u8; mem::size_of::<Magic>()] =
        Magic::CENTRAL_DIRECTORY_END_SIGNATURE.to_le_bytes();

    const EOCD64_SIG_BYTES: [u8; mem::size_of::<Magic>()] =
        Magic::ZIP64_CENTRAL_DIRECTORY_END_SIGNATURE.to_le_bytes();

    const CDFH_SIG_BYTES: [u8; mem::size_of::<Magic>()] =
        Magic::CENTRAL_DIRECTORY_HEADER_SIGNATURE.to_le_bytes();

    // Instantiate the mandatory finder
    let mut eocd_finder = MagicFinder::<Backwards<'static>>::new(&EOCD_SIG_BYTES, 0, end_exclusive);
    let mut subfinder: Option<OptimisticMagicFinder<Forward<'static>>> = None;

    // Keep the last errors for cases of improper EOCD instances.
    let mut parsing_error = None;

    while let Some(eocd_offset) = eocd_finder.next(reader)? {
        // Attempt to parse the EOCD block
        let eocd = match Zip32CentralDirectoryEnd::parse(reader) {
            Ok(eocd) => eocd,
            Err(e) => {
                if parsing_error.is_none() {
                    parsing_error = Some(e);
                }
                continue;
            }
        };

        // ! Relaxed (inequality) due to garbage-after-comment Python files
        // Consistency check: the EOCD comment must terminate before the end of file
        if eocd.zip_file_comment.len() as u64 + eocd_offset + 22 > file_len {
            parsing_error = Some(invalid!("Invalid EOCD comment length"));
            continue;
        }

        let zip64_metadata = if eocd.may_be_zip64() {
            fn try_read_eocd64_locator(
                reader: &mut (impl Read + Seek + ?Sized),
                eocd_offset: u64,
            ) -> ZipResult<(u64, Zip64CentralDirectoryEndLocator)> {
                if eocd_offset < mem::size_of::<Zip64CDELocatorBlock>() as u64 {
                    return Err(invalid!("EOCD64 Locator does not fit in file"));
                }

                let locator64_offset = eocd_offset - mem::size_of::<Zip64CDELocatorBlock>() as u64;

                reader.seek(io::SeekFrom::Start(locator64_offset))?;
                let locator64 = Zip64CentralDirectoryEndLocator::parse(reader);
                Ok((locator64_offset, locator64?))
            }

            try_read_eocd64_locator(reader, eocd_offset).ok()
        } else {
            None
        };

        let Some((locator64_offset, locator64)) = zip64_metadata else {
            // Branch out for zip32
            let relative_cd_offset = eocd.central_directory_offset as u64;

            // If the archive is empty, there is nothing more to be checked, the archive is correct.
            if eocd.number_of_files == 0 {
                return Ok(CentralDirectoryEndInfo {
                    eocd: (eocd, eocd_offset).into(),
                    eocd64: None,
                    archive_offset: eocd_offset.saturating_sub(relative_cd_offset),
                });
            }

            // Consistency check: the CD relative offset cannot be after the EOCD
            if relative_cd_offset >= eocd_offset {
                parsing_error = Some(invalid!("Invalid CDFH offset in EOCD"));
                continue;
            }

            // Attempt to find the first CDFH
            let subfinder = subfinder
                .get_or_insert_with(OptimisticMagicFinder::new_empty)
                .repurpose(
                    &CDFH_SIG_BYTES,
                    // The CDFH must be before the EOCD and after the relative offset,
                    // because prepended junk can only move it forward.
                    (relative_cd_offset, eocd_offset),
                    match archive_offset {
                        ArchiveOffset::Known(n) => {
                            Some((relative_cd_offset.saturating_add(n).min(eocd_offset), true))
                        }
                        _ => Some((relative_cd_offset, false)),
                    },
                );

            // Consistency check: find the first CDFH
            if let Some(cd_offset) = subfinder.next(reader)? {
                // The first CDFH will define the archive offset
                let archive_offset = cd_offset - relative_cd_offset;

                return Ok(CentralDirectoryEndInfo {
                    eocd: (eocd, eocd_offset).into(),
                    eocd64: None,
                    archive_offset,
                });
            }

            parsing_error = Some(invalid!("No CDFH found"));
            continue;
        };

        // Consistency check: the EOCD64 offset must be before EOCD64 Locator offset */
        if locator64.end_of_central_directory_offset >= locator64_offset {
            parsing_error = Some(invalid!("Invalid EOCD64 Locator CD offset"));
            continue;
        }

        if locator64.number_of_disks > 1 {
            parsing_error = Some(invalid!("Multi-disk ZIP files are not supported"));
            continue;
        }

        // This was hidden inside a function to collect errors in a single place.
        // Once try blocks are stabilized, this can go away.
        fn try_read_eocd64<R: Read + Seek + ?Sized>(
            reader: &mut R,
            locator64: &Zip64CentralDirectoryEndLocator,
            expected_length: u64,
        ) -> ZipResult<Zip64CentralDirectoryEnd> {
            let z64 = Zip64CentralDirectoryEnd::parse(reader, expected_length)?;

            // Consistency check: EOCD64 locator should agree with the EOCD64
            if z64.disk_with_central_directory != locator64.disk_with_central_directory {
                return Err(invalid!("Invalid EOCD64: inconsistency with Locator data"));
            }

            // Consistency check: the EOCD64 must have the expected length
            if z64.record_size + 12 != expected_length {
                return Err(invalid!("Invalid EOCD64: inconsistent length"));
            }

            Ok(z64)
        }

        // Attempt to find the EOCD64 with an initial guess
        let subfinder = subfinder
            .get_or_insert_with(OptimisticMagicFinder::new_empty)
            .repurpose(
                &EOCD64_SIG_BYTES,
                (locator64.end_of_central_directory_offset, locator64_offset),
                match archive_offset {
                    ArchiveOffset::Known(n) => Some((
                        locator64
                            .end_of_central_directory_offset
                            .saturating_add(n)
                            .min(locator64_offset),
                        true,
                    )),
                    _ => Some((locator64.end_of_central_directory_offset, false)),
                },
            );

        // Consistency check: Find the EOCD64
        let mut local_error = None;
        while let Some(eocd64_offset) = subfinder.next(reader)? {
            let archive_offset = eocd64_offset - locator64.end_of_central_directory_offset;

            match try_read_eocd64(
                reader,
                &locator64,
                locator64_offset.saturating_sub(eocd64_offset),
            ) {
                Ok(eocd64) => {
                    if eocd64_offset
                        < eocd64
                            .number_of_files
                            .saturating_mul(
                                mem::size_of::<crate::types::ZipCentralEntryBlock>() as u64
                            )
                            .saturating_add(eocd64.central_directory_offset)
                    {
                        local_error =
                            Some(invalid!("Invalid EOCD64: inconsistent number of files"));
                        continue;
                    }

                    return Ok(CentralDirectoryEndInfo {
                        eocd: (eocd, eocd_offset).into(),
                        eocd64: Some((eocd64, eocd64_offset).into()),
                        archive_offset,
                    });
                }
                Err(e) => {
                    local_error = Some(e);
                }
            }
        }

        parsing_error = local_error.or(Some(invalid!("Could not find EOCD64")));
    }

    Err(parsing_error.unwrap_or(invalid!("Could not find EOCD")))
}

pub(crate) fn is_dir(filename: &str) -> bool {
    filename
        .chars()
        .next_back()
        .is_some_and(|c| c == '/' || c == '\\')
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use crate::{
        result::{invalid, ZipError},
        spec::{FixedSizeBlock, Magic, Pod},
    };

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    #[repr(packed, C)]
    pub struct TestBlock {
        magic: Magic,
        pub file_name_length: u16,
    }

    unsafe impl Pod for TestBlock {}

    impl FixedSizeBlock for TestBlock {
        const MAGIC: Magic = Magic::literal(0x01111);

        fn magic(self) -> Magic {
            self.magic
        }

        const WRONG_MAGIC_ERROR: ZipError = invalid!("unreachable");

        to_and_from_le![(magic, Magic), (file_name_length, u16)];
    }

    /// Demonstrate that a block object can be safely written to memory and deserialized back out.
    #[test]
    fn block_serde() {
        let block = TestBlock {
            magic: TestBlock::MAGIC,
            file_name_length: 3,
        };
        let mut c = Cursor::new(Vec::new());
        block.write(&mut c).unwrap();
        c.set_position(0);
        let block2 = TestBlock::parse(&mut c).unwrap();
        assert_eq!(block, block2);
    }
}
