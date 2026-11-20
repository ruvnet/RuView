//! Types for reading ZIP archives

/// Upper bound on the uncompressed size of a symlink entry.
///
/// A symlink target is a filesystem path, so its length is bounded by the
/// platform (`PATH_MAX` is 4096 on Linux) no matter what the archive says. The
/// uncompressed size stored in the central directory is attacker-controlled,
/// so entries above this bound are rejected rather than pre-allocated.
pub(crate) const MAX_SYMLINK_TARGET_LEN: u64 = 4096;

#[cfg(feature = "aes-crypto")]
use crate::aes::{AesReader, AesReaderValid};
use crate::cfg_if;
use crate::compression::{CompressionMethod, Decompressor};
use crate::cp437::FromCp437;
use crate::crc32::Crc32Reader;
use crate::extra_fields::{ExtendedTimestamp, ExtraField, Ntfs};
use crate::result::{invalid, ZipError, ZipResult};
use crate::spec::{
    self, CentralDirectoryEndInfo, DataAndPosition, FixedSizeBlock, Pod, ZIP64_BYTES_THR,
};
use crate::types::{
    AesMode, AesVendorVersion, DateTime, SimpleFileOptions, System, ZipCentralEntryBlock,
    ZipFileData, ZipLocalEntryBlock,
};
use crate::zipcrypto::{ZipCryptoReader, ZipCryptoReaderValid, ZipCryptoValidator};
use core::mem::{replace, size_of};
use core::ops::{Deref, Range};
use indexmap::IndexMap;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::io::{self, copy, sink, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

mod config;

pub use config::{ArchiveOffset, Config};

/// Provides high level API for reading from a stream.
pub(crate) mod stream;

pub(crate) mod magic_finder;

/// Immutable metadata about a `ZipArchive`.
#[derive(Debug)]
pub struct ZipArchiveMetadata {
    pub(crate) files: IndexMap<Box<str>, ZipFileData>,
    pub(crate) offset: u64,
    pub(crate) dir_start: u64,
    // This isn't yet used anywhere, but it is here for use cases in the future.
    #[allow(dead_code)]
    pub(crate) config: Config,
    pub(crate) comment: Box<[u8]>,
    pub(crate) zip64_comment: Option<Box<[u8]>>,
}

pub(crate) mod zip_archive {
    use crate::read::ZipArchiveMetadata;
    use indexmap::IndexMap;
    use std::sync::Arc;

    #[derive(Debug)]
    pub(crate) struct SharedBuilder {
        pub(crate) files: Vec<super::ZipFileData>,
        pub(super) offset: u64,
        pub(super) dir_start: u64,
        // This isn't yet used anywhere, but it is here for use cases in the future.
        #[allow(dead_code)]
        pub(super) config: super::Config,
    }

    impl SharedBuilder {
        pub fn build(
            self,
            comment: Box<[u8]>,
            zip64_comment: Option<Box<[u8]>>,
        ) -> ZipArchiveMetadata {
            let mut index_map = IndexMap::with_capacity(self.files.len());
            self.files.into_iter().for_each(|file| {
                index_map.insert(file.file_name.clone(), file);
            });
            ZipArchiveMetadata {
                files: index_map,
                offset: self.offset,
                dir_start: self.dir_start,
                config: self.config,
                comment,
                zip64_comment,
            }
        }
    }

    /// ZIP archive reader
    ///
    /// At the moment, this type is cheap to clone if this is the case for the
    /// reader it uses. However, this is not guaranteed by this crate and it may
    /// change in the future.
    ///
    /// ```no_run
    /// use std::io::{Read, Seek};
    /// fn list_zip_contents(reader: impl Read + Seek) -> zip::result::ZipResult<()> {
    ///     use zip::HasZipMetadata;
    ///     let mut zip = zip::ZipArchive::new(reader)?;
    ///
    ///     for i in 0..zip.len() {
    ///         let mut file = zip.by_index(i)?;
    ///         println!("Filename: {}", file.name());
    ///         std::io::copy(&mut file, &mut std::io::stdout())?;
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    #[derive(Clone, Debug)]
    pub struct ZipArchive<R> {
        pub(super) reader: R,
        pub(super) shared: Arc<ZipArchiveMetadata>,
    }
}

#[cfg(feature = "aes-crypto")]
use crate::aes::PWD_VERIFY_LENGTH;
use crate::extra_fields::UnicodeExtraField;
use crate::result::ZipError::InvalidPassword;
use crate::spec::is_dir;
use crate::types::ffi::{S_IFLNK, S_IFREG};
use crate::unstable::{path_to_string, LittleEndianReadExt};
pub use zip_archive::ZipArchive;

#[allow(clippy::large_enum_variant)]
pub(crate) enum CryptoReader<'a, R: Read + ?Sized> {
    Plaintext(io::Take<&'a mut R>),
    ZipCrypto(ZipCryptoReaderValid<io::Take<&'a mut R>>),
    #[cfg(feature = "aes-crypto")]
    Aes {
        reader: AesReaderValid<io::Take<&'a mut R>>,
        vendor_version: AesVendorVersion,
    },
}

impl<R: Read + ?Sized> Read for CryptoReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            CryptoReader::Plaintext(r) => r.read(buf),
            CryptoReader::ZipCrypto(r) => r.read(buf),
            #[cfg(feature = "aes-crypto")]
            CryptoReader::Aes { reader: r, .. } => r.read(buf),
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        match self {
            CryptoReader::Plaintext(r) => r.read_to_end(buf),
            CryptoReader::ZipCrypto(r) => r.read_to_end(buf),
            #[cfg(feature = "aes-crypto")]
            CryptoReader::Aes { reader: r, .. } => r.read_to_end(buf),
        }
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        match self {
            CryptoReader::Plaintext(r) => r.read_to_string(buf),
            CryptoReader::ZipCrypto(r) => r.read_to_string(buf),
            #[cfg(feature = "aes-crypto")]
            CryptoReader::Aes { reader: r, .. } => r.read_to_string(buf),
        }
    }
}

impl<'a, R: Read + ?Sized> CryptoReader<'a, R> {
    /// Consumes this decoder, returning the underlying reader.
    pub fn into_inner(self) -> io::Take<&'a mut R> {
        match self {
            CryptoReader::Plaintext(r) => r,
            CryptoReader::ZipCrypto(r) => r.into_inner(),
            #[cfg(feature = "aes-crypto")]
            CryptoReader::Aes { reader: r, .. } => r.into_inner(),
        }
    }

    /// Returns `true` if the data is encrypted using AE2.
    #[allow(clippy::needless_return)] // can't use cfg_if_expr! because const
    pub const fn is_ae2_encrypted(&self) -> bool {
        cfg_if! {
            if #[cfg(feature = "aes-crypto")] {
                return matches!(
                    self,
                    CryptoReader::Aes {
                        vendor_version: AesVendorVersion::Ae2,
                        ..
                    }
                );
            } else {
                return false;
            }
        }
    }
}

#[cold]
fn invalid_state<T>() -> io::Result<T> {
    Err(io::Error::other("ZipFileReader was in an invalid state"))
}

pub(crate) enum ZipFileReader<'a, R: Read + ?Sized> {
    NoReader,
    Raw(io::Take<&'a mut R>),
    Compressed(Box<Crc32Reader<Decompressor<io::BufReader<CryptoReader<'a, R>>>>>),
}

impl<R: Read + ?Sized> Read for ZipFileReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ZipFileReader::NoReader => invalid_state(),
            ZipFileReader::Raw(r) => r.read(buf),
            ZipFileReader::Compressed(r) => r.read(buf),
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        match self {
            ZipFileReader::NoReader => invalid_state(),
            ZipFileReader::Raw(r) => r.read_exact(buf),
            ZipFileReader::Compressed(r) => r.read_exact(buf),
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        match self {
            ZipFileReader::NoReader => invalid_state(),
            ZipFileReader::Raw(r) => r.read_to_end(buf),
            ZipFileReader::Compressed(r) => r.read_to_end(buf),
        }
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        match self {
            ZipFileReader::NoReader => invalid_state(),
            ZipFileReader::Raw(r) => r.read_to_string(buf),
            ZipFileReader::Compressed(r) => r.read_to_string(buf),
        }
    }
}

impl<'a, R: Read + ?Sized> ZipFileReader<'a, R> {
    fn into_inner(self) -> io::Result<io::Take<&'a mut R>> {
        match self {
            ZipFileReader::NoReader => invalid_state(),
            ZipFileReader::Raw(r) => Ok(r),
            ZipFileReader::Compressed(r) => {
                Ok(r.into_inner().into_inner()?.into_inner().into_inner())
            }
        }
    }
}

/// A struct for reading a zip file
pub struct ZipFile<'a, R: Read + ?Sized> {
    pub(crate) data: Cow<'a, ZipFileData>,
    pub(crate) reader: ZipFileReader<'a, R>,
}

/// A struct for reading and seeking a zip file
pub struct ZipFileSeek<'a, R> {
    data: Cow<'a, ZipFileData>,
    reader: ZipFileSeekReader<'a, R>,
}

enum ZipFileSeekReader<'a, R: ?Sized> {
    Raw(SeekableTake<'a, R>),
}

struct SeekableTake<'a, R: ?Sized> {
    inner: &'a mut R,
    inner_starting_offset: u64,
    length: u64,
    current_offset: u64,
}

impl<'a, R: Seek + ?Sized> SeekableTake<'a, R> {
    pub fn new(inner: &'a mut R, length: u64) -> io::Result<Self> {
        let inner_starting_offset = inner.stream_position()?;
        Ok(Self {
            inner,
            inner_starting_offset,
            length,
            current_offset: 0,
        })
    }
}

impl<R: Seek + ?Sized> Seek for SeekableTake<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let offset = match pos {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(offset) => self.length.checked_add_signed(offset),
            SeekFrom::Current(offset) => self.current_offset.checked_add_signed(offset),
        };
        match offset {
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position",
            )),
            Some(offset) => {
                let clamped_offset = std::cmp::min(self.length, offset);
                let new_inner_offset = self
                    .inner
                    .seek(SeekFrom::Start(self.inner_starting_offset + clamped_offset))?;
                self.current_offset = new_inner_offset - self.inner_starting_offset;
                Ok(self.current_offset)
            }
        }
    }
}

impl<R: Read + ?Sized> Read for SeekableTake<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let written = self
            .inner
            .take(self.length - self.current_offset)
            .read(buf)?;
        self.current_offset += written as u64;
        Ok(written)
    }
}

pub(crate) fn make_writable_dir_all<T: AsRef<Path>>(outpath: T) -> Result<(), ZipError> {
    use std::fs;
    fs::create_dir_all(outpath.as_ref())?;
    #[cfg(unix)]
    {
        // Dirs must be writable until all normal files are extracted
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            outpath.as_ref(),
            std::fs::Permissions::from_mode(
                0o700 | std::fs::metadata(outpath.as_ref())?.permissions().mode(),
            ),
        )?;
    }
    Ok(())
}

pub(crate) fn find_content<'a, R: Read + Seek + ?Sized>(
    data: &ZipFileData,
    reader: &'a mut R,
) -> ZipResult<io::Take<&'a mut R>> {
    // TODO: use .get_or_try_init() once stabilized to provide a closure returning a Result!
    let data_start = data.data_start(reader)?;

    reader.seek(SeekFrom::Start(data_start))?;
    Ok(reader.take(data.compressed_size))
}

fn find_content_seek<'a, R: Read + Seek + ?Sized>(
    data: &ZipFileData,
    reader: &'a mut R,
) -> ZipResult<SeekableTake<'a, R>> {
    // Parse local header
    let data_start = data.data_start(reader)?;
    reader.seek(SeekFrom::Start(data_start))?;

    // Explicit Ok and ? are needed to convert io::Error to ZipError
    Ok(SeekableTake::new(reader, data.compressed_size)?)
}

pub(crate) fn find_data_start(
    data: &ZipFileData,
    reader: &mut (impl Read + Seek + ?Sized),
) -> Result<u64, ZipError> {
    // Go to start of data.
    reader.seek(SeekFrom::Start(data.header_start))?;

    // Parse static-sized fields and check the magic value.
    let block = ZipLocalEntryBlock::parse(reader)?;

    // Calculate the end of the local header from the fields we just parsed.
    let variable_fields_len =
        // Each of these fields must be converted to u64 before adding, as the result may
        // easily overflow a u16.
        block.file_name_length as u64 + block.extra_field_length as u64;
    let data_start =
        data.header_start + size_of::<ZipLocalEntryBlock>() as u64 + variable_fields_len;

    // Set the value so we don't have to read it again.
    match data.data_start.set(data_start) {
        Ok(()) => (),
        // If the value was already set in the meantime, ensure it matches (this is probably
        // unnecessary).
        Err(_) => {
            debug_assert_eq!(*data.data_start.get().unwrap(), data_start);
        }
    }

    Ok(data_start)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_crypto_reader<'a, R: Read + ?Sized>(
    data: &ZipFileData,
    reader: io::Take<&'a mut R>,
    password: Option<&[u8]>,
    aes_info: Option<(AesMode, AesVendorVersion, CompressionMethod)>,
) -> ZipResult<CryptoReader<'a, R>> {
    #[allow(deprecated)]
    {
        if let CompressionMethod::Unsupported(_) = data.compression_method {
            return unsupported_zip_error("Compression method not supported");
        }
    }

    let reader = match (password, aes_info) {
        #[cfg(not(feature = "aes-crypto"))]
        (Some(_), Some(_)) => {
            return Err(ZipError::UnsupportedArchive(
                "AES encrypted files cannot be decrypted without the aes-crypto feature.",
            ))
        }
        #[cfg(feature = "aes-crypto")]
        (Some(password), Some((aes_mode, vendor_version, _))) => CryptoReader::Aes {
            reader: AesReader::new(reader, aes_mode, data.compressed_size).validate(password)?,
            vendor_version,
        },
        (Some(password), None) => {
            let validator = if data.using_data_descriptor {
                ZipCryptoValidator::InfoZipMsdosTime(
                    data.last_modified_time.map_or(0, |x| x.timepart()),
                )
            } else {
                ZipCryptoValidator::PkzipCrc32(data.crc32)
            };
            CryptoReader::ZipCrypto(ZipCryptoReader::new(reader, password).validate(validator)?)
        }
        (None, Some(_)) => return Err(InvalidPassword),
        (None, None) => CryptoReader::Plaintext(reader),
    };
    Ok(reader)
}

pub(crate) fn make_reader<'a, R: Read + ?Sized>(
    compression_method: CompressionMethod,
    uncompressed_size: u64,
    crc32: u32,
    reader: CryptoReader<'a, R>,
    #[cfg(feature = "legacy-zip")] flags: u16,
) -> ZipResult<ZipFileReader<'a, R>> {
    let ae2_encrypted = reader.is_ae2_encrypted();
    #[cfg(not(feature = "legacy-zip"))]
    let flags = 0;
    Ok(ZipFileReader::Compressed(Box::new(Crc32Reader::new(
        Decompressor::new(
            io::BufReader::new(reader),
            compression_method,
            uncompressed_size,
            flags,
        )?,
        crc32,
        ae2_encrypted,
    ))))
}

pub(crate) fn make_symlink<T>(
    outpath: &Path,
    target: &[u8],
    #[cfg_attr(not(windows), allow(unused))] existing_files: &IndexMap<Box<str>, T>,
) -> ZipResult<()> {
    #[cfg_attr(not(any(unix, windows)), allow(unused))]
    let Ok(target_str) = std::str::from_utf8(target) else {
        return Err(invalid!("Invalid UTF-8 as symlink target"));
    };

    cfg_if! {
        if #[cfg(unix)] {
            std::os::unix::fs::symlink(Path::new(&target_str), outpath)?;
        } else if #[cfg(windows)] {
            let target = Path::new(OsStr::new(&target_str));
            let target_is_dir_from_archive =
                existing_files.contains_key(target_str) && is_dir(target_str);
            let target_is_dir = if target_is_dir_from_archive {
                true
            } else if let Ok(meta) = std::fs::metadata(target) {
                meta.is_dir()
            } else {
                false
            };
            if target_is_dir {
                std::os::windows::fs::symlink_dir(target, outpath)?;
            } else {
                std::os::windows::fs::symlink_file(target, outpath)?;
            }
        } else {
            use std::fs::File;
            let output = File::create(outpath);
            output?.write_all(target)?;
        }
    }

    Ok(())
}

#[derive(Debug)]
pub(crate) struct CentralDirectoryInfo {
    pub(crate) archive_offset: u64,
    pub(crate) directory_start: u64,
    pub(crate) number_of_files: usize,
    pub(crate) disk_number: u32,
    pub(crate) disk_with_central_directory: u32,
}

impl<'a> TryFrom<&'a CentralDirectoryEndInfo> for CentralDirectoryInfo {
    type Error = ZipError;

    fn try_from(value: &'a CentralDirectoryEndInfo) -> Result<Self, Self::Error> {
        let (relative_cd_offset, number_of_files, disk_number, disk_with_central_directory) =
            match &value.eocd64 {
                Some(DataAndPosition { data: eocd64, .. }) => {
                    if eocd64.number_of_files_on_this_disk > eocd64.number_of_files {
                        return Err(invalid!("ZIP64 footer indicates more files on this disk than in the whole archive"));
                    }
                    (
                        eocd64.central_directory_offset,
                        eocd64.number_of_files as usize,
                        eocd64.disk_number,
                        eocd64.disk_with_central_directory,
                    )
                }
                _ => (
                    value.eocd.data.central_directory_offset as u64,
                    value.eocd.data.number_of_files_on_this_disk as usize,
                    value.eocd.data.disk_number as u32,
                    value.eocd.data.disk_with_central_directory as u32,
                ),
            };

        let directory_start = relative_cd_offset
            .checked_add(value.archive_offset)
            .ok_or(invalid!("Invalid central directory size or offset"))?;

        Ok(Self {
            archive_offset: value.archive_offset,
            directory_start,
            number_of_files,
            disk_number,
            disk_with_central_directory,
        })
    }
}

/// Store all entries which specify a numeric "mode" which is familiar to POSIX operating systems.
#[cfg(unix)]
#[derive(Default, Debug)]
struct UnixFileModes {
    map: std::collections::BTreeMap<PathBuf, u32>,
}

#[cfg(unix)]
impl UnixFileModes {
    #[cfg_attr(not(debug_assertions), allow(unused))]
    pub fn add_mode(&mut self, path: PathBuf, mode: u32) {
        // We don't print a warning or consider it remotely out of the ordinary to receive two
        // separate modes for the same path: just take the later one.
        let old_entry = self.map.insert(path, mode);
        debug_assert_eq!(old_entry, None);
    }

    // Child nodes will be sorted later lexicographically, so reversing the order puts them first.
    pub fn all_perms_with_children_first(
        self,
    ) -> impl IntoIterator<Item = (PathBuf, std::fs::Permissions)> {
        use std::os::unix::fs::PermissionsExt;
        self.map
            .into_iter()
            .rev()
            .map(|(p, m)| (p, std::fs::Permissions::from_mode(m)))
    }
}

impl<R> ZipArchive<R> {
    pub(crate) fn from_finalized_writer(
        files: IndexMap<Box<str>, ZipFileData>,
        comment: Box<[u8]>,
        zip64_comment: Option<Box<[u8]>>,
        reader: R,
        central_start: u64,
    ) -> ZipResult<Self> {
        let initial_offset = match files.first() {
            Some((_, file)) => file.header_start,
            None => central_start,
        };
        let shared = Arc::new(ZipArchiveMetadata {
            files,
            offset: initial_offset,
            dir_start: central_start,
            config: Config {
                archive_offset: ArchiveOffset::Known(initial_offset),
            },
            comment,
            zip64_comment,
        });
        Ok(Self { reader, shared })
    }

    /// Total size of the files in the archive, if it can be known. Doesn't include directories or
    /// metadata.
    pub fn decompressed_size(&self) -> Option<u128> {
        let mut total = 0u128;
        for file in self.shared.files.values() {
            if file.using_data_descriptor {
                return None;
            }
            total = total.checked_add(file.uncompressed_size as u128)?;
        }
        Some(total)
    }
}

impl<R: Read + Seek> ZipArchive<R> {
    pub(crate) fn merge_contents<W: Write + Seek>(
        &mut self,
        mut w: W,
    ) -> ZipResult<IndexMap<Box<str>, ZipFileData>> {
        if self.shared.files.is_empty() {
            return Ok(IndexMap::new());
        }
        let mut new_files = self.shared.files.clone();
        /* The first file header will probably start at the beginning of the file, but zip doesn't
         * enforce that, and executable zips like PEX files will have a shebang line so will
         * definitely be greater than 0.
         *
         * assert_eq!(0, new_files[0].header_start); // Avoid this.
         */

        let first_new_file_header_start = w.stream_position()?;

        /* Push back file header starts for all entries in the covered files. */
        new_files.values_mut().try_for_each(|f| {
            /* This is probably the only really important thing to change. */
            f.header_start = f
                .header_start
                .checked_add(first_new_file_header_start)
                .ok_or(invalid!(
                    "new header start from merge would have been too large"
                ))?;
            /* This is only ever used internally to cache metadata lookups (it's not part of the
             * zip spec), and 0 is the sentinel value. */
            f.central_header_start = 0;
            /* This is an atomic variable so it can be updated from another thread in the
             * implementation (which is good!). */
            if let Some(old_data_start) = f.data_start.take() {
                let new_data_start = old_data_start
                    .checked_add(first_new_file_header_start)
                    .ok_or(invalid!(
                        "new data start from merge would have been too large"
                    ))?;
                f.data_start.get_or_init(|| new_data_start);
            }
            Ok::<_, ZipError>(())
        })?;

        /* Rewind to the beginning of the file.
         *
         * NB: we *could* decide to start copying from new_files[0].header_start instead, which
         * would avoid copying over e.g. any pex shebangs or other file contents that start before
         * the first zip file entry. However, zip files actually shouldn't care about garbage data
         * in *between* real entries, since the central directory header records the correct start
         * location of each, and keeping track of that math is more complicated logic that will only
         * rarely be used, since most zips that get merged together are likely to be produced
         * specifically for that purpose (and therefore are unlikely to have a shebang or other
         * preface). Finally, this preserves any data that might actually be useful.
         */
        self.reader.rewind()?;
        /* Find the end of the file data. */
        let length_to_read = self.shared.dir_start;
        /* Produce a Read that reads bytes up until the start of the central directory header.
         * This "as &mut dyn Read" trick is used elsewhere to avoid having to clone the underlying
         * handle, which it really shouldn't need to anyway. */
        let mut limited_raw = (&mut self.reader as &mut dyn Read).take(length_to_read);
        /* Copy over file data from source archive directly. */
        io::copy(&mut limited_raw, &mut w)?;

        /* Return the files we've just written to the data stream. */
        Ok(new_files)
    }

    /// Get the directory start offset and number of files. This is done in a
    /// separate function to ease the control flow design.
    pub(crate) fn get_metadata(config: Config, reader: &mut R) -> ZipResult<ZipArchiveMetadata> {
        // End of the probed region, initially set to the end of the file
        let file_len = reader.seek(io::SeekFrom::End(0))?;
        let mut end_exclusive = file_len;
        let mut last_err = None;

        loop {
            // Find the EOCD and possibly EOCD64 entries and determine the archive offset.
            let cde = match spec::find_central_directory(
                reader,
                config.archive_offset,
                end_exclusive,
                file_len,
            ) {
                Ok(cde) => cde,
                Err(e) => {
                    // return the previous error first (if there is)
                    return Err(last_err.unwrap_or(e));
                }
            };

            // Turn EOCD into internal representation.
            match CentralDirectoryInfo::try_from(&cde)
                .and_then(|info| Self::read_central_header(info, config, reader))
            {
                Ok(shared) => {
                    return Ok(shared.build(
                        cde.eocd.data.zip_file_comment,
                        cde.eocd64.map(|v| v.data.extensible_data_sector),
                    ));
                }
                Err(e) => {
                    last_err = Some(e);
                }
            };
            // Something went wrong while decoding the cde, try to find a new one
            end_exclusive = cde.eocd.position;
            continue;
        }
    }

    fn read_central_header(
        dir_info: CentralDirectoryInfo,
        config: Config,
        reader: &mut R,
    ) -> Result<zip_archive::SharedBuilder, ZipError> {
        // If the parsed number of files is greater than the offset then
        // something fishy is going on and we shouldn't trust number_of_files.
        let file_capacity = if dir_info.number_of_files > dir_info.directory_start as usize {
            0
        } else {
            dir_info.number_of_files
        };

        if dir_info.disk_number != dir_info.disk_with_central_directory {
            return unsupported_zip_error("Support for multi-disk files is not implemented");
        }

        if file_capacity.saturating_mul(size_of::<ZipFileData>()) > isize::MAX as usize {
            return unsupported_zip_error("Oversized central directory");
        }

        let mut files = Vec::with_capacity(file_capacity);
        reader.seek(SeekFrom::Start(dir_info.directory_start))?;
        for _ in 0..dir_info.number_of_files {
            let file = central_header_to_zip_file(reader, &dir_info)?;
            files.push(file);
        }

        Ok(zip_archive::SharedBuilder {
            files,
            offset: dir_info.archive_offset,
            dir_start: dir_info.directory_start,
            config,
        })
    }

    /// Returns the verification value and salt for the AES encryption of the file
    ///
    /// It fails if the file number is invalid.
    ///
    /// # Returns
    ///
    /// - None if the file is not encrypted with AES
    #[cfg(feature = "aes-crypto")]
    pub fn get_aes_verification_key_and_salt(
        &mut self,
        file_number: usize,
    ) -> ZipResult<Option<AesInfo>> {
        let (_, data) = self
            .shared
            .files
            .get_index(file_number)
            .ok_or(ZipError::FileNotFound)?;

        let limit_reader = find_content(data, &mut self.reader)?;
        match data.aes_mode {
            None => Ok(None),
            Some((aes_mode, _, _)) => {
                let (verification_value, salt) =
                    AesReader::new(limit_reader, aes_mode, data.compressed_size)
                        .get_verification_value_and_salt()?;
                let aes_info = AesInfo {
                    aes_mode,
                    verification_value,
                    salt,
                };
                Ok(Some(aes_info))
            }
        }
    }

    /// Read a ZIP archive, collecting the files it contains.
    ///
    /// This uses the central directory record of the ZIP file, and ignores local file headers.
    ///
    /// A default [`Config`] is used.
    pub fn new(reader: R) -> ZipResult<ZipArchive<R>> {
        Self::with_config(Default::default(), reader)
    }

    /// Get the metadata associated with the ZIP archive.
    ///
    /// This can be used with [`Self::unsafe_new_with_metadata`] to create a new reader over the
    /// same file without needing to reparse the metadata.
    pub fn metadata(&self) -> Arc<ZipArchiveMetadata> {
        self.shared.clone()
    }

    /// Read a ZIP archive using the given `metadata`.
    ///
    /// This is useful for creating multiple readers over the same file without
    /// needing to reparse the metadata.
    ///
    /// # Safety
    /// `unsafe` is used here to indicate that `reader` and `metadata` could
    /// potentially be incompatible, and it is left to the user to ensure they are.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::fs;
    /// use rayon::prelude::*;
    ///
    /// const FILE_NAME: &str = "my_data.zip";
    ///
    /// let file = fs::File::open(FILE_NAME).unwrap();
    /// let mut archive = zip::ZipArchive::new(file).unwrap();
    ///
    /// let file_names = (0..archive.len())
    ///     .into_par_iter()
    ///     .map_init({
    ///         let metadata = archive.metadata().clone();
    ///         move || {
    ///             let file = fs::File::open(FILE_NAME).unwrap();
    ///             unsafe { zip::ZipArchive::unsafe_new_with_metadata(file, metadata.clone()) }
    ///         }},
    ///         |archive, i| {
    ///             let mut file = archive.by_index(i).unwrap();
    ///             file.enclosed_name()
    ///         }
    ///     )
    ///     .filter_map(|name| name)
    ///     .collect::<Vec<_>>();
    /// ```
    pub unsafe fn unsafe_new_with_metadata(reader: R, metadata: Arc<ZipArchiveMetadata>) -> Self {
        Self {
            reader,
            shared: metadata,
        }
    }

    /// Read a ZIP archive providing a read configuration, collecting the files it contains.
    ///
    /// This uses the central directory record of the ZIP file, and ignores local file headers.
    pub fn with_config(config: Config, mut reader: R) -> ZipResult<ZipArchive<R>> {
        let shared = Self::get_metadata(config, &mut reader)?;

        Ok(ZipArchive {
            reader,
            shared: shared.into(),
        })
    }

    /// Extract a Zip archive into a directory, overwriting files if they
    /// already exist. Paths are sanitized with [`ZipFile::enclosed_name`]. Symbolic links are only
    /// created and followed if the target is within the destination directory (this is checked
    /// conservatively using [`std::fs::canonicalize`]).
    ///
    /// Extraction is not atomic. If an error is encountered, some of the files
    /// may be left on disk. However, on Unix targets, no newly-created directories with part but
    /// not all of their contents extracted will be readable, writable or usable as process working
    /// directories by any non-root user except you.
    ///
    /// On Unix and Windows, symbolic links are extracted correctly. On other platforms such as
    /// WebAssembly, symbolic links aren't supported, so they're extracted as normal files
    /// containing the target path in UTF-8.
    pub fn extract<P: AsRef<Path>>(&mut self, directory: P) -> ZipResult<()> {
        self.extract_internal(directory, None::<fn(&Path) -> bool>)
    }

    /// Extracts a Zip archive into a directory in the same fashion as
    /// [`ZipArchive::extract`], but detects a "root" directory in the archive
    /// (a single top-level directory that contains the rest of the archive's
    /// entries) and extracts its contents directly.
    ///
    /// For a sensible default `filter`, you can use [`root_dir_common_filter`].
    /// For a custom `filter`, see [`RootDirFilter`].
    ///
    /// See [`ZipArchive::root_dir`] for more information on how the root
    /// directory is detected and the meaning of the `filter` parameter.
    ///
    /// ## Example
    ///
    /// Imagine a Zip archive with the following structure:
    ///
    /// ```text
    /// root/file1.txt
    /// root/file2.txt
    /// root/sub/file3.txt
    /// root/sub/subsub/file4.txt
    /// ```
    ///
    /// If the archive is extracted to `foo` using [`ZipArchive::extract`],
    /// the resulting directory structure will be:
    ///
    /// ```text
    /// foo/root/file1.txt
    /// foo/root/file2.txt
    /// foo/root/sub/file3.txt
    /// foo/root/sub/subsub/file4.txt
    /// ```
    ///
    /// If the archive is extracted to `foo` using
    /// [`ZipArchive::extract_unwrapped_root_dir`], the resulting directory
    /// structure will be:
    ///
    /// ```text
    /// foo/file1.txt
    /// foo/file2.txt
    /// foo/sub/file3.txt
    /// foo/sub/subsub/file4.txt
    /// ```
    ///
    /// ## Example - No Root Directory
    ///
    /// Imagine a Zip archive with the following structure:
    ///
    /// ```text
    /// root/file1.txt
    /// root/file2.txt
    /// root/sub/file3.txt
    /// root/sub/subsub/file4.txt
    /// other/file5.txt
    /// ```
    ///
    /// Due to the presence of the `other` directory,
    /// [`ZipArchive::extract_unwrapped_root_dir`] will extract this in the same
    /// fashion as [`ZipArchive::extract`] as there is now no "root directory."
    pub fn extract_unwrapped_root_dir<P: AsRef<Path>>(
        &mut self,
        directory: P,
        root_dir_filter: impl RootDirFilter,
    ) -> ZipResult<()> {
        self.extract_internal(directory, Some(root_dir_filter))
    }

    fn extract_internal<P: AsRef<Path>>(
        &mut self,
        directory: P,
        root_dir_filter: Option<impl RootDirFilter>,
    ) -> ZipResult<()> {
        use std::fs;

        fs::create_dir_all(&directory)?;
        let directory = directory.as_ref().canonicalize()?;

        let root_dir = root_dir_filter
            .and_then(|filter| {
                self.root_dir(&filter)
                    .transpose()
                    .map(|root_dir| root_dir.map(|root_dir| (root_dir, filter)))
            })
            .transpose()?;

        // If we have a root dir, simplify the path components to be more
        // appropriate for passing to `safe_prepare_path`
        let root_dir = root_dir
            .as_ref()
            .map(|(root_dir, filter)| {
                crate::path::simplified_components(root_dir)
                    .ok_or_else(|| {
                        // Should be unreachable
                        debug_assert!(false, "Invalid root dir path");

                        invalid!("Invalid root dir path")
                    })
                    .map(|root_dir| (root_dir, filter))
            })
            .transpose()?;

        #[cfg(unix)]
        let mut files_by_unix_mode = UnixFileModes::default();

        for i in 0..self.len() {
            let mut file = self.by_index(i)?;

            let mut outpath = directory.clone();
            /* TODO: the control flow of this method call and subsequent expectations about the
             *       values in this loop is extremely difficult to follow. It also appears to
             *       perform a nested loop upon extracting every single file entry? Why does it
             *       accept two arguments that point to the same directory path, one mutable? */
            file.safe_prepare_path(directory.as_ref(), &mut outpath, root_dir.as_ref())?;

            #[cfg(any(unix, windows))]
            if file.is_symlink() {
                let declared_len = file.size();
                if declared_len > MAX_SYMLINK_TARGET_LEN {
                    return Err(invalid!(
                        "symlink target declares {} bytes, more than the {} byte limit",
                        declared_len,
                        MAX_SYMLINK_TARGET_LEN
                    ));
                }
                let mut target = Vec::with_capacity(declared_len as usize);
                file.read_to_end(&mut target)?;
                drop(file);
                make_symlink(&outpath, &target, &self.shared.files)?;
                continue;
            } else if file.is_dir() {
                crate::read::make_writable_dir_all(&outpath)?;
                continue;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;

            // Check for real permissions, which we'll set in a second pass.
            #[cfg(unix)]
            if let Some(mode) = file.unix_mode() {
                files_by_unix_mode.add_mode(outpath, mode);
            }

            // Set original timestamp.
            #[cfg(feature = "chrono")]
            if let Some(last_modified) = file.last_modified() {
                if let Some(t) = datetime_to_systemtime(&last_modified) {
                    outfile.set_modified(t)?;
                }
            }
        }

        // Ensure we update children's permissions before making a parent unwritable.
        #[cfg(unix)]
        for (path, perms) in files_by_unix_mode.all_perms_with_children_first() {
            std::fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    /// Number of files contained in this zip.
    pub fn len(&self) -> usize {
        self.shared.files.len()
    }

    /// Get the starting offset of the zip central directory.
    pub fn central_directory_start(&self) -> u64 {
        self.shared.dir_start
    }

    /// Whether this zip archive contains no files
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the offset from the beginning of the underlying reader that this zip begins at, in bytes.
    ///
    /// Normally this value is zero, but if the zip has arbitrary data prepended to it, then this value will be the size
    /// of that prepended data.
    pub fn offset(&self) -> u64 {
        self.shared.offset
    }

    /// Get the comment of the zip archive.
    pub fn comment(&self) -> &[u8] {
        &self.shared.comment
    }

    /// Get the ZIP64 comment of the zip archive, if it is ZIP64.
    pub fn zip64_comment(&self) -> Option<&[u8]> {
        self.shared.zip64_comment.as_deref()
    }

    /// Returns an iterator over all the file and directory names in this archive.
    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        self.shared.files.keys().map(|s| s.as_ref())
    }

    /// Returns Ok(true) if any compressed data in this archive belongs to more than one file. This
    /// doesn't make the archive invalid, but some programs will refuse to decompress it because the
    /// copies would take up space independently in the destination.
    pub fn has_overlapping_files(&mut self) -> ZipResult<bool> {
        let mut ranges = Vec::<Range<u64>>::with_capacity(self.shared.files.len());
        for file in self.shared.files.values() {
            if file.compressed_size == 0 {
                continue;
            }
            let start = file.data_start(&mut self.reader)?;
            let end = start + file.compressed_size;
            if ranges
                .iter()
                .any(|range| range.start <= end && start <= range.end)
            {
                return Ok(true);
            }
            ranges.push(start..end);
        }
        Ok(false)
    }

    /// Search for a file entry by name, decrypt with given password
    ///
    /// # Warning
    ///
    /// The implementation of the cryptographic algorithms has not
    /// gone through a correctness review, and you should assume it is insecure:
    /// passwords used with this API may be compromised.
    ///
    /// This function sometimes accepts wrong password. This is because the ZIP spec only allows us
    /// to check for a 1/256 chance that the password is correct.
    /// There are many passwords out there that will also pass the validity checks
    /// we are able to perform. This is a weakness of the ZipCrypto algorithm,
    /// due to its fairly primitive approach to cryptography.
    pub fn by_name_decrypt(&mut self, name: &str, password: &[u8]) -> ZipResult<ZipFile<'_, R>> {
        self.by_name_with_optional_password(name, Some(password))
    }

    /// Search for a file entry by name
    pub fn by_name(&mut self, name: &str) -> ZipResult<ZipFile<'_, R>> {
        self.by_name_with_optional_password(name, None)
    }

    /// Get the index of a file entry by name, if it's present.
    #[inline(always)]
    pub fn index_for_name(&self, name: &str) -> Option<usize> {
        self.shared.files.get_index_of(name)
    }

    /// Search for a file entry by path, decrypt with given password
    ///
    /// # Warning
    ///
    /// The implementation of the cryptographic algorithms has not
    /// gone through a correctness review, and you should assume it is insecure:
    /// passwords used with this API may be compromised.
    ///
    /// This function sometimes accepts wrong password. This is because the ZIP spec only allows us
    /// to check for a 1/256 chance that the password is correct.
    /// There are many passwords out there that will also pass the validity checks
    /// we are able to perform. This is a weakness of the ZipCrypto algorithm,
    /// due to its fairly primitive approach to cryptography.
    pub fn by_path_decrypt<T: AsRef<Path>>(
        &mut self,
        path: T,
        password: &[u8],
    ) -> ZipResult<ZipFile<'_, R>> {
        self.index_for_path(path)
            .ok_or(ZipError::FileNotFound)
            .and_then(|index| {
                self.by_index_with_options(index, ZipReadOptions::new().password(Some(password)))
            })
    }

    /// Search for a file entry by path
    pub fn by_path<T: AsRef<Path>>(&mut self, path: T) -> ZipResult<ZipFile<'_, R>> {
        self.index_for_path(path)
            .ok_or(ZipError::FileNotFound)
            .and_then(|index| self.by_index_with_options(index, ZipReadOptions::new()))
    }

    /// Get the index of a file entry by path, if it's present.
    #[inline(always)]
    pub fn index_for_path<T: AsRef<Path>>(&self, path: T) -> Option<usize> {
        self.index_for_name(&path_to_string(path))
    }

    /// Get the name of a file entry, if it's present.
    #[inline(always)]
    pub fn name_for_index(&self, index: usize) -> Option<&str> {
        self.shared
            .files
            .get_index(index)
            .map(|(name, _)| name.as_ref())
    }

    /// Search for a file entry by name and return a seekable object.
    pub fn by_name_seek(&mut self, name: &str) -> ZipResult<ZipFileSeek<'_, R>> {
        self.by_index_seek(self.index_for_name(name).ok_or(ZipError::FileNotFound)?)
    }

    /// Search for a file entry by index and return a seekable object.
    pub fn by_index_seek(&mut self, index: usize) -> ZipResult<ZipFileSeek<'_, R>> {
        let reader = &mut self.reader;
        self.shared
            .files
            .get_index(index)
            .ok_or(ZipError::FileNotFound)
            .and_then(move |(_, data)| {
                let seek_reader = match data.compression_method {
                    CompressionMethod::Stored => {
                        ZipFileSeekReader::Raw(find_content_seek(data, reader)?)
                    }
                    _ => {
                        return Err(ZipError::UnsupportedArchive(
                            "Seekable compressed files are not yet supported",
                        ))
                    }
                };
                Ok(ZipFileSeek {
                    reader: seek_reader,
                    data: Cow::Borrowed(data),
                })
            })
    }

    fn by_name_with_optional_password<'a>(
        &'a mut self,
        name: &str,
        password: Option<&[u8]>,
    ) -> ZipResult<ZipFile<'a, R>> {
        let Some(index) = self.shared.files.get_index_of(name) else {
            return Err(ZipError::FileNotFound);
        };
        self.by_index_with_options(index, ZipReadOptions::new().password(password))
    }

    /// Get a contained file by index, decrypt with given password
    ///
    /// # Warning
    ///
    /// The implementation of the cryptographic algorithms has not
    /// gone through a correctness review, and you should assume it is insecure:
    /// passwords used with this API may be compromised.
    ///
    /// This function sometimes accepts wrong password. This is because the ZIP spec only allows us
    /// to check for a 1/256 chance that the password is correct.
    /// There are many passwords out there that will also pass the validity checks
    /// we are able to perform. This is a weakness of the ZipCrypto algorithm,
    /// due to its fairly primitive approach to cryptography.
    pub fn by_index_decrypt(
        &mut self,
        file_number: usize,
        password: &[u8],
    ) -> ZipResult<ZipFile<'_, R>> {
        self.by_index_with_options(file_number, ZipReadOptions::new().password(Some(password)))
    }

    /// Get a contained file by index
    pub fn by_index(&mut self, file_number: usize) -> ZipResult<ZipFile<'_, R>> {
        self.by_index_with_options(file_number, ZipReadOptions::new())
    }

    /// Get a contained file by index without decompressing it
    pub fn by_index_raw(&mut self, file_number: usize) -> ZipResult<ZipFile<'_, R>> {
        let reader = &mut self.reader;
        let (_, data) = self
            .shared
            .files
            .get_index(file_number)
            .ok_or(ZipError::FileNotFound)?;
        Ok(ZipFile {
            reader: ZipFileReader::Raw(find_content(data, reader)?),
            data: Cow::Borrowed(data),
        })
    }

    /// Get a contained file by index with options.
    pub fn by_index_with_options(
        &mut self,
        file_number: usize,
        mut options: ZipReadOptions<'_>,
    ) -> ZipResult<ZipFile<'_, R>> {
        let (_, data) = self
            .shared
            .files
            .get_index(file_number)
            .ok_or(ZipError::FileNotFound)?;

        if options.ignore_encryption_flag {
            // Always use no password when we're ignoring the encryption flag.
            options.password = None;
        } else {
            // Require and use the password only if the file is encrypted.
            match (options.password, data.encrypted) {
                (None, true) => {
                    return Err(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED))
                }
                // Password supplied, but none needed! Discard.
                (Some(_), false) => options.password = None,
                _ => {}
            }
        }
        let limit_reader = find_content(data, &mut self.reader)?;

        let crypto_reader =
            make_crypto_reader(data, limit_reader, options.password, data.aes_mode)?;

        Ok(ZipFile {
            data: Cow::Borrowed(data),
            reader: make_reader(
                data.compression_method,
                data.uncompressed_size,
                data.crc32,
                crypto_reader,
                #[cfg(feature = "legacy-zip")]
                data.flags,
            )?,
        })
    }

    /// Find the "root directory" of an archive if it exists, filtering out
    /// irrelevant entries when searching.
    ///
    /// Our definition of a "root directory" is a single top-level directory
    /// that contains the rest of the archive's entries. This is useful for
    /// extracting archives that contain a single top-level directory that
    /// you want to "unwrap" and extract directly.
    ///
    /// For a sensible default filter, you can use [`root_dir_common_filter`].
    /// For a custom filter, see [`RootDirFilter`].
    pub fn root_dir(&self, filter: impl RootDirFilter) -> ZipResult<Option<PathBuf>> {
        let mut root_dir: Option<PathBuf> = None;

        for i in 0..self.len() {
            let (_, file) = self
                .shared
                .files
                .get_index(i)
                .ok_or(ZipError::FileNotFound)?;

            let path = match file.enclosed_name() {
                Some(path) => path,
                None => return Ok(None),
            };

            if !filter(&path) {
                continue;
            }

            macro_rules! replace_root_dir {
                ($path:ident) => {
                    match &mut root_dir {
                        Some(root_dir) => {
                            if *root_dir != $path {
                                // We've found multiple root directories,
                                // abort.
                                return Ok(None);
                            } else {
                                continue;
                            }
                        }

                        None => {
                            root_dir = Some($path.into());
                            continue;
                        }
                    }
                };
            }

            // If this entry is located at the root of the archive...
            if path.components().count() == 1 {
                if file.is_dir() {
                    // If it's a directory, it could be the root directory.
                    replace_root_dir!(path);
                } else {
                    // If it's anything else, this archive does not have a
                    // root directory.
                    return Ok(None);
                }
            }

            // Find the root directory for this entry.
            let mut path = path.as_path();
            while let Some(parent) = path.parent().filter(|path| *path != Path::new("")) {
                path = parent;
            }

            replace_root_dir!(path);
        }

        Ok(root_dir)
    }

    /// Unwrap and return the inner reader object
    ///
    /// The position of the reader is undefined.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

/// Holds the AES information of a file in the zip archive
#[derive(Debug)]
#[cfg(feature = "aes-crypto")]
pub struct AesInfo {
    /// The AES encryption mode
    pub aes_mode: AesMode,
    /// The verification key
    pub verification_value: [u8; PWD_VERIFY_LENGTH],
    /// The salt
    pub salt: Vec<u8>,
}

const fn unsupported_zip_error<T>(detail: &'static str) -> ZipResult<T> {
    Err(ZipError::UnsupportedArchive(detail))
}

/// Parse a central directory entry to collect the information for the file.
pub(crate) fn central_header_to_zip_file<R: Read + Seek>(
    reader: &mut R,
    central_directory: &CentralDirectoryInfo,
) -> ZipResult<ZipFileData> {
    let central_header_start = reader.stream_position()?;

    // Parse central header
    let block = ZipCentralEntryBlock::parse(reader)?;

    let file = central_header_to_zip_file_inner(
        reader,
        central_directory.archive_offset,
        central_header_start,
        block,
    )?;

    let central_header_end = reader.stream_position()?;

    reader.seek(SeekFrom::Start(central_header_end))?;
    Ok(file)
}

#[inline]
fn read_variable_length_byte_field<R: Read>(reader: &mut R, len: usize) -> ZipResult<Box<[u8]>> {
    let mut data = vec![0; len].into_boxed_slice();
    if let Err(e) = reader.read_exact(&mut data) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Err(invalid!(
                "Variable-length field extends beyond file boundary"
            ));
        }
        return Err(e.into());
    }
    Ok(data)
}

/// Parse a central directory entry to collect the information for the file.
fn central_header_to_zip_file_inner<R: Read>(
    reader: &mut R,
    archive_offset: u64,
    central_header_start: u64,
    block: ZipCentralEntryBlock,
) -> ZipResult<ZipFileData> {
    let ZipCentralEntryBlock {
        // magic,
        version_made_by,
        // version_to_extract,
        flags,
        compression_method,
        last_mod_time,
        last_mod_date,
        crc32,
        compressed_size,
        uncompressed_size,
        file_name_length,
        extra_field_length,
        file_comment_length,
        // disk_number,
        // internal_file_attributes,
        external_file_attributes,
        offset,
        ..
    } = block;

    let encrypted = flags & 1 == 1;
    let is_utf8 = flags & (1 << 11) != 0;
    let using_data_descriptor = flags & (1 << 3) != 0;

    let file_name_raw = read_variable_length_byte_field(reader, file_name_length as usize)?;
    let extra_field = read_variable_length_byte_field(reader, extra_field_length as usize)?;
    let file_comment_raw = read_variable_length_byte_field(reader, file_comment_length as usize)?;
    let file_name: Box<str> = match is_utf8 {
        true => String::from_utf8_lossy(&file_name_raw).into(),
        false => file_name_raw.clone().from_cp437(),
    };
    let file_comment: Box<str> = match is_utf8 {
        true => String::from_utf8_lossy(&file_comment_raw).into(),
        false => file_comment_raw.from_cp437(),
    };

    // Construct the result
    let mut result = ZipFileData {
        system: System::from((version_made_by >> 8) as u8),
        /* NB: this strips the top 8 bits! */
        version_made_by: version_made_by as u8,
        encrypted,
        using_data_descriptor,
        is_utf8,
        compression_method: CompressionMethod::parse_from_u16(compression_method),
        compression_level: None,
        last_modified_time: DateTime::try_from_msdos(last_mod_date, last_mod_time).ok(),
        crc32,
        compressed_size: compressed_size.into(),
        uncompressed_size: uncompressed_size.into(),
        flags,
        file_name,
        file_name_raw,
        extra_field: Some(Arc::new(extra_field.to_vec())),
        central_extra_field: None,
        file_comment,
        header_start: offset.into(),
        extra_data_start: None,
        central_header_start,
        data_start: OnceLock::new(),
        external_attributes: external_file_attributes,
        large_file: false,
        aes_mode: None,
        aes_extra_data_start: 0,
        extra_fields: Vec::new(),
    };
    parse_extra_field(&mut result)?;

    let aes_enabled = result.compression_method == CompressionMethod::AES;
    if aes_enabled && result.aes_mode.is_none() {
        return Err(invalid!("AES encryption without AES extra data field"));
    }

    // Account for shifted zip offsets.
    result.header_start = result
        .header_start
        .checked_add(archive_offset)
        .ok_or(invalid!("Archive header is too large"))?;

    Ok(result)
}

pub(crate) fn parse_extra_field(file: &mut ZipFileData) -> ZipResult<()> {
    let mut extra_field = file.extra_field.clone();
    let mut central_extra_field = file.central_extra_field.clone();
    for field_group in [&mut extra_field, &mut central_extra_field] {
        let Some(extra_field) = field_group else {
            continue;
        };
        let mut modified = false;
        let mut processed_extra_field = vec![];
        let len = extra_field.len();
        let mut reader = io::Cursor::new(&**extra_field);

        let mut position = reader.position();
        while position < len as u64 {
            let old_position = position;
            let remove = parse_single_extra_field(file, &mut reader, position, false)?;
            position = reader.position();
            if remove {
                modified = true;
            } else {
                let field_len = (position - old_position) as usize;
                let write_start = processed_extra_field.len();
                reader.seek(SeekFrom::Start(old_position))?;
                processed_extra_field.extend_from_slice(&vec![0u8; field_len]);
                if let Err(e) = reader
                    .read_exact(&mut processed_extra_field[write_start..(write_start + field_len)])
                {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        return Err(invalid!("Extra field content exceeds declared length"));
                    }
                    return Err(e.into());
                }
            }
        }
        if modified {
            *field_group = Some(Arc::new(processed_extra_field));
        }
    }
    file.extra_field = extra_field;
    file.central_extra_field = central_extra_field;
    Ok(())
}

pub(crate) fn parse_single_extra_field<R: Read>(
    file: &mut ZipFileData,
    reader: &mut R,
    bytes_already_read: u64,
    disallow_zip64: bool,
) -> ZipResult<bool> {
    let kind = match reader.read_u16_le() {
        Ok(kind) => kind,
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let len = match reader.read_u16_le() {
        Ok(len) => len,
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(invalid!("Extra field header truncated"))
        }
        Err(e) => return Err(e.into()),
    };
    match kind {
        // Zip64 extended information extra field
        0x0001 => {
            if disallow_zip64 {
                return Err(invalid!("Can't write a custom field using the ZIP64 ID"));
            }
            file.large_file = true;
            let mut consumed_len = 0;
            if len >= 24 || file.uncompressed_size == spec::ZIP64_BYTES_THR {
                file.uncompressed_size = match reader.read_u64_le() {
                    Ok(v) => v,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        return Err(invalid!("ZIP64 extra field truncated"))
                    }
                    Err(e) => return Err(e.into()),
                };
                consumed_len += size_of::<u64>();
            }
            if len >= 24 || file.compressed_size == spec::ZIP64_BYTES_THR {
                file.compressed_size = match reader.read_u64_le() {
                    Ok(v) => v,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        return Err(invalid!("ZIP64 extra field truncated"))
                    }
                    Err(e) => return Err(e.into()),
                };
                consumed_len += size_of::<u64>();
            }
            if len >= 24 || file.header_start == spec::ZIP64_BYTES_THR {
                file.header_start = match reader.read_u64_le() {
                    Ok(v) => v,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        return Err(invalid!("ZIP64 extra field truncated"))
                    }
                    Err(e) => return Err(e.into()),
                };
                consumed_len += size_of::<u64>();
            }
            let Some(leftover_len) = (len as usize).checked_sub(consumed_len) else {
                return Err(invalid!("ZIP64 extra-data field is the wrong length"));
            };
            if let Err(e) = reader.read_exact(&mut vec![0u8; leftover_len]) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    return Err(invalid!("ZIP64 extra field truncated"));
                }
                return Err(e.into());
            }
            return Ok(true);
        }
        0x000a => {
            // NTFS extra field
            file.extra_fields
                .push(ExtraField::Ntfs(Ntfs::try_from_reader(reader, len)?));
        }
        0x9901 => {
            // AES
            if len != 7 {
                return Err(ZipError::UnsupportedArchive(
                    "AES extra data field has an unsupported length",
                ));
            }
            let vendor_version = reader.read_u16_le()?;
            let vendor_id = reader.read_u16_le()?;
            let mut out = [0u8];
            if let Err(e) = reader.read_exact(&mut out) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    return Err(invalid!("AES extra field truncated"));
                }
                return Err(e.into());
            }
            let aes_mode = out[0];
            let compression_method = CompressionMethod::parse_from_u16(reader.read_u16_le()?);

            if vendor_id != 0x4541 {
                return Err(invalid!("Invalid AES vendor"));
            }
            let vendor_version = match vendor_version {
                0x0001 => AesVendorVersion::Ae1,
                0x0002 => AesVendorVersion::Ae2,
                _ => return Err(invalid!("Invalid AES vendor version")),
            };
            match aes_mode {
                0x01 => file.aes_mode = Some((AesMode::Aes128, vendor_version, compression_method)),
                0x02 => file.aes_mode = Some((AesMode::Aes192, vendor_version, compression_method)),
                0x03 => file.aes_mode = Some((AesMode::Aes256, vendor_version, compression_method)),
                _ => return Err(invalid!("Invalid AES encryption strength")),
            };
            file.compression_method = compression_method;
            file.aes_extra_data_start = bytes_already_read;
        }
        0x5455 => {
            // extended timestamp
            // https://libzip.org/specifications/extrafld.txt

            file.extra_fields.push(ExtraField::ExtendedTimestamp(
                ExtendedTimestamp::try_from_reader(reader, len)?,
            ));
        }
        0x6375 => {
            // Info-ZIP Unicode Comment Extra Field
            // APPNOTE 4.6.8 and https://libzip.org/specifications/extrafld.txt
            file.file_comment = String::from_utf8(
                UnicodeExtraField::try_from_reader(reader, len)?
                    .unwrap_valid(file.file_comment.as_bytes())?
                    .into_vec(),
            )?
            .into();
        }
        0x7075 => {
            // Info-ZIP Unicode Path Extra Field
            // APPNOTE 4.6.9 and https://libzip.org/specifications/extrafld.txt
            file.file_name_raw = UnicodeExtraField::try_from_reader(reader, len)?
                .unwrap_valid(&file.file_name_raw)?;
            file.file_name =
                String::from_utf8(file.file_name_raw.clone().into_vec())?.into_boxed_str();
            file.is_utf8 = true;
        }
        _ => {
            if let Err(e) = reader.read_exact(&mut vec![0u8; len as usize]) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    return Err(invalid!("Extra field content truncated"));
                }
                return Err(e.into());
            }
            // Other fields are ignored
        }
    }
    Ok(false)
}

/// A trait for exposing file metadata inside the zip.
pub trait HasZipMetadata {
    /// Get the file metadata
    fn get_metadata(&self) -> &ZipFileData;
}

/// Options for reading a file from an archive.
#[derive(Default)]
pub struct ZipReadOptions<'a> {
    /// The password to use when decrypting the file.  This is ignored if not required.
    password: Option<&'a [u8]>,

    /// Ignore the value of the encryption flag and proceed as if the file were plaintext.
    ignore_encryption_flag: bool,
}

impl<'a> ZipReadOptions<'a> {
    /// Create a new set of options with the default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the password, if any, to use.  Return for chaining.
    #[must_use]
    pub fn password(mut self, password: Option<&'a [u8]>) -> Self {
        self.password = password;
        self
    }

    /// Set the ignore encryption flag.  Return for chaining.
    #[must_use]
    pub fn ignore_encryption_flag(mut self, ignore: bool) -> Self {
        self.ignore_encryption_flag = ignore;
        self
    }
}

/// Methods for retrieving information on zip files
impl<'a, R: Read + ?Sized> ZipFile<'a, R> {
    pub(crate) fn take_raw_reader(&mut self) -> io::Result<io::Take<&'a mut R>> {
        replace(&mut self.reader, ZipFileReader::NoReader).into_inner()
    }

    /// Get the version of the file
    pub fn version_made_by(&self) -> (u8, u8) {
        (
            self.get_metadata().version_made_by / 10,
            self.get_metadata().version_made_by % 10,
        )
    }

    /// Get the name of the file
    ///
    /// # Warnings
    ///
    /// It is dangerous to use this name directly when extracting an archive.
    /// It may contain an absolute path (`/etc/shadow`), or break out of the
    /// current directory (`../runtime`). Carelessly writing to these paths
    /// allows an attacker to craft a ZIP archive that will overwrite critical
    /// files.
    ///
    /// You can use the [`ZipFile::enclosed_name`] method to validate the name
    /// as a safe path.
    pub fn name(&self) -> &str {
        &self.get_metadata().file_name
    }

    /// Get the name of the file, in the raw (internal) byte representation.
    ///
    /// The encoding of this data is currently undefined.
    pub fn name_raw(&self) -> &[u8] {
        &self.get_metadata().file_name_raw
    }

    /// Get the name of the file in a sanitized form. It truncates the name to the first NULL byte,
    /// removes a leading '/' and removes '..' parts.
    #[deprecated(
        since = "0.5.7",
        note = "by stripping `..`s from the path, the meaning of paths can change.
                `mangled_name` can be used if this behaviour is desirable"
    )]
    pub fn sanitized_name(&self) -> PathBuf {
        self.mangled_name()
    }

    /// Rewrite the path, ignoring any path components with special meaning.
    ///
    /// - Absolute paths are made relative
    /// - [`ParentDir`]s are ignored
    /// - Truncates the filename at a NULL byte
    ///
    /// This is appropriate if you need to be able to extract *something* from
    /// any archive, but will easily misrepresent trivial paths like
    /// `foo/../bar` as `foo/bar` (instead of `bar`). Because of this,
    /// [`ZipFile::enclosed_name`] is the better option in most scenarios.
    ///
    /// [`ParentDir`]: `Component::ParentDir`
    pub fn mangled_name(&self) -> PathBuf {
        self.get_metadata().file_name_sanitized()
    }

    /// Ensure the file path is safe to use as a [`Path`].
    ///
    /// - It can't contain NULL bytes
    /// - It can't resolve to a path outside the current directory
    ///   > `foo/../bar` is fine, `foo/../../bar` is not.
    /// - It can't be an absolute path
    ///
    /// This will read well-formed ZIP files correctly, and is resistant
    /// to path-based exploits. It is recommended over
    /// [`ZipFile::mangled_name`].
    pub fn enclosed_name(&self) -> Option<PathBuf> {
        self.get_metadata().enclosed_name()
    }

    pub(crate) fn simplified_components(&self) -> Option<Vec<&OsStr>> {
        self.get_metadata().simplified_components()
    }

    /// Prepare the path for extraction by creating necessary missing directories and checking for symlinks to be contained within the base path.
    ///
    /// `base_path` parameter is assumed to be canonicalized.
    pub(crate) fn safe_prepare_path(
        &self,
        base_path: &Path,
        outpath: &mut PathBuf,
        root_dir: Option<&(Vec<&OsStr>, impl RootDirFilter)>,
    ) -> ZipResult<()> {
        let components = self
            .simplified_components()
            .ok_or(invalid!("Invalid file path"))?;

        let components = match root_dir {
            Some((root_dir, filter)) => match components.strip_prefix(&**root_dir) {
                Some(components) => components,

                // In this case, we expect that the file was not in the root
                // directory, but was filtered out when searching for the
                // root directory.
                None => {
                    // We could technically find ourselves at this code
                    // path if the user provides an unstable or
                    // non-deterministic `filter` function.
                    //
                    // If debug assertions are on, we should panic here.
                    // Otherwise, the safest thing to do here is to just
                    // extract as-is.
                    debug_assert!(
                        !filter(&PathBuf::from_iter(components.iter())),
                        "Root directory filter should not match at this point"
                    );

                    // Extract as-is.
                    &components[..]
                }
            },

            None => &components[..],
        };

        let components_len = components.len();

        for (is_last, component) in components
            .iter()
            .copied()
            .enumerate()
            .map(|(i, c)| (i == components_len - 1, c))
        {
            // we can skip the target directory itself because the base path is assumed to be "trusted" (if the user say extract to a symlink we can follow it)
            outpath.push(component);

            // check if the path is a symlink, the target must be _inherently_ within the directory
            for limit in (0..5u8).rev() {
                let meta = match std::fs::symlink_metadata(&outpath) {
                    Ok(meta) => meta,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        if !is_last {
                            crate::read::make_writable_dir_all(&outpath)?;
                        }
                        break;
                    }
                    Err(e) => return Err(e.into()),
                };

                if !meta.is_symlink() {
                    break;
                }

                if limit == 0 {
                    return Err(invalid!("Extraction followed a symlink too deep"));
                }

                // note that we cannot accept links that do not inherently resolve to a path inside the directory to prevent:
                // - disclosure of unrelated path exists (no check for a path exist and then ../ out)
                // - issues with file-system specific path resolution (case sensitivity, etc)
                let target = std::fs::read_link(&outpath)?;

                if !crate::path::simplified_components(&target)
                    .ok_or(invalid!("Invalid symlink target path"))?
                    .starts_with(
                        &crate::path::simplified_components(base_path)
                            .ok_or(invalid!("Invalid base path"))?,
                    )
                {
                    let is_absolute_enclosed = base_path
                        .components()
                        .map(Some)
                        .chain(std::iter::once(None))
                        .zip(target.components().map(Some).chain(std::iter::repeat(None)))
                        .all(|(a, b)| match (a, b) {
                            // both components are normal
                            (Some(Component::Normal(a)), Some(Component::Normal(b))) => a == b,
                            // both components consumed fully
                            (None, None) => true,
                            // target consumed fully but base path is not
                            (Some(_), None) => false,
                            // base path consumed fully but target is not (and normal)
                            (None, Some(Component::CurDir | Component::Normal(_))) => true,
                            _ => false,
                        });

                    if !is_absolute_enclosed {
                        return Err(invalid!("Symlink is not inherently safe"));
                    }
                }

                outpath.push(target);
            }
        }
        Ok(())
    }

    /// Get the comment of the file
    pub fn comment(&self) -> &str {
        &self.get_metadata().file_comment
    }

    /// Get the compression method used to store the file
    pub fn compression(&self) -> CompressionMethod {
        self.get_metadata().compression_method
    }

    /// Get if the files is encrypted or not
    pub fn encrypted(&self) -> bool {
        self.data.encrypted
    }

    /// Get the size of the file, in bytes, in the archive
    pub fn compressed_size(&self) -> u64 {
        self.get_metadata().compressed_size
    }

    /// Get the size of the file, in bytes, when uncompressed
    pub fn size(&self) -> u64 {
        self.get_metadata().uncompressed_size
    }

    /// Get the time the file was last modified
    pub fn last_modified(&self) -> Option<DateTime> {
        self.data.last_modified_time
    }
    /// Returns whether the file is actually a directory
    pub fn is_dir(&self) -> bool {
        is_dir(self.name())
    }

    /// Returns whether the file is actually a symbolic link
    pub fn is_symlink(&self) -> bool {
        self.unix_mode()
            .is_some_and(|mode| mode & S_IFLNK == S_IFLNK)
    }

    /// Returns whether the file is a normal file (i.e. not a directory or symlink)
    pub fn is_file(&self) -> bool {
        !self.is_dir() && !self.is_symlink()
    }

    /// Get unix mode for the file
    pub fn unix_mode(&self) -> Option<u32> {
        self.get_metadata().unix_mode()
    }

    /// Get the CRC32 hash of the original file
    pub fn crc32(&self) -> u32 {
        self.get_metadata().crc32
    }

    /// Get the extra data of the zip header for this file
    pub fn extra_data(&self) -> Option<&[u8]> {
        self.get_metadata()
            .extra_field
            .as_ref()
            .map(|v| v.deref().deref())
    }

    /// Get the starting offset of the data of the compressed file
    pub fn data_start(&self) -> u64 {
        *self.data.data_start.get().unwrap()
    }

    /// Get the starting offset of the zip header for this file
    pub fn header_start(&self) -> u64 {
        self.get_metadata().header_start
    }
    /// Get the starting offset of the zip header in the central directory for this file
    pub fn central_header_start(&self) -> u64 {
        self.get_metadata().central_header_start
    }

    /// Get the [`SimpleFileOptions`] that would be used to write this file to
    /// a new zip archive.
    pub fn options(&self) -> SimpleFileOptions {
        let mut options = SimpleFileOptions::default()
            .large_file(self.compressed_size().max(self.size()) > ZIP64_BYTES_THR)
            .compression_method(self.compression())
            .unix_permissions(self.unix_mode().unwrap_or(0o644) | S_IFREG)
            .last_modified_time(
                self.last_modified()
                    .filter(|m| m.is_valid())
                    .unwrap_or_else(DateTime::default_for_write),
            );

        options.normalize();
        #[cfg(feature = "aes-crypto")]
        if let Some(aes) = self.get_metadata().aes_mode {
            // Preserve AES metadata in options for downstream writers.
            // This is metadata-only and does not trigger encryption.
            options.aes_mode = Some(aes);
        }
        options
    }
}

/// Methods for retrieving information on zip files
impl<R: Read> ZipFile<'_, R> {
    /// iterate through all extra fields
    pub fn extra_data_fields(&self) -> impl Iterator<Item = &ExtraField> {
        self.data.extra_fields.iter()
    }
}

impl<R: Read + ?Sized> HasZipMetadata for ZipFile<'_, R> {
    fn get_metadata(&self) -> &ZipFileData {
        self.data.as_ref()
    }
}

impl<R: Read + ?Sized> Read for ZipFile<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.reader.read_exact(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.reader.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.reader.read_to_string(buf)
    }
}

impl<R: Read> Read for ZipFileSeek<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.reader {
            ZipFileSeekReader::Raw(r) => r.read(buf),
        }
    }
}

impl<R: Seek> Seek for ZipFileSeek<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match &mut self.reader {
            ZipFileSeekReader::Raw(r) => r.seek(pos),
        }
    }
}

impl<R> HasZipMetadata for ZipFileSeek<'_, R> {
    fn get_metadata(&self) -> &ZipFileData {
        self.data.as_ref()
    }
}

impl<R: Read + ?Sized> Drop for ZipFile<'_, R> {
    fn drop(&mut self) {
        // self.data is Owned, this reader is constructed by a streaming reader.
        // In this case, we want to exhaust the reader so that the next file is accessible.
        if let Cow::Owned(_) = self.data {
            // Get the inner `Take` reader so all decryption, decompression and CRC calculation is skipped.
            if let Ok(mut inner) = self.take_raw_reader() {
                let _ = copy(&mut inner, &mut sink());
            }
        }
    }
}

/// Read ZipFile structures from a non-seekable reader.
///
/// This is an alternative method to read a zip file. If possible, use the ZipArchive functions
/// as some information will be missing when reading this manner.
///
/// Reads a file header from the start of the stream. Will return `Ok(Some(..))` if a file is
/// present at the start of the stream. Returns `Ok(None)` if the start of the central directory
/// is encountered. No more files should be read after this.
///
/// The Drop implementation of ZipFile ensures that the reader will be correctly positioned after
/// the structure is done.
///
/// Missing fields are:
/// * `comment`: set to an empty string
/// * `data_start`: set to 0
/// * `external_attributes`: `unix_mode()`: will return None
pub fn read_zipfile_from_stream<R: Read>(reader: &mut R) -> ZipResult<Option<ZipFile<'_, R>>> {
    // We can't use the typical ::parse() method, as we follow separate code paths depending on the
    // "magic" value (since the magic value will be from the central directory header if we've
    // finished iterating over all the actual files).
    /* TODO: smallvec? */

    let mut block = ZipLocalEntryBlock::zeroed();
    reader.read_exact(block.as_bytes_mut())?;

    match block.magic().from_le() {
        spec::Magic::LOCAL_FILE_HEADER_SIGNATURE => (),
        spec::Magic::CENTRAL_DIRECTORY_HEADER_SIGNATURE => return Ok(None),
        _ => return Err(ZipLocalEntryBlock::WRONG_MAGIC_ERROR),
    }

    let block = block.from_le();

    let mut result = ZipFileData::from_local_block(block, reader)?;

    match parse_extra_field(&mut result) {
        Ok(..) | Err(ZipError::Io(..)) => {}
        Err(e) => return Err(e),
    }

    let limit_reader = reader.take(result.compressed_size);
    let crypto_reader = make_crypto_reader(&result, limit_reader, None, None)?;
    let ZipFileData {
        crc32,
        uncompressed_size,
        compression_method,
        #[cfg(feature = "legacy-zip")]
        flags,
        ..
    } = result;

    Ok(Some(ZipFile {
        data: Cow::Owned(result),
        reader: make_reader(
            compression_method,
            uncompressed_size,
            crc32,
            crypto_reader,
            #[cfg(feature = "legacy-zip")]
            flags,
        )?,
    }))
}

/// A filter that determines whether an entry should be ignored when searching
/// for the root directory of a Zip archive.
///
/// Returns `true` if the entry should be considered, and `false` if it should
/// be ignored.
///
/// See [`root_dir_common_filter`] for a sensible default filter.
pub trait RootDirFilter: Fn(&Path) -> bool {}
impl<F: Fn(&Path) -> bool> RootDirFilter for F {}

/// Common filters when finding the root directory of a Zip archive.
///
/// This filter is a sensible default for most use cases and filters out common
/// system files that are usually irrelevant to the contents of the archive.
///
/// Currently, the filter ignores:
/// - `/__MACOSX/`
/// - `/.DS_Store`
/// - `/Thumbs.db`
///
/// **This function is not guaranteed to be stable and may change in future versions.**
///
/// # Example
///
/// ```rust
/// # use std::path::Path;
/// assert!(zip::read::root_dir_common_filter(Path::new("foo.txt")));
/// assert!(!zip::read::root_dir_common_filter(Path::new(".DS_Store")));
/// assert!(!zip::read::root_dir_common_filter(Path::new("Thumbs.db")));
/// assert!(!zip::read::root_dir_common_filter(Path::new("__MACOSX")));
/// assert!(!zip::read::root_dir_common_filter(Path::new("__MACOSX/foo.txt")));
/// ```
pub fn root_dir_common_filter(path: &Path) -> bool {
    const COMMON_FILTER_ROOT_FILES: &[&str] = &[".DS_Store", "Thumbs.db"];

    if path.starts_with("__MACOSX") {
        return false;
    }

    if path.components().count() == 1
        && path.file_name().is_some_and(|file_name| {
            COMMON_FILTER_ROOT_FILES
                .iter()
                .map(OsStr::new)
                .any(|cmp| cmp == file_name)
        })
    {
        return false;
    }

    true
}

#[cfg(feature = "chrono")]
/// Generate a `SystemTime` from a `DateTime`.
fn datetime_to_systemtime(datetime: &DateTime) -> Option<std::time::SystemTime> {
    if let Some(t) = generate_chrono_datetime(datetime) {
        let time = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(t, chrono::Utc);
        return Some(time.into());
    }
    None
}

#[cfg(feature = "chrono")]
/// Generate a `NaiveDateTime` from a `DateTime`.
fn generate_chrono_datetime(datetime: &DateTime) -> Option<chrono::NaiveDateTime> {
    if let Some(d) = chrono::NaiveDate::from_ymd_opt(
        datetime.year().into(),
        datetime.month().into(),
        datetime.day().into(),
    ) {
        if let Some(d) = d.and_hms_opt(
            datetime.hour().into(),
            datetime.minute().into(),
            datetime.second().into(),
        ) {
            return Some(d);
        }
    }
    None
}

/// Read ZipFile from a non-seekable reader like [read_zipfile_from_stream] does, but assume the
/// given compressed size and don't read any further ahead than that.
pub fn read_zipfile_from_stream_with_compressed_size<R: io::Read>(
    reader: &mut R,
    compressed_size: u64,
) -> ZipResult<Option<ZipFile<'_, R>>> {
    let mut block = ZipLocalEntryBlock::zeroed();
    reader.read_exact(block.as_bytes_mut())?;

    match block.magic().from_le() {
        spec::Magic::LOCAL_FILE_HEADER_SIGNATURE => (),
        spec::Magic::CENTRAL_DIRECTORY_HEADER_SIGNATURE => return Ok(None),
        _ => return Err(ZipLocalEntryBlock::WRONG_MAGIC_ERROR),
    }

    let block = block.from_le();

    let mut result = ZipFileData::from_local_block(block, reader)?;
    result.compressed_size = compressed_size;

    if result.encrypted {
        return unsupported_zip_error("Encrypted files are not supported");
    }

    let limit_reader = reader.take(result.compressed_size);
    let crypto_reader = make_crypto_reader(&result, limit_reader, None, None)?;
    let ZipFileData {
        crc32,
        compression_method,
        uncompressed_size,
        #[cfg(feature = "legacy-zip")]
        flags,
        ..
    } = result;

    Ok(Some(ZipFile {
        data: Cow::Owned(result),
        reader: make_reader(
            compression_method,
            uncompressed_size,
            crc32,
            crypto_reader,
            #[cfg(feature = "legacy-zip")]
            flags,
        )?,
    }))
}

#[cfg(test)]
mod test {
    use crate::read::ZipReadOptions;
    use crate::result::ZipResult;
    use crate::types::SimpleFileOptions;
    use crate::CompressionMethod::Stored;
    use crate::{ZipArchive, ZipWriter};
    use std::io::{Cursor, Read, Write};
    use tempfile::TempDir;

    #[test]
    fn invalid_offset() {
        use super::ZipArchive;

        let reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/invalid_offset.zip"
        )));
        assert!(reader.is_err());
    }

    #[test]
    fn invalid_offset2() {
        use super::ZipArchive;

        let reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/invalid_offset2.zip"
        )));
        assert!(reader.is_err());
    }

    #[test]
    fn zip64_with_leading_junk() {
        use super::ZipArchive;

        let reader =
            ZipArchive::new(Cursor::new(include_bytes!("../tests/data/zip64_demo.zip"))).unwrap();
        assert_eq!(reader.len(), 1);
    }

    #[test]
    fn zip_contents() {
        use super::ZipArchive;

        let mut reader =
            ZipArchive::new(Cursor::new(include_bytes!("../tests/data/mimetype.zip"))).unwrap();
        assert_eq!(reader.comment(), b"");
        assert_eq!(reader.by_index(0).unwrap().central_header_start(), 77);
    }

    #[test]
    fn zip_read_streaming() {
        use super::read_zipfile_from_stream;

        let mut reader = Cursor::new(include_bytes!("../tests/data/mimetype.zip"));
        loop {
            if read_zipfile_from_stream(&mut reader).unwrap().is_none() {
                break;
            }
        }
    }

    #[test]
    fn zip_clone() {
        use super::ZipArchive;
        use std::io::Read;

        let mut reader1 =
            ZipArchive::new(Cursor::new(include_bytes!("../tests/data/mimetype.zip"))).unwrap();
        let mut reader2 = reader1.clone();

        let mut file1 = reader1.by_index(0).unwrap();
        let mut file2 = reader2.by_index(0).unwrap();

        let t = file1.last_modified().unwrap();
        assert_eq!(
            (
                t.year(),
                t.month(),
                t.day(),
                t.hour(),
                t.minute(),
                t.second()
            ),
            (1980, 1, 1, 0, 0, 0)
        );

        let mut buf1 = [0; 5];
        let mut buf2 = [0; 5];
        let mut buf3 = [0; 5];
        let mut buf4 = [0; 5];

        file1.read_exact(&mut buf1).unwrap();
        file2.read_exact(&mut buf2).unwrap();
        file1.read_exact(&mut buf3).unwrap();
        file2.read_exact(&mut buf4).unwrap();

        assert_eq!(buf1, buf2);
        assert_eq!(buf3, buf4);
        assert_ne!(buf1, buf3);
    }

    #[test]
    fn file_and_dir_predicates() {
        use super::ZipArchive;

        let mut zip = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/files_and_dirs.zip"
        )))
        .unwrap();

        for i in 0..zip.len() {
            let zip_file = zip.by_index(i).unwrap();
            let full_name = zip_file.enclosed_name().unwrap();
            let file_name = full_name.file_name().unwrap().to_str().unwrap();
            assert!(
                (file_name.starts_with("dir") && zip_file.is_dir())
                    || (file_name.starts_with("file") && zip_file.is_file())
            );
        }
    }

    #[test]
    fn zip64_magic_in_filenames() {
        let files = vec![
            include_bytes!("../tests/data/zip64_magic_in_filename_1.zip").to_vec(),
            include_bytes!("../tests/data/zip64_magic_in_filename_2.zip").to_vec(),
            include_bytes!("../tests/data/zip64_magic_in_filename_3.zip").to_vec(),
            include_bytes!("../tests/data/zip64_magic_in_filename_4.zip").to_vec(),
            include_bytes!("../tests/data/zip64_magic_in_filename_5.zip").to_vec(),
        ];
        // Although we don't allow adding files whose names contain the ZIP64 CDB-end or
        // CDB-end-locator signatures, we still read them when they aren't genuinely ambiguous.
        for file in files {
            ZipArchive::new(Cursor::new(file)).unwrap();
        }
    }

    /// test case to ensure we don't preemptively over allocate based on the
    /// declared number of files in the CDE of an invalid zip when the number of
    /// files declared is more than the alleged offset in the CDE
    #[test]
    fn invalid_cde_number_of_files_allocation_smaller_offset() {
        use super::ZipArchive;

        let reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/invalid_cde_number_of_files_allocation_smaller_offset.zip"
        )));
        assert!(reader.is_err() || reader.unwrap().is_empty());
    }

    /// test case to ensure we don't preemptively over allocate based on the
    /// declared number of files in the CDE of an invalid zip when the number of
    /// files declared is less than the alleged offset in the CDE
    #[test]
    fn invalid_cde_number_of_files_allocation_greater_offset() {
        use super::ZipArchive;

        let reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/invalid_cde_number_of_files_allocation_greater_offset.zip"
        )));
        assert!(reader.is_err());
    }

    #[cfg(feature = "deflate64")]
    #[test]
    fn deflate64_index_out_of_bounds() -> std::io::Result<()> {
        let mut reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/raw_deflate64_index_out_of_bounds.zip"
        )))?;
        std::io::copy(&mut reader.by_index(0)?, &mut std::io::sink()).expect_err("Invalid file");
        Ok(())
    }

    #[cfg(feature = "deflate64")]
    #[test]
    fn deflate64_not_enough_space() {
        ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/deflate64_issue_25.zip"
        )))
        .expect_err("Invalid file");
    }

    #[cfg(feature = "deflate-flate2")]
    #[test]
    fn test_read_with_data_descriptor() {
        use std::io::Read;

        let mut reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/data_descriptor.zip"
        )))
        .unwrap();
        let mut decompressed = [0u8; 16];
        let mut file = reader.by_index(0).unwrap();
        assert_eq!(file.read(&mut decompressed).unwrap(), 12);
    }

    #[test]
    fn test_is_symlink() -> std::io::Result<()> {
        let mut reader = ZipArchive::new(Cursor::new(include_bytes!("../tests/data/symlink.zip")))?;
        assert!(reader.by_index(0)?.is_symlink());
        let tempdir = TempDir::with_prefix("test_is_symlink")?;
        reader.extract(&tempdir)?;
        assert!(tempdir.path().join("bar").is_symlink());
        Ok(())
    }

    #[test]
    #[cfg(feature = "deflate-flate2")]
    fn test_utf8_extra_field() {
        let mut reader =
            ZipArchive::new(Cursor::new(include_bytes!("../tests/data/chinese.zip"))).unwrap();
        reader.by_name("七个房间.txt").unwrap();
    }

    #[test]
    fn test_utf8() {
        let mut reader =
            ZipArchive::new(Cursor::new(include_bytes!("../tests/data/linux-7z.zip"))).unwrap();
        reader.by_name("你好.txt").unwrap();
    }

    #[test]
    fn test_utf8_2() {
        let mut reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/windows-7zip.zip"
        )))
        .unwrap();
        reader.by_name("你好.txt").unwrap();
    }

    #[test]
    fn test_64k_files() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions {
            compression_method: Stored,
            ..Default::default()
        };
        for i in 0..=u16::MAX {
            let file_name = format!("{i}.txt");
            writer.start_file(&*file_name, options)?;
            writer.write_all(i.to_string().as_bytes())?;
        }

        let mut reader = ZipArchive::new(writer.finish()?)?;
        for i in 0..=u16::MAX {
            let expected_name = format!("{i}.txt");
            let expected_contents = i.to_string();
            let expected_contents = expected_contents.as_bytes();
            let mut file = reader.by_name(&expected_name)?;
            let mut contents = Vec::with_capacity(expected_contents.len());
            file.read_to_end(&mut contents)?;
            assert_eq!(contents, expected_contents);
            drop(file);
            contents.clear();
            let mut file = reader.by_index(i as usize)?;
            file.read_to_end(&mut contents)?;
            assert_eq!(contents, expected_contents);
        }
        Ok(())
    }

    /// Symlinks being extracted shouldn't be followed out of the destination directory.
    #[test]
    fn test_cannot_symlink_outside_destination() -> ZipResult<()> {
        use std::fs::create_dir;

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.add_symlink("symlink/", "../dest-sibling/", SimpleFileOptions::default())?;
        writer.start_file("symlink/dest-file", SimpleFileOptions::default())?;
        let mut reader = writer.finish_into_readable()?;
        let dest_parent = TempDir::with_prefix("read__test_cannot_symlink_outside_destination")?;
        let dest_sibling = dest_parent.path().join("dest-sibling");
        create_dir(&dest_sibling)?;
        let dest = dest_parent.path().join("dest");
        create_dir(&dest)?;
        assert!(reader.extract(dest).is_err());
        assert!(!dest_sibling.join("dest-file").exists());
        Ok(())
    }

    #[test]
    fn test_can_create_destination() -> ZipResult<()> {
        let mut reader =
            ZipArchive::new(Cursor::new(include_bytes!("../tests/data/mimetype.zip")))?;
        let dest = TempDir::with_prefix("read__test_can_create_destination")?;
        reader.extract(&dest)?;
        assert!(dest.path().join("mimetype").exists());
        Ok(())
    }

    #[test]
    fn test_central_directory_not_at_end() -> ZipResult<()> {
        let mut reader = ZipArchive::new(Cursor::new(include_bytes!("../tests/data/omni.ja")))?;
        let mut file = reader.by_name("chrome.manifest")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?; // ensures valid UTF-8
        assert!(!contents.is_empty(), "chrome.manifest should not be empty");
        drop(file);
        for i in 0..reader.len() {
            let mut file = reader.by_index(i)?;
            // Attempt to read a small portion or all of each file to ensure it's accessible
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            assert_eq!(
                buffer.len(),
                file.size() as usize,
                "File size mismatch for {}",
                file.name()
            );
        }
        Ok(())
    }

    #[test]
    fn test_ignore_encryption_flag() -> ZipResult<()> {
        let mut reader = ZipArchive::new(Cursor::new(include_bytes!(
            "../tests/data/ignore_encryption_flag.zip"
        )))?;

        // Get the file entry by ignoring its encryption flag.
        let mut file =
            reader.by_index_with_options(0, ZipReadOptions::new().ignore_encryption_flag(true))?;
        let mut contents = String::new();
        assert_eq!(file.name(), "plaintext.txt");

        // The file claims it is encrypted, but it is not.
        assert!(file.encrypted());
        file.read_to_string(&mut contents)?; // ensures valid UTF-8
        assert_eq!(contents, "This file is not encrypted.\n");
        Ok(())
    }
}
