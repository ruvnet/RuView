//! Types that specify what is contained in a ZIP.
use crate::cfg_if_expr;
use crate::cp437::FromCp437;
use crate::write::FileOptionExtension;
use crate::zipcrypto::EncryptWith;
use core::fmt::{self, Debug, Formatter};
use core::mem;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use typed_path::{Utf8WindowsComponent, Utf8WindowsPath};

#[cfg(feature = "chrono")]
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
#[cfg(feature = "jiff-02")]
use jiff::civil;

use crate::result::{invalid, ZipError, ZipResult};
use crate::spec::{self, FixedSizeBlock, Pod};

pub(crate) mod ffi {
    pub const S_IFDIR: u32 = 0o0040000;
    pub const S_IFREG: u32 = 0o0100000;
    pub const S_IFLNK: u32 = 0o0120000;
}

use crate::extra_fields::ExtraField;
use crate::read::find_data_start;
use crate::result::DateTimeRangeError;
use crate::spec::is_dir;
use crate::types::ffi::S_IFDIR;
use crate::{CompressionMethod, ZIP64_BYTES_THR};
use std::io::{Read, Seek};
#[cfg(feature = "time")]
use time::{error::ComponentRange, Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

pub(crate) struct ZipRawValues {
    pub(crate) crc32: u32,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum System {
    Dos = 0,
    Unix = 3,
    #[default]
    Unknown,
}

impl From<u8> for System {
    fn from(system: u8) -> Self {
        match system {
            0 => Self::Dos,
            3 => Self::Unix,
            _ => Self::Unknown,
        }
    }
}

impl From<System> for u8 {
    fn from(system: System) -> Self {
        match system {
            System::Dos => 0,
            System::Unix => 3,
            System::Unknown => 4,
        }
    }
}

/// Metadata for a file to be written
#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub struct FileOptions<'k, T: FileOptionExtension> {
    pub(crate) compression_method: CompressionMethod,
    pub(crate) compression_level: Option<i64>,
    pub(crate) last_modified_time: DateTime,
    pub(crate) permissions: Option<u32>,
    pub(crate) large_file: bool,
    pub(crate) encrypt_with: Option<EncryptWith<'k>>,
    pub(crate) extended_options: T,
    pub(crate) alignment: u16,
    #[cfg(feature = "deflate-zopfli")]
    pub(super) zopfli_buffer_size: Option<usize>,
    #[cfg(feature = "aes-crypto")]
    pub(crate) aes_mode: Option<(AesMode, AesVendorVersion, CompressionMethod)>,
}
/// Simple File Options. Can be copied and good for simple writing zip files
pub type SimpleFileOptions = FileOptions<'static, ()>;

/// Representation of a moment in time.
///
/// Zip files use an old format from DOS to store timestamps,
/// with its own set of peculiarities.
/// For example, it has a resolution of 2 seconds!
///
/// A [`DateTime`] can be stored directly in a zipfile with [`FileOptions::last_modified_time`],
/// or read from one with [`ZipFile::last_modified`](crate::read::ZipFile::last_modified).
///
/// # Warning
///
/// Because there is no timezone associated with the [`DateTime`], they should ideally only
/// be used for user-facing descriptions.
///
/// Modern zip files store more precise timestamps; see [`crate::extra_fields::ExtendedTimestamp`]
/// for details.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DateTime {
    datepart: u16,
    timepart: u16,
}

impl Debug for DateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if *self == Self::default() {
            return f.write_str("DateTime::default()");
        }
        f.write_fmt(format_args!(
            "DateTime::from_date_and_time({}, {}, {}, {}, {}, {})?",
            self.year(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        ))
    }
}

impl DateTime {
    /// Constructs a default datetime of 1980-01-01 00:00:00.
    pub const DEFAULT: Self = DateTime {
        datepart: 0b0000000000100001,
        timepart: 0,
    };

    /// Returns the current time if possible, otherwise the default of 1980-01-01.
    #[cfg(feature = "time")]
    pub fn default_for_write() -> Self {
        let now = OffsetDateTime::now_utc();
        PrimitiveDateTime::new(now.date(), now.time())
            .try_into()
            .unwrap_or_else(|_| DateTime::default())
    }

    /// Returns the current time if possible, otherwise the default of 1980-01-01.
    #[cfg(not(feature = "time"))]
    pub fn default_for_write() -> Self {
        DateTime::default()
    }
}

#[cfg(feature = "_arbitrary")]
impl arbitrary::Arbitrary<'_> for DateTime {
    fn arbitrary(u: &mut arbitrary::Unstructured) -> arbitrary::Result<Self> {
        let year: u16 = u.int_in_range(1980..=2107)?;
        let month: u16 = u.int_in_range(1..=12)?;
        let day: u16 = u.int_in_range(1..=31)?;
        let datepart = day | (month << 5) | ((year - 1980) << 9);
        let hour: u16 = u.int_in_range(0..=23)?;
        let minute: u16 = u.int_in_range(0..=59)?;
        let second: u16 = u.int_in_range(0..=58)?;
        let timepart = (second >> 1) | (minute << 5) | (hour << 11);
        Ok(DateTime { datepart, timepart })
    }
}

#[cfg(feature = "chrono")]
impl TryFrom<NaiveDateTime> for DateTime {
    type Error = DateTimeRangeError;

    fn try_from(value: NaiveDateTime) -> Result<Self, Self::Error> {
        DateTime::from_date_and_time(
            value.year().try_into()?,
            value.month().try_into()?,
            value.day().try_into()?,
            value.hour().try_into()?,
            value.minute().try_into()?,
            value.second().try_into()?,
        )
    }
}

#[cfg(feature = "chrono")]
impl TryFrom<DateTime> for NaiveDateTime {
    type Error = DateTimeRangeError;

    fn try_from(value: DateTime) -> Result<Self, Self::Error> {
        let date = NaiveDate::from_ymd_opt(
            value.year().into(),
            value.month().into(),
            value.day().into(),
        )
        .ok_or(DateTimeRangeError)?;
        let time = NaiveTime::from_hms_opt(
            value.hour().into(),
            value.minute().into(),
            value.second().into(),
        )
        .ok_or(DateTimeRangeError)?;
        Ok(NaiveDateTime::new(date, time))
    }
}

#[cfg(feature = "jiff-02")]
impl TryFrom<civil::DateTime> for DateTime {
    type Error = DateTimeRangeError;

    fn try_from(value: civil::DateTime) -> Result<Self, Self::Error> {
        Self::from_date_and_time(
            value.year().try_into()?,
            value.month() as u8,
            value.day() as u8,
            value.hour() as u8,
            value.minute() as u8,
            value.second() as u8,
        )
    }
}

#[cfg(feature = "jiff-02")]
impl TryFrom<DateTime> for civil::DateTime {
    type Error = jiff::Error;

    fn try_from(value: DateTime) -> Result<Self, Self::Error> {
        Self::new(
            value.year() as i16,
            value.month() as i8,
            value.day() as i8,
            value.hour() as i8,
            value.minute() as i8,
            value.second() as i8,
            0,
        )
    }
}

impl TryFrom<(u16, u16)> for DateTime {
    type Error = DateTimeRangeError;

    #[inline]
    fn try_from(values: (u16, u16)) -> Result<Self, Self::Error> {
        Self::try_from_msdos(values.0, values.1)
    }
}

impl From<DateTime> for (u16, u16) {
    #[inline]
    fn from(dt: DateTime) -> Self {
        (dt.datepart(), dt.timepart())
    }
}

impl Default for DateTime {
    /// Constructs an 'default' datetime of 1980-01-01 00:00:00
    fn default() -> DateTime {
        DateTime::DEFAULT
    }
}

impl fmt::Display for DateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}

impl DateTime {
    /// Converts an msdos (u16, u16) pair to a DateTime object
    ///
    /// # Safety
    /// The caller must ensure the date and time are valid.
    pub const unsafe fn from_msdos_unchecked(datepart: u16, timepart: u16) -> DateTime {
        DateTime { datepart, timepart }
    }

    /// Converts an msdos (u16, u16) pair to a DateTime object if it represents a valid date and
    /// time.
    pub fn try_from_msdos(datepart: u16, timepart: u16) -> Result<DateTime, DateTimeRangeError> {
        let seconds = (timepart & 0b0000000000011111) << 1;
        let minutes = (timepart & 0b0000011111100000) >> 5;
        let hours = (timepart & 0b1111100000000000) >> 11;
        let days = datepart & 0b0000000000011111;
        let months = (datepart & 0b0000000111100000) >> 5;
        let years = (datepart & 0b1111111000000000) >> 9;
        Self::from_date_and_time(
            years.checked_add(1980).ok_or(DateTimeRangeError)?,
            months.try_into()?,
            days.try_into()?,
            hours.try_into()?,
            minutes.try_into()?,
            seconds.try_into()?,
        )
    }

    /// Constructs a DateTime from a specific date and time
    ///
    /// The bounds are:
    /// * year: [1980, 2107]
    /// * month: [1, 12]
    /// * day: [1, 28..=31]
    /// * hour: [0, 23]
    /// * minute: [0, 59]
    /// * second: [0, 58]
    pub fn from_date_and_time(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<DateTime, DateTimeRangeError> {
        fn is_leap_year(year: u16) -> bool {
            (year % 4 == 0) && ((year % 25 != 0) || (year % 16 == 0))
        }

        if (1980..=2107).contains(&year)
            && (1..=12).contains(&month)
            && (1..=31).contains(&day)
            && hour <= 23
            && minute <= 59
            && second <= 60
        {
            let second = second.min(58); // exFAT can't store leap seconds
            let max_day = match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 if is_leap_year(year) => 29,
                2 => 28,
                _ => unreachable!(),
            };
            if day > max_day {
                return Err(DateTimeRangeError);
            }
            let datepart = (day as u16) | ((month as u16) << 5) | ((year - 1980) << 9);
            let timepart = ((second as u16) >> 1) | ((minute as u16) << 5) | ((hour as u16) << 11);
            Ok(DateTime { datepart, timepart })
        } else {
            Err(DateTimeRangeError)
        }
    }

    /// Indicates whether this date and time can be written to a zip archive.
    pub fn is_valid(&self) -> bool {
        Self::try_from_msdos(self.datepart, self.timepart).is_ok()
    }

    #[cfg(feature = "time")]
    /// Converts a OffsetDateTime object to a DateTime
    ///
    /// Returns `Err` when this object is out of bounds
    #[deprecated(since = "0.6.4", note = "use `DateTime::try_from()` instead")]
    pub fn from_time(dt: OffsetDateTime) -> Result<DateTime, DateTimeRangeError> {
        dt.try_into()
    }

    /// Gets the time portion of this datetime in the msdos representation
    pub const fn timepart(&self) -> u16 {
        self.timepart
    }

    /// Gets the date portion of this datetime in the msdos representation
    pub const fn datepart(&self) -> u16 {
        self.datepart
    }

    #[cfg(feature = "time")]
    /// Converts the DateTime to a OffsetDateTime structure
    #[deprecated(since = "1.3.1", note = "use `OffsetDateTime::try_from()` instead")]
    pub fn to_time(&self) -> Result<OffsetDateTime, ComponentRange> {
        (*self).try_into()
    }

    /// Get the year. There is no epoch, i.e. 2018 will be returned as 2018.
    pub const fn year(&self) -> u16 {
        (self.datepart >> 9) + 1980
    }

    /// Get the month, where 1 = january and 12 = december
    ///
    /// # Warning
    ///
    /// When read from a zip file, this may not be a reasonable value
    pub const fn month(&self) -> u8 {
        ((self.datepart & 0b0000000111100000) >> 5) as u8
    }

    /// Get the day
    ///
    /// # Warning
    ///
    /// When read from a zip file, this may not be a reasonable value
    pub const fn day(&self) -> u8 {
        (self.datepart & 0b0000000000011111) as u8
    }

    /// Get the hour
    ///
    /// # Warning
    ///
    /// When read from a zip file, this may not be a reasonable value
    pub const fn hour(&self) -> u8 {
        (self.timepart >> 11) as u8
    }

    /// Get the minute
    ///
    /// # Warning
    ///
    /// When read from a zip file, this may not be a reasonable value
    pub const fn minute(&self) -> u8 {
        ((self.timepart & 0b0000011111100000) >> 5) as u8
    }

    /// Get the second
    ///
    /// # Warning
    ///
    /// When read from a zip file, this may not be a reasonable value
    pub const fn second(&self) -> u8 {
        ((self.timepart & 0b0000000000011111) << 1) as u8
    }
}

#[cfg(feature = "time")]
impl TryFrom<OffsetDateTime> for DateTime {
    type Error = DateTimeRangeError;

    fn try_from(dt: OffsetDateTime) -> Result<Self, Self::Error> {
        Self::try_from(PrimitiveDateTime::new(dt.date(), dt.time()))
    }
}

#[cfg(feature = "time")]
impl TryFrom<PrimitiveDateTime> for DateTime {
    type Error = DateTimeRangeError;

    fn try_from(dt: PrimitiveDateTime) -> Result<Self, Self::Error> {
        Self::from_date_and_time(
            dt.year().try_into()?,
            dt.month().into(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
        )
    }
}

#[cfg(feature = "time")]
impl TryFrom<DateTime> for OffsetDateTime {
    type Error = ComponentRange;

    fn try_from(dt: DateTime) -> Result<Self, Self::Error> {
        PrimitiveDateTime::try_from(dt).map(PrimitiveDateTime::assume_utc)
    }
}

#[cfg(feature = "time")]
impl TryFrom<DateTime> for PrimitiveDateTime {
    type Error = ComponentRange;

    fn try_from(dt: DateTime) -> Result<Self, Self::Error> {
        let date =
            Date::from_calendar_date(dt.year() as i32, Month::try_from(dt.month())?, dt.day())?;
        let time = Time::from_hms(dt.hour(), dt.minute(), dt.second())?;
        Ok(PrimitiveDateTime::new(date, time))
    }
}

pub const MIN_VERSION: u8 = 10;
pub const DEFAULT_VERSION: u8 = 45;

/// Structure representing a ZIP file.
#[derive(Debug, Clone, Default)]
pub struct ZipFileData {
    /// Compatibility of the file attribute information
    pub system: System,
    /// Specification version
    pub version_made_by: u8,
    /// ZIP flags
    pub flags: u16,
    /// True if the file is encrypted.
    pub encrypted: bool,
    /// True if file_name and file_comment are UTF8
    pub is_utf8: bool,
    /// True if the file uses a data-descriptor section
    pub using_data_descriptor: bool,
    /// Compression method used to store the file
    pub compression_method: crate::compression::CompressionMethod,
    /// Compression level to store the file
    pub compression_level: Option<i64>,
    /// Last modified time. This will only have a 2 second precision.
    pub last_modified_time: Option<DateTime>,
    /// CRC32 checksum
    pub crc32: u32,
    /// Size of the file in the ZIP
    pub compressed_size: u64,
    /// Size of the file when extracted
    pub uncompressed_size: u64,
    /// Name of the file
    pub file_name: Box<str>,
    /// Raw file name. To be used when file_name was incorrectly decoded.
    pub file_name_raw: Box<[u8]>,
    /// Extra field usually used for storage expansion
    pub extra_field: Option<Arc<Vec<u8>>>,
    /// Extra field only written to central directory
    pub central_extra_field: Option<Arc<Vec<u8>>>,
    /// File comment
    pub file_comment: Box<str>,
    /// Specifies where the local header of the file starts
    pub header_start: u64,
    /// Specifies where the extra data of the file starts
    pub extra_data_start: Option<u64>,
    /// Specifies where the central header of the file starts
    ///
    /// Note that when this is not known, it is set to 0
    pub central_header_start: u64,
    /// Specifies where the compressed data of the file starts
    pub data_start: OnceLock<u64>,
    /// External file attributes
    pub external_attributes: u32,
    /// Reserve local ZIP64 extra field
    pub large_file: bool,
    /// AES mode if applicable
    pub aes_mode: Option<(AesMode, AesVendorVersion, CompressionMethod)>,
    /// Specifies where in the extra data the AES metadata starts
    pub aes_extra_data_start: u64,

    /// extra fields, see <https://libzip.org/specifications/extrafld.txt>
    pub extra_fields: Vec<ExtraField>,
}

impl ZipFileData {
    /// Get the starting offset of the data of the compressed file
    pub fn data_start(&self, reader: &mut (impl Read + Seek + ?Sized)) -> ZipResult<u64> {
        match self.data_start.get() {
            Some(data_start) => Ok(*data_start),
            None => Ok(find_data_start(self, reader)?),
        }
    }

    #[allow(dead_code)]
    pub fn is_dir(&self) -> bool {
        is_dir(&self.file_name)
    }

    pub fn file_name_sanitized(&self) -> PathBuf {
        let no_null_filename = match self.file_name.find('\0') {
            Some(index) => &self.file_name[0..index],
            None => &self.file_name,
        };

        Utf8WindowsPath::new(no_null_filename)
            .components()
            .filter(|component| matches!(*component, Utf8WindowsComponent::Normal(..)))
            .fold(PathBuf::new(), |mut path, cur| {
                if let Utf8WindowsComponent::Normal(s) = cur {
                    path.push(s);
                }
                path
            })
    }

    /// Simplify the file name by removing the prefix and parent directories and only return normal components
    pub(crate) fn simplified_components(&self) -> Option<Vec<&OsStr>> {
        if self.file_name.contains('\0') {
            return None;
        }
        let input = Path::new(OsStr::new(&*self.file_name));
        crate::path::simplified_components(input)
    }

    pub(crate) fn enclosed_name(&self) -> Option<PathBuf> {
        if self.file_name.contains('\0') {
            return None;
        }
        let mut depth = 0usize;
        let mut out_path = PathBuf::new();
        for component in Utf8WindowsPath::new(&self.file_name).components() {
            match component {
                Utf8WindowsComponent::Prefix(_) | Utf8WindowsComponent::RootDir => {
                    if depth > 0 {
                        return None;
                    }
                }
                Utf8WindowsComponent::ParentDir => {
                    depth = depth.checked_sub(1)?;
                    out_path.pop();
                }
                Utf8WindowsComponent::Normal(s) => {
                    depth += 1;
                    out_path.push(s);
                }
                Utf8WindowsComponent::CurDir => (),
            }
        }
        Some(out_path)
    }

    /// Get unix mode for the file
    pub(crate) const fn unix_mode(&self) -> Option<u32> {
        if self.external_attributes == 0 {
            return None;
        }
        let unix_mode = self.external_attributes >> 16;
        if unix_mode != 0 {
            // If the high 16 bits are non-zero, they probably contain Unix permissions.
            // This happens for archives created on Windows by this crate or other tools,
            // and is the only way to identify symlinks in such archives.
            return Some(unix_mode);
        }
        match self.system {
            System::Unix => Some(unix_mode),
            System::Dos => {
                // Interpret MS-DOS directory bit
                let mut mode = if 0x10 == (self.external_attributes & 0x10) {
                    ffi::S_IFDIR | 0o0775
                } else {
                    ffi::S_IFREG | 0o0664
                };
                if 0x01 == (self.external_attributes & 0x01) {
                    // Read-only bit; strip write permissions
                    mode &= !0o222;
                }
                Some(mode)
            }
            _ => None,
        }
    }

    /// PKZIP version needed to open this file (from APPNOTE 4.4.3.2).
    pub fn version_needed(&self) -> u16 {
        let compression_version: u16 = match self.compression_method {
            CompressionMethod::Stored => MIN_VERSION.into(),
            #[cfg(feature = "_deflate-any")]
            CompressionMethod::Deflated => 20,
            #[cfg(feature = "_bzip2_any")]
            CompressionMethod::Bzip2 => 46,
            #[cfg(feature = "deflate64")]
            CompressionMethod::Deflate64 => 21,
            #[cfg(feature = "lzma")]
            CompressionMethod::Lzma => 63,
            #[cfg(feature = "xz")]
            CompressionMethod::Xz => 63,
            // APPNOTE doesn't specify a version for Zstandard
            _ => DEFAULT_VERSION as u16,
        };
        let crypto_version: u16 = if self.aes_mode.is_some() {
            51
        } else if self.encrypted {
            20
        } else {
            10
        };
        let misc_feature_version: u16 = if self.large_file {
            45
        } else if self
            .unix_mode()
            .is_some_and(|mode| mode & S_IFDIR == S_IFDIR)
        {
            // file is directory
            20
        } else {
            10
        };
        compression_version
            .max(crypto_version)
            .max(misc_feature_version)
    }
    #[inline(always)]
    pub(crate) fn extra_field_len(&self) -> usize {
        self.extra_field
            .as_ref()
            .map(|v| v.len())
            .unwrap_or_default()
    }
    #[inline(always)]
    pub(crate) fn central_extra_field_len(&self) -> usize {
        self.central_extra_field
            .as_ref()
            .map(|v| v.len())
            .unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn initialize_local_block<S, T: FileOptionExtension>(
        name: S,
        options: &FileOptions<T>,
        raw_values: ZipRawValues,
        header_start: u64,
        extra_data_start: Option<u64>,
        aes_extra_data_start: u64,
        compression_method: crate::compression::CompressionMethod,
        aes_mode: Option<(AesMode, AesVendorVersion, CompressionMethod)>,
        extra_field: &[u8],
    ) -> Self
    where
        S: ToString,
    {
        let permissions = options.permissions.unwrap_or(0o100644);
        let file_name: Box<str> = name.to_string().into_boxed_str();
        let file_name_raw: Box<[u8]> = file_name.bytes().collect();
        let mut external_attributes = permissions << 16;
        let system = if (permissions & ffi::S_IFLNK) == ffi::S_IFLNK {
            System::Unix
        } else if cfg!(windows) {
            if is_dir(&file_name) {
                // DOS directory bit
                external_attributes |= 0x10;
            }
            if options
                .permissions
                .is_some_and(|permissions| permissions & 0o444 == 0)
            {
                // DOS read-only bit
                external_attributes |= 0x01;
            }
            System::Dos
        } else {
            System::Unix
        };
        let mut local_block = ZipFileData {
            system,
            version_made_by: DEFAULT_VERSION,
            flags: 0,
            encrypted: options.encrypt_with.is_some()
                || cfg_if_expr! {
                    #[cfg(feature = "aes-crypto")] => options.aes_mode.is_some(),
                    _ => false
                },
            using_data_descriptor: false,
            is_utf8: !file_name.is_ascii(),
            compression_method,
            compression_level: options.compression_level,
            last_modified_time: Some(options.last_modified_time),
            crc32: raw_values.crc32,
            compressed_size: raw_values.compressed_size,
            uncompressed_size: raw_values.uncompressed_size,
            file_name, // Never used for saving, but used as map key in insert_file_data()
            file_name_raw,
            extra_field: Some(extra_field.to_vec().into()),
            central_extra_field: options.extended_options.central_extra_data().cloned(),
            file_comment: String::with_capacity(0).into_boxed_str(),
            header_start,
            data_start: OnceLock::new(),
            central_header_start: 0,
            external_attributes,
            large_file: options.large_file,
            aes_mode,
            extra_fields: Vec::new(),
            extra_data_start,
            aes_extra_data_start,
        };
        local_block.version_made_by = local_block.version_needed() as u8;
        local_block
    }

    pub(crate) fn from_local_block<R: std::io::Read + ?Sized>(
        block: ZipLocalEntryBlock,
        reader: &mut R,
    ) -> ZipResult<Self> {
        let ZipLocalEntryBlock {
            // magic,
            version_made_by,
            flags,
            compression_method,
            last_mod_time,
            last_mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            file_name_length,
            extra_field_length,
            ..
        } = block;

        let encrypted: bool = flags & 1 == 1;
        if encrypted {
            return Err(ZipError::UnsupportedArchive(
                "Encrypted files are not supported",
            ));
        }

        /* FIXME: these were previously incorrect: add testing! */
        /* flags & (1 << 3) != 0 */
        let using_data_descriptor: bool = flags & (1 << 3) == 1 << 3;
        if using_data_descriptor {
            return Err(ZipError::UnsupportedArchive(
                "The file length is not available in the local header",
            ));
        }

        /* flags & (1 << 1) != 0 */
        let is_utf8: bool = flags & (1 << 11) != 0;
        let compression_method = crate::CompressionMethod::parse_from_u16(compression_method);
        let file_name_length: usize = file_name_length.into();
        let extra_field_length: usize = extra_field_length.into();

        let mut file_name_raw = vec![0u8; file_name_length];
        if let Err(e) = reader.read_exact(&mut file_name_raw) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Err(invalid!("File name extends beyond file boundary"));
            }
            return Err(e.into());
        }
        let mut extra_field = vec![0u8; extra_field_length];
        if let Err(e) = reader.read_exact(&mut extra_field) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Err(invalid!("Extra field extends beyond file boundary"));
            }
            return Err(e.into());
        }

        let file_name: Box<str> = match is_utf8 {
            true => String::from_utf8_lossy(&file_name_raw).into(),
            false => file_name_raw.clone().from_cp437().into(),
        };

        let system: u8 = (version_made_by >> 8).try_into().unwrap();
        Ok(ZipFileData {
            system: System::from(system),
            /* NB: this strips the top 8 bits! */
            version_made_by: version_made_by as u8,
            flags,
            encrypted,
            using_data_descriptor,
            is_utf8,
            compression_method,
            compression_level: None,
            last_modified_time: DateTime::try_from_msdos(last_mod_date, last_mod_time).ok(),
            crc32,
            compressed_size: compressed_size.into(),
            uncompressed_size: uncompressed_size.into(),
            file_name,
            file_name_raw: file_name_raw.into(),
            extra_field: Some(Arc::new(extra_field)),
            central_extra_field: None,
            file_comment: String::with_capacity(0).into_boxed_str(), // file comment is only available in the central directory
            // header_start and data start are not available, but also don't matter, since seeking is
            // not available.
            header_start: 0,
            data_start: OnceLock::new(),
            central_header_start: 0,
            // The external_attributes field is only available in the central directory.
            // We set this to zero, which should be valid as the docs state 'If input came
            // from standard input, this field is set to zero.'
            external_attributes: 0,
            large_file: false,
            aes_mode: None,
            extra_fields: Vec::new(),
            extra_data_start: None,
            aes_extra_data_start: 0,
        })
    }

    fn is_utf8(&self) -> bool {
        std::str::from_utf8(&self.file_name_raw).is_ok()
    }

    fn is_ascii(&self) -> bool {
        self.file_name_raw.is_ascii()
    }

    fn flags(&self) -> u16 {
        let utf8_bit: u16 = if self.is_utf8() && !self.is_ascii() {
            1u16 << 11
        } else {
            0
        };

        let using_data_descriptor_bit = if self.using_data_descriptor {
            1u16 << 3
        } else {
            0
        };

        let encrypted_bit: u16 = if self.encrypted { 1u16 << 0 } else { 0 };

        utf8_bit | using_data_descriptor_bit | encrypted_bit
    }

    fn clamp_size_field(&self, field: u64) -> u32 {
        if self.large_file {
            spec::ZIP64_BYTES_THR as u32
        } else {
            field.min(spec::ZIP64_BYTES_THR).try_into().unwrap()
        }
    }

    pub(crate) fn local_block(&self) -> ZipResult<ZipLocalEntryBlock> {
        let (compressed_size, uncompressed_size) = if self.using_data_descriptor {
            (0, 0)
        } else {
            (
                self.clamp_size_field(self.compressed_size),
                self.clamp_size_field(self.uncompressed_size),
            )
        };
        let extra_field_length: u16 = self
            .extra_field_len()
            .try_into()
            .map_err(|_| invalid!("Extra data field is too large"))?;

        let last_modified_time = self
            .last_modified_time
            .unwrap_or_else(DateTime::default_for_write);
        Ok(ZipLocalEntryBlock {
            magic: ZipLocalEntryBlock::MAGIC,
            version_made_by: self.version_needed(),
            flags: self.flags(),
            compression_method: self.compression_method.serialize_to_u16(),
            last_mod_time: last_modified_time.timepart(),
            last_mod_date: last_modified_time.datepart(),
            crc32: self.crc32,
            compressed_size,
            uncompressed_size,
            file_name_length: self.file_name_raw.len().try_into().unwrap(),
            extra_field_length,
        })
    }

    pub(crate) fn block(&self) -> ZipResult<ZipCentralEntryBlock> {
        let extra_field_len: u16 = self.extra_field_len().try_into().unwrap();
        let central_extra_field_len: u16 = self.central_extra_field_len().try_into().unwrap();
        let last_modified_time = self
            .last_modified_time
            .unwrap_or_else(DateTime::default_for_write);
        let version_to_extract = self.version_needed();
        let version_made_by = (self.version_made_by as u16).max(version_to_extract);
        Ok(ZipCentralEntryBlock {
            magic: ZipCentralEntryBlock::MAGIC,
            version_made_by: ((self.system as u16) << 8) | version_made_by,
            version_to_extract,
            flags: self.flags(),
            compression_method: self.compression_method.serialize_to_u16(),
            last_mod_time: last_modified_time.timepart(),
            last_mod_date: last_modified_time.datepart(),
            crc32: self.crc32,
            compressed_size: self
                .compressed_size
                .min(spec::ZIP64_BYTES_THR)
                .try_into()
                .unwrap(),
            uncompressed_size: self
                .uncompressed_size
                .min(spec::ZIP64_BYTES_THR)
                .try_into()
                .unwrap(),
            file_name_length: self.file_name_raw.len().try_into().unwrap(),
            extra_field_length: extra_field_len.checked_add(central_extra_field_len).ok_or(
                invalid!("Extra field length in central directory exceeds 64KiB"),
            )?,
            file_comment_length: self.file_comment.len().try_into().unwrap(),
            disk_number: 0,
            internal_file_attributes: 0,
            external_file_attributes: self.external_attributes,
            offset: self
                .header_start
                .min(spec::ZIP64_BYTES_THR)
                .try_into()
                .unwrap(),
        })
    }

    pub(crate) fn zip64_extra_field_block(&self) -> Option<Zip64ExtraFieldBlock> {
        Zip64ExtraFieldBlock::maybe_new(
            self.large_file,
            self.uncompressed_size,
            self.compressed_size,
            self.header_start,
        )
    }

    pub(crate) fn write_data_descriptor<W: std::io::Write>(
        &self,
        writer: &mut W,
        auto_large_file: bool,
    ) -> Result<(), ZipError> {
        if self.large_file {
            return self.zip64_data_descriptor_block().write(writer);
        }
        if self.compressed_size > spec::ZIP64_BYTES_THR
            || self.uncompressed_size > spec::ZIP64_BYTES_THR
        {
            if auto_large_file {
                return self.zip64_data_descriptor_block().write(writer);
            }
            return Err(ZipError::Io(std::io::Error::other(
                "Large file option has not been set - use .large_file(true) in options",
            )));
        }
        self.data_descriptor_block().write(writer)
    }

    pub(crate) fn data_descriptor_block(&self) -> ZipDataDescriptorBlock {
        ZipDataDescriptorBlock {
            magic: ZipDataDescriptorBlock::MAGIC,
            crc32: self.crc32,
            compressed_size: self.compressed_size as u32,
            uncompressed_size: self.uncompressed_size as u32,
        }
    }

    pub(crate) fn zip64_data_descriptor_block(&self) -> Zip64DataDescriptorBlock {
        Zip64DataDescriptorBlock {
            magic: Zip64DataDescriptorBlock::MAGIC,
            crc32: self.crc32,
            compressed_size: self.compressed_size,
            uncompressed_size: self.uncompressed_size,
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(packed, C)]
pub(crate) struct ZipCentralEntryBlock {
    magic: spec::Magic,
    pub version_made_by: u16,
    pub version_to_extract: u16,
    pub flags: u16,
    pub compression_method: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub file_name_length: u16,
    pub extra_field_length: u16,
    pub file_comment_length: u16,
    pub disk_number: u16,
    pub internal_file_attributes: u16,
    pub external_file_attributes: u32,
    pub offset: u32,
}

unsafe impl Pod for ZipCentralEntryBlock {}

impl FixedSizeBlock for ZipCentralEntryBlock {
    const MAGIC: spec::Magic = spec::Magic::CENTRAL_DIRECTORY_HEADER_SIGNATURE;

    #[inline(always)]
    fn magic(self) -> spec::Magic {
        self.magic
    }

    const WRONG_MAGIC_ERROR: ZipError = invalid!("Invalid Central Directory header");

    to_and_from_le![
        (magic, spec::Magic),
        (version_made_by, u16),
        (version_to_extract, u16),
        (flags, u16),
        (compression_method, u16),
        (last_mod_time, u16),
        (last_mod_date, u16),
        (crc32, u32),
        (compressed_size, u32),
        (uncompressed_size, u32),
        (file_name_length, u16),
        (extra_field_length, u16),
        (file_comment_length, u16),
        (disk_number, u16),
        (internal_file_attributes, u16),
        (external_file_attributes, u32),
        (offset, u32),
    ];
}

#[derive(Copy, Clone, Debug)]
#[repr(packed, C)]
pub(crate) struct ZipLocalEntryBlock {
    magic: spec::Magic,
    pub version_made_by: u16,
    pub flags: u16,
    pub compression_method: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub file_name_length: u16,
    pub extra_field_length: u16,
}

unsafe impl Pod for ZipLocalEntryBlock {}

impl FixedSizeBlock for ZipLocalEntryBlock {
    const MAGIC: spec::Magic = spec::Magic::LOCAL_FILE_HEADER_SIGNATURE;

    #[inline(always)]
    fn magic(self) -> spec::Magic {
        self.magic
    }

    const WRONG_MAGIC_ERROR: ZipError = invalid!("Invalid local file header");

    to_and_from_le![
        (magic, spec::Magic),
        (version_made_by, u16),
        (flags, u16),
        (compression_method, u16),
        (last_mod_time, u16),
        (last_mod_date, u16),
        (crc32, u32),
        (compressed_size, u32),
        (uncompressed_size, u32),
        (file_name_length, u16),
        (extra_field_length, u16),
    ];
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Zip64ExtraFieldBlock {
    magic: spec::ExtraFieldMagic,
    size: u16,
    uncompressed_size: Option<u64>,
    compressed_size: Option<u64>,
    header_start: Option<u64>,
    // Excluded fields:
    // u32: disk start number
}

impl Zip64ExtraFieldBlock {
    pub(crate) fn maybe_new(
        large_file: bool,
        uncompressed_size: u64,
        compressed_size: u64,
        header_start: u64,
    ) -> Option<Zip64ExtraFieldBlock> {
        let mut size: u16 = 0;
        let uncompressed_size = if uncompressed_size >= ZIP64_BYTES_THR || large_file {
            size += mem::size_of::<u64>() as u16;
            Some(uncompressed_size)
        } else {
            None
        };
        let compressed_size = if compressed_size >= ZIP64_BYTES_THR || large_file {
            size += mem::size_of::<u64>() as u16;
            Some(compressed_size)
        } else {
            None
        };
        let header_start = if header_start >= ZIP64_BYTES_THR {
            size += mem::size_of::<u64>() as u16;
            Some(header_start)
        } else {
            None
        };
        if size == 0 {
            return None;
        }

        Some(Zip64ExtraFieldBlock {
            magic: spec::ExtraFieldMagic::ZIP64_EXTRA_FIELD_TAG,
            size,
            uncompressed_size,
            compressed_size,
            header_start,
        })
    }
}

impl Zip64ExtraFieldBlock {
    pub fn full_size(&self) -> usize {
        assert!(self.size > 0);
        self.size as usize + mem::size_of::<spec::ExtraFieldMagic>() + mem::size_of::<u16>()
    }

    pub fn serialize(self) -> Box<[u8]> {
        let Self {
            magic,
            size,
            uncompressed_size,
            compressed_size,
            header_start,
        } = self;

        let full_size = self.full_size();

        let mut ret = Vec::with_capacity(full_size);
        ret.extend(magic.to_le_bytes());
        ret.extend(u16::to_le_bytes(size));

        if let Some(uncompressed_size) = uncompressed_size {
            ret.extend(u64::to_le_bytes(uncompressed_size));
        }
        if let Some(compressed_size) = compressed_size {
            ret.extend(u64::to_le_bytes(compressed_size));
        }
        if let Some(header_start) = header_start {
            ret.extend(u64::to_le_bytes(header_start));
        }
        debug_assert_eq!(ret.len(), full_size);

        ret.into_boxed_slice()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(packed, C)]
pub(crate) struct ZipDataDescriptorBlock {
    magic: spec::Magic,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}

unsafe impl Pod for ZipDataDescriptorBlock {}

impl FixedSizeBlock for ZipDataDescriptorBlock {
    const MAGIC: spec::Magic = spec::Magic::DATA_DESCRIPTOR_SIGNATURE;

    #[inline(always)]
    fn magic(self) -> spec::Magic {
        self.magic
    }

    const WRONG_MAGIC_ERROR: ZipError = invalid!("Invalid data descriptor header");

    to_and_from_le![
        (magic, spec::Magic),
        (crc32, u32),
        (compressed_size, u32),
        (uncompressed_size, u32),
    ];
}

#[derive(Copy, Clone, Debug)]
#[repr(packed, C)]
pub(crate) struct Zip64DataDescriptorBlock {
    magic: spec::Magic,
    pub crc32: u32,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
}

unsafe impl Pod for Zip64DataDescriptorBlock {}

impl FixedSizeBlock for Zip64DataDescriptorBlock {
    const MAGIC: spec::Magic = spec::Magic::DATA_DESCRIPTOR_SIGNATURE;

    #[inline(always)]
    fn magic(self) -> spec::Magic {
        self.magic
    }

    const WRONG_MAGIC_ERROR: ZipError = invalid!("Invalid zip64 data descriptor header");

    to_and_from_le![
        (magic, spec::Magic),
        (crc32, u32),
        (compressed_size, u64),
        (uncompressed_size, u64),
    ];
}

/// The encryption specification used to encrypt a file with AES.
///
/// According to the [specification](https://www.winzip.com/win/en/aes_info.html#winzip11) AE-2
/// does not make use of the CRC check.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum AesVendorVersion {
    Ae1 = 0x0001,
    Ae2 = 0x0002,
}

/// AES variant used.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "_arbitrary", derive(arbitrary::Arbitrary))]
#[repr(u8)]
pub enum AesMode {
    /// 128-bit AES encryption.
    Aes128 = 0x01,
    /// 192-bit AES encryption.
    Aes192 = 0x02,
    /// 256-bit AES encryption.
    Aes256 = 0x03,
}

#[cfg(feature = "aes-crypto")]
impl AesMode {
    /// Length of the salt for the given AES mode.
    pub const fn salt_length(&self) -> usize {
        self.key_length() / 2
    }

    /// Length of the key for the given AES mode.
    pub const fn key_length(&self) -> usize {
        match self {
            Self::Aes128 => 16,
            Self::Aes192 => 24,
            Self::Aes256 => 32,
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn system() {
        use super::System;
        assert_eq!(u8::from(System::Dos), 0u8);
        assert_eq!(System::Dos as u8, 0u8);
        assert_eq!(System::Unix as u8, 3u8);
        assert_eq!(u8::from(System::Unix), 3u8);
        assert_eq!(System::from(0), System::Dos);
        assert_eq!(System::from(3), System::Unix);
        assert_eq!(u8::from(System::Unknown), 4u8);
        assert_eq!(System::Unknown as u8, 4u8);
    }

    #[test]
    fn unix_mode_robustness() {
        use super::{System, ZipFileData};
        use crate::types::ffi::S_IFLNK;
        let mut data = ZipFileData {
            system: System::Dos,
            external_attributes: (S_IFLNK | 0o777) << 16,
            ..ZipFileData::default()
        };
        assert_eq!(data.unix_mode(), Some(S_IFLNK | 0o777));

        data.system = System::Unknown;
        assert_eq!(data.unix_mode(), Some(S_IFLNK | 0o777));

        data.external_attributes = 0x10; // DOS directory bit
        data.system = System::Dos;
        assert_eq!(
            data.unix_mode().unwrap() & 0o170000,
            crate::types::ffi::S_IFDIR
        );
    }

    #[test]
    fn sanitize() {
        let file_name = "/path/../../../../etc/./passwd\0/etc/shadow".to_string();
        let data = ZipFileData {
            system: System::Dos,
            version_made_by: 0,
            flags: 0,
            encrypted: false,
            using_data_descriptor: false,
            is_utf8: true,
            compression_method: crate::compression::CompressionMethod::Stored,
            compression_level: None,
            last_modified_time: None,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            file_name: file_name.clone().into_boxed_str(),
            file_name_raw: file_name.into_bytes().into_boxed_slice(),
            extra_field: None,
            central_extra_field: None,
            file_comment: String::with_capacity(0).into_boxed_str(),
            header_start: 0,
            extra_data_start: None,
            data_start: OnceLock::new(),
            central_header_start: 0,
            external_attributes: 0,
            large_file: false,
            aes_mode: None,
            aes_extra_data_start: 0,
            extra_fields: Vec::new(),
        };
        assert_eq!(data.file_name_sanitized(), PathBuf::from("path/etc/passwd"));
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn datetime_default() {
        use super::DateTime;
        let dt = DateTime::default();
        assert_eq!(dt.timepart(), 0);
        assert_eq!(dt.datepart(), 0b0000000_0001_00001);
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn datetime_max() {
        use super::DateTime;
        let dt = DateTime::from_date_and_time(2107, 12, 31, 23, 59, 58).unwrap();
        assert_eq!(dt.timepart(), 0b10111_111011_11101);
        assert_eq!(dt.datepart(), 0b1111111_1100_11111);
    }

    #[test]
    fn datetime_equality() {
        use super::DateTime;

        let dt = DateTime::from_date_and_time(2018, 11, 17, 10, 38, 30).unwrap();
        assert_eq!(
            dt,
            DateTime::from_date_and_time(2018, 11, 17, 10, 38, 30).unwrap()
        );
        assert_ne!(dt, DateTime::default());
    }

    #[test]
    fn datetime_order() {
        use std::cmp::Ordering;

        use super::DateTime;

        let dt = DateTime::from_date_and_time(2018, 11, 17, 10, 38, 30).unwrap();
        assert_eq!(
            dt.cmp(&DateTime::from_date_and_time(2018, 11, 17, 10, 38, 30).unwrap()),
            Ordering::Equal
        );
        // year
        assert!(dt < DateTime::from_date_and_time(2019, 11, 17, 10, 38, 30).unwrap());
        assert!(dt > DateTime::from_date_and_time(2017, 11, 17, 10, 38, 30).unwrap());
        // month
        assert!(dt < DateTime::from_date_and_time(2018, 12, 17, 10, 38, 30).unwrap());
        assert!(dt > DateTime::from_date_and_time(2018, 10, 17, 10, 38, 30).unwrap());
        // day
        assert!(dt < DateTime::from_date_and_time(2018, 11, 18, 10, 38, 30).unwrap());
        assert!(dt > DateTime::from_date_and_time(2018, 11, 16, 10, 38, 30).unwrap());
        // hour
        assert!(dt < DateTime::from_date_and_time(2018, 11, 17, 11, 38, 30).unwrap());
        assert!(dt > DateTime::from_date_and_time(2018, 11, 17, 9, 38, 30).unwrap());
        // minute
        assert!(dt < DateTime::from_date_and_time(2018, 11, 17, 10, 39, 30).unwrap());
        assert!(dt > DateTime::from_date_and_time(2018, 11, 17, 10, 37, 30).unwrap());
        // second
        assert!(dt < DateTime::from_date_and_time(2018, 11, 17, 10, 38, 32).unwrap());
        assert_eq!(
            dt.cmp(&DateTime::from_date_and_time(2018, 11, 17, 10, 38, 31).unwrap()),
            Ordering::Equal
        );
        assert!(dt > DateTime::from_date_and_time(2018, 11, 17, 10, 38, 29).unwrap());
        assert!(dt > DateTime::from_date_and_time(2018, 11, 17, 10, 38, 28).unwrap());
    }

    #[test]
    fn datetime_display() {
        use super::DateTime;

        assert_eq!(format!("{}", DateTime::default()), "1980-01-01 00:00:00");
        assert_eq!(
            format!(
                "{}",
                DateTime::from_date_and_time(2018, 11, 17, 10, 38, 30).unwrap()
            ),
            "2018-11-17 10:38:30"
        );
        assert_eq!(
            format!(
                "{}",
                DateTime::from_date_and_time(2107, 12, 31, 23, 59, 58).unwrap()
            ),
            "2107-12-31 23:59:58"
        );
    }

    #[test]
    fn datetime_bounds() {
        use super::DateTime;

        assert!(DateTime::from_date_and_time(2000, 1, 1, 23, 59, 60).is_ok());
        assert!(DateTime::from_date_and_time(2000, 1, 1, 24, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2000, 1, 1, 0, 60, 0).is_err());
        assert!(DateTime::from_date_and_time(2000, 1, 1, 0, 0, 61).is_err());

        assert!(DateTime::from_date_and_time(2107, 12, 31, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(1979, 1, 1, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(1980, 0, 1, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(1980, 1, 0, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2108, 12, 31, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2107, 13, 31, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2107, 12, 32, 0, 0, 0).is_err());

        assert!(DateTime::from_date_and_time(2018, 1, 31, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 2, 28, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 2, 29, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2018, 3, 31, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 4, 30, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 4, 31, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2018, 5, 31, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 6, 30, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 6, 31, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2018, 7, 31, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 8, 31, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 9, 30, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 9, 31, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2018, 10, 31, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 11, 30, 0, 0, 0).is_ok());
        assert!(DateTime::from_date_and_time(2018, 11, 31, 0, 0, 0).is_err());
        assert!(DateTime::from_date_and_time(2018, 12, 31, 0, 0, 0).is_ok());

        // leap year: divisible by 4
        assert!(DateTime::from_date_and_time(2024, 2, 29, 0, 0, 0).is_ok());
        // leap year: divisible by 100 and by 400
        assert!(DateTime::from_date_and_time(2000, 2, 29, 0, 0, 0).is_ok());
        // common year: divisible by 100 but not by 400
        assert!(DateTime::from_date_and_time(2100, 2, 29, 0, 0, 0).is_err());
    }

    use std::{path::PathBuf, sync::OnceLock};

    #[cfg(feature = "time")]
    use time::{format_description::well_known::Rfc3339, OffsetDateTime, PrimitiveDateTime};

    use crate::types::{System, ZipFileData};

    #[cfg(feature = "time")]
    #[test]
    fn datetime_try_from_offset_datetime() {
        use time::macros::datetime;

        use super::DateTime;

        // 2018-11-17 10:38:30
        let dt = DateTime::try_from(datetime!(2018-11-17 10:38:30 UTC)).unwrap();
        assert_eq!(dt.year(), 2018);
        assert_eq!(dt.month(), 11);
        assert_eq!(dt.day(), 17);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 38);
        assert_eq!(dt.second(), 30);
    }

    #[cfg(feature = "time")]
    #[test]
    fn datetime_try_from_primitive_datetime() {
        use time::macros::datetime;

        use super::DateTime;

        // 2018-11-17 10:38:30
        let dt = DateTime::try_from(datetime!(2018-11-17 10:38:30)).unwrap();
        assert_eq!(dt.year(), 2018);
        assert_eq!(dt.month(), 11);
        assert_eq!(dt.day(), 17);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 38);
        assert_eq!(dt.second(), 30);
    }

    #[cfg(feature = "time")]
    #[test]
    fn datetime_try_from_bounds() {
        use super::DateTime;
        use time::macros::datetime;

        // 1979-12-31 23:59:59
        assert!(DateTime::try_from(datetime!(1979-12-31 23:59:59)).is_err());

        // 1980-01-01 00:00:00
        assert!(DateTime::try_from(datetime!(1980-01-01 00:00:00)).is_ok());

        // 2107-12-31 23:59:59
        assert!(DateTime::try_from(datetime!(2107-12-31 23:59:59)).is_ok());

        // 2108-01-01 00:00:00
        assert!(DateTime::try_from(datetime!(2108-01-01 00:00:00)).is_err());
    }

    #[cfg(feature = "time")]
    #[test]
    fn offset_datetime_try_from_datetime() {
        use time::macros::datetime;

        use super::DateTime;

        // 2018-11-17 10:38:30 UTC
        let dt =
            OffsetDateTime::try_from(DateTime::try_from_msdos(0x4D71, 0x54CF).unwrap()).unwrap();
        assert_eq!(dt, datetime!(2018-11-17 10:38:30 UTC));
    }

    #[cfg(feature = "time")]
    #[test]
    fn primitive_datetime_try_from_datetime() {
        use time::macros::datetime;

        use super::DateTime;

        // 2018-11-17 10:38:30
        let dt =
            PrimitiveDateTime::try_from(DateTime::try_from_msdos(0x4D71, 0x54CF).unwrap()).unwrap();
        assert_eq!(dt, datetime!(2018-11-17 10:38:30));
    }

    #[cfg(feature = "time")]
    #[test]
    fn offset_datetime_try_from_bounds() {
        use super::DateTime;

        // 1980-00-00 00:00:00
        assert!(OffsetDateTime::try_from(unsafe {
            DateTime::from_msdos_unchecked(0x0000, 0x0000)
        })
        .is_err());

        // 2107-15-31 31:63:62
        assert!(OffsetDateTime::try_from(unsafe {
            DateTime::from_msdos_unchecked(0xFFFF, 0xFFFF)
        })
        .is_err());
    }

    #[cfg(feature = "time")]
    #[test]
    fn primitive_datetime_try_from_bounds() {
        use super::DateTime;

        // 1980-00-00 00:00:00
        assert!(PrimitiveDateTime::try_from(unsafe {
            DateTime::from_msdos_unchecked(0x0000, 0x0000)
        })
        .is_err());

        // 2107-15-31 31:63:62
        assert!(PrimitiveDateTime::try_from(unsafe {
            DateTime::from_msdos_unchecked(0xFFFF, 0xFFFF)
        })
        .is_err());
    }

    #[cfg(feature = "jiff-02")]
    #[test]
    fn datetime_try_from_civil_datetime() {
        use jiff::civil;

        use super::DateTime;

        // 2018-11-17 10:38:30
        let dt = DateTime::try_from(civil::datetime(2018, 11, 17, 10, 38, 30, 0)).unwrap();
        assert_eq!(dt.year(), 2018);
        assert_eq!(dt.month(), 11);
        assert_eq!(dt.day(), 17);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 38);
        assert_eq!(dt.second(), 30);
    }

    #[cfg(feature = "jiff-02")]
    #[test]
    fn datetime_try_from_civil_datetime_bounds() {
        use jiff::civil;

        use super::DateTime;

        // 1979-12-31 23:59:59
        assert!(DateTime::try_from(civil::datetime(1979, 12, 31, 23, 59, 59, 0)).is_err());

        // 1980-01-01 00:00:00
        assert!(DateTime::try_from(civil::datetime(1980, 1, 1, 0, 0, 0, 0)).is_ok());

        // 2107-12-31 23:59:59
        assert!(DateTime::try_from(civil::datetime(2107, 12, 31, 23, 59, 59, 0)).is_ok());

        // 2108-01-01 00:00:00
        assert!(DateTime::try_from(civil::datetime(2108, 1, 1, 0, 0, 0, 0)).is_err());
    }

    #[cfg(feature = "jiff-02")]
    #[test]
    fn civil_datetime_try_from_datetime() {
        use jiff::civil;

        use super::DateTime;

        // 2018-11-17 10:38:30 UTC
        let dt =
            civil::DateTime::try_from(DateTime::try_from_msdos(0x4D71, 0x54CF).unwrap()).unwrap();
        assert_eq!(dt, civil::datetime(2018, 11, 17, 10, 38, 30, 0));
    }

    #[cfg(feature = "jiff-02")]
    #[test]
    fn civil_datetime_try_from_datetime_bounds() {
        use jiff::civil;

        use super::DateTime;

        // 1980-00-00 00:00:00
        assert!(civil::DateTime::try_from(unsafe {
            DateTime::from_msdos_unchecked(0x0000, 0x0000)
        })
        .is_err());

        // 2107-15-31 31:63:62
        assert!(civil::DateTime::try_from(unsafe {
            DateTime::from_msdos_unchecked(0xFFFF, 0xFFFF)
        })
        .is_err());
    }

    #[test]
    #[allow(deprecated)]
    fn time_conversion() {
        use super::DateTime;
        let dt = DateTime::try_from_msdos(0x4D71, 0x54CF).unwrap();
        assert_eq!(dt.year(), 2018);
        assert_eq!(dt.month(), 11);
        assert_eq!(dt.day(), 17);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 38);
        assert_eq!(dt.second(), 30);

        let dt = DateTime::try_from((0x4D71, 0x54CF)).unwrap();
        assert_eq!(dt.year(), 2018);
        assert_eq!(dt.month(), 11);
        assert_eq!(dt.day(), 17);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 38);
        assert_eq!(dt.second(), 30);

        #[cfg(feature = "time")]
        assert_eq!(
            dt.to_time().unwrap().format(&Rfc3339).unwrap(),
            "2018-11-17T10:38:30Z"
        );

        assert_eq!(<(u16, u16)>::from(dt), (0x4D71, 0x54CF));
    }

    #[test]
    #[allow(deprecated)]
    fn time_out_of_bounds() {
        use super::DateTime;
        let dt = unsafe { DateTime::from_msdos_unchecked(0xFFFF, 0xFFFF) };
        assert_eq!(dt.year(), 2107);
        assert_eq!(dt.month(), 15);
        assert_eq!(dt.day(), 31);
        assert_eq!(dt.hour(), 31);
        assert_eq!(dt.minute(), 63);
        assert_eq!(dt.second(), 62);

        #[cfg(feature = "time")]
        assert!(dt.to_time().is_err());

        let dt = unsafe { DateTime::from_msdos_unchecked(0x0000, 0x0000) };
        assert_eq!(dt.year(), 1980);
        assert_eq!(dt.month(), 0);
        assert_eq!(dt.day(), 0);
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);

        #[cfg(feature = "time")]
        assert!(dt.to_time().is_err());
    }

    #[cfg(feature = "time")]
    #[test]
    fn time_at_january() {
        use super::DateTime;

        // 2020-01-01 00:00:00
        let clock = OffsetDateTime::from_unix_timestamp(1_577_836_800).unwrap();

        assert!(DateTime::try_from(PrimitiveDateTime::new(clock.date(), clock.time())).is_ok());
    }
}
