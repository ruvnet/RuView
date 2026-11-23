//! Types for creating ZIP archives

use crate::compression::CompressionMethod;
use crate::read::{central_header_to_zip_file, ZipArchive, ZipFile};
use crate::result::{ZipError, ZipResult};
use crate::spec;
use crate::types::{AtomicU64, DateTime, System, ZipFileData, DEFAULT_VERSION};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use crc32fast::Hasher;
use std::convert::TryInto;
use std::default::Default;
use std::io;
use std::io::prelude::*;
use std::mem;

#[cfg(any(
    feature = "deflate",
    feature = "deflate-miniz",
    feature = "deflate-zlib"
))]
use flate2::write::DeflateEncoder;

#[cfg(feature = "bzip2")]
use bzip2::write::BzEncoder;

#[cfg(feature = "time")]
use time::OffsetDateTime;

#[cfg(feature = "zstd")]
use zstd::stream::write::Encoder as ZstdEncoder;

enum MaybeEncrypted<W> {
    Unencrypted(W),
    Encrypted(crate::zipcrypto::ZipCryptoWriter<W>),
}
impl<W: Write> Write for MaybeEncrypted<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            MaybeEncrypted::Unencrypted(w) => w.write(buf),
            MaybeEncrypted::Encrypted(w) => w.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            MaybeEncrypted::Unencrypted(w) => w.flush(),
            MaybeEncrypted::Encrypted(w) => w.flush(),
        }
    }
}
enum GenericZipWriter<W: Write + io::Seek> {
    Closed,
    Storer(MaybeEncrypted<W>),
    #[cfg(any(
        feature = "deflate",
        feature = "deflate-miniz",
        feature = "deflate-zlib"
    ))]
    Deflater(DeflateEncoder<MaybeEncrypted<W>>),
    #[cfg(feature = "bzip2")]
    Bzip2(BzEncoder<MaybeEncrypted<W>>),
    #[cfg(feature = "zstd")]
    Zstd(ZstdEncoder<'static, MaybeEncrypted<W>>),
}
// Put the struct declaration in a private module to convince rustdoc to display ZipWriter nicely
pub(crate) mod zip_writer {
    use super::*;
    /// ZIP archive generator
    ///
    /// Handles the bookkeeping involved in building an archive, and provides an
    /// API to edit its contents.
    ///
    /// ```
    /// # fn doit() -> zip::result::ZipResult<()>
    /// # {
    /// # use zip::ZipWriter;
    /// use std::io::Write;
    /// use zip::write::FileOptions;
    ///
    /// // We use a buffer here, though you'd normally use a `File`
    /// let mut buf = [0; 65536];
    /// let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf[..]));
    ///
    /// let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    /// zip.start_file("hello_world.txt", options)?;
    /// zip.write(b"Hello, World!")?;
    ///
    /// // Apply the changes you've made.
    /// // Dropping the `ZipWriter` will have the same effect, but may silently fail
    /// zip.finish()?;
    ///
    /// # Ok(())
    /// # }
    /// # doit().unwrap();
    /// ```
    pub struct ZipWriter<W: Write + io::Seek> {
        pub(super) inner: GenericZipWriter<W>,
        pub(super) files: Vec<ZipFileData>,
        pub(super) stats: ZipWriterStats,
        pub(super) writing_to_file: bool,
        pub(super) writing_to_extra_field: bool,
        pub(super) writing_to_central_extra_field_only: bool,
        pub(super) writing_raw: bool,
        pub(super) comment: Vec<u8>,
    }
}
pub use zip_writer::ZipWriter;

#[derive(Default)]
struct ZipWriterStats {
    hasher: Hasher,
    start: u64,
    bytes_written: u64,
}

struct ZipRawValues {
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
}

/// Metadata for a file to be written
#[derive(Copy, Clone)]
pub struct FileOptions {
    compression_method: CompressionMethod,
    compression_level: Option<i32>,
    last_modified_time: DateTime,
    permissions: Option<u32>,
    large_file: bool,
    encrypt_with: Option<crate::zipcrypto::ZipCryptoKeys>,
}

impl FileOptions {
    /// Set the compression method for the new file
    ///
    /// The default is `CompressionMethod::Deflated`. If the deflate compression feature is
    /// disabled, `CompressionMethod::Stored` becomes the default.
    #[must_use]
    pub fn compression_method(mut self, method: CompressionMethod) -> FileOptions {
        self.compression_method = method;
        self
    }

    /// Set the compression level for the new file
    ///
    /// `None` value specifies default compression level.
    ///
    /// Range of values depends on compression method:
    /// * `Deflated`: 0 - 9. Default is 6
    /// * `Bzip2`: 0 - 9. Default is 6
    /// * `Zstd`: -7 - 22, with zero being mapped to default level. Default is 3
    /// * others: only `None` is allowed
    #[must_use]
    pub fn compression_level(mut self, level: Option<i32>) -> FileOptions {
        self.compression_level = level;
        self
    }

    /// Set the last modified time
    ///
    /// The default is the current timestamp if the 'time' feature is enabled, and 1980-01-01
    /// otherwise
    #[must_use]
    pub fn last_modified_time(mut self, mod_time: DateTime) -> FileOptions {
        self.last_modified_time = mod_time;
        self
    }

    /// Set the permissions for the new file.
    ///
    /// The format is represented with unix-style permissions.
    /// The default is `0o644`, which represents `rw-r--r--` for files,
    /// and `0o755`, which represents `rwxr-xr-x` for directories.
    ///
    /// This method only preserves the file permissions bits (via a `& 0o777`) and discards
    /// higher file mode bits. So it cannot be used to denote an entry as a directory,
    /// symlink, or other special file type.
    #[must_use]
    pub fn unix_permissions(mut self, mode: u32) -> FileOptions {
        self.permissions = Some(mode & 0o777);
        self
    }

    /// Set whether the new file's compressed and uncompressed size is less than 4 GiB.
    ///
    /// If set to `false` and the file exceeds the limit, an I/O error is thrown. If set to `true`,
    /// readers will require ZIP64 support and if the file does not exceed the limit, 20 B are
    /// wasted. The default is `false`.
    #[must_use]
    pub fn large_file(mut self, large: bool) -> FileOptions {
        self.large_file = large;
        self
    }
    pub(crate) fn with_deprecated_encryption(mut self, password: &[u8]) -> FileOptions {
        self.encrypt_with = Some(crate::zipcrypto::ZipCryptoKeys::derive(password));
        self
    }
}

impl Default for FileOptions {
    /// Construct a new FileOptions object
    fn default() -> Self {
        Self {
            #[cfg(any(
                feature = "deflate",
                feature = "deflate-miniz",
                feature = "deflate-zlib"
            ))]
            compression_method: CompressionMethod::Deflated,
            #[cfg(not(any(
                feature = "deflate",
                feature = "deflate-miniz",
                feature = "deflate-zlib"
            )))]
            compression_method: CompressionMethod::Stored,
            compression_level: None,
            #[cfg(feature = "time")]
            last_modified_time: OffsetDateTime::now_utc().try_into().unwrap_or_default(),
            #[cfg(not(feature = "time"))]
            last_modified_time: DateTime::default(),
            permissions: None,
            large_file: false,
            encrypt_with: None,
        }
    }
}

impl<W: Write + io::Seek> Write for ZipWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.writing_to_file {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "No file has been started",
            ));
        }
        match self.inner.ref_mut() {
            Some(ref mut w) => {
                if self.writing_to_extra_field {
                    self.files.last_mut().unwrap().extra_field.write(buf)
                } else {
                    let write_result = w.write(buf);
                    if let Ok(count) = write_result {
                        self.stats.update(&buf[0..count]);
                        if self.stats.bytes_written > spec::ZIP64_BYTES_THR
                            && !self.files.last_mut().unwrap().large_file
                        {
                            let _inner = mem::replace(&mut self.inner, GenericZipWriter::Closed);
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                "Large file option has not been set",
                            ));
                        }
                    }
                    write_result
                }
            }
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "ZipWriter was already closed",
            )),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.ref_mut() {
            Some(ref mut w) => w.flush(),
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "ZipWriter was already closed",
            )),
        }
    }
}

impl ZipWriterStats {
    fn update(&mut self, buf: &[u8]) {
        self.hasher.update(buf);
        self.bytes_written += buf.len() as u64;
    }
}

impl<A: Read + Write + io::Seek> ZipWriter<A> {
    /// Initializes the archive from an existing ZIP archive, making it ready for append.
    pub fn new_append(mut readwriter: A) -> ZipResult<ZipWriter<A>> {
        let (footer, cde_start_pos) = spec::CentralDirectoryEnd::find_and_parse(&mut readwriter)?;

        if footer.disk_number != footer.disk_with_central_directory {
            return Err(ZipError::UnsupportedArchive(
                "Support for multi-disk files is not implemented",
            ));
        }

        let (archive_offset, directory_start, number_of_files) =
            ZipArchive::get_directory_counts(&mut readwriter, &footer, cde_start_pos)?;

        if readwriter
            .seek(io::SeekFrom::Start(directory_start))
            .is_err()
        {
            return Err(ZipError::InvalidArchive(
                "Could not seek to start of central directory",
            ));
        }

        let files = (0..number_of_files)
            .map(|_| central_header_to_zip_file(&mut readwriter, archive_offset))
            .collect::<Result<Vec<_>, _>>()?;

        let _ = readwriter.seek(io::SeekFrom::Start(directory_start)); // seek directory_start to overwrite it

        Ok(ZipWriter {
            inner: GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(readwriter)),
            files,
            stats: Default::default(),
            writing_to_file: false,
            writing_to_extra_field: false,
            writing_to_central_extra_field_only: false,
            comment: footer.zip_file_comment,
            writing_raw: true, // avoid recomputing the last file's header
        })
    }
}

impl<W: Write + io::Seek> ZipWriter<W> {
    /// Initializes the archive.
    ///
    /// Before writing to this object, the [`ZipWriter::start_file`] function should be called.
    pub fn new(inner: W) -> ZipWriter<W> {
        ZipWriter {
            inner: GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(inner)),
            files: Vec::new(),
            stats: Default::default(),
            writing_to_file: false,
            writing_to_extra_field: false,
            writing_to_central_extra_field_only: false,
            writing_raw: false,
            comment: Vec::new(),
        }
    }

    /// Set ZIP archive comment.
    pub fn set_comment<S>(&mut self, comment: S)
    where
        S: Into<String>,
    {
        self.set_raw_comment(comment.into().into())
    }

    /// Set ZIP archive comment.
    ///
    /// This sets the raw bytes of the comment. The comment
    /// is typically expected to be encoded in UTF-8
    pub fn set_raw_comment(&mut self, comment: Vec<u8>) {
        self.comment = comment;
    }

    /// Start a new file for with the requested options.
    fn start_entry<S>(
        &mut self,
        name: S,
        options: FileOptions,
        raw_values: Option<ZipRawValues>,
    ) -> ZipResult<()>
    where
        S: Into<String>,
    {
        self.finish_file()?;

        let raw_values = raw_values.unwrap_or(ZipRawValues {
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
        });

        {
            let writer = self.inner.get_plain();
            let header_start = writer.stream_position()?;

            let permissions = options.permissions.unwrap_or(0o100644);
            let mut file = ZipFileData {
                system: System::Unix,
                version_made_by: DEFAULT_VERSION,
                encrypted: options.encrypt_with.is_some(),
                using_data_descriptor: false,
                compression_method: options.compression_method,
                compression_level: options.compression_level,
                last_modified_time: options.last_modified_time,
                crc32: raw_values.crc32,
                compressed_size: raw_values.compressed_size,
                uncompressed_size: raw_values.uncompressed_size,
                file_name: name.into(),
                file_name_raw: Vec::new(), // Never used for saving
                extra_field: Vec::new(),
                file_comment: String::new(),
                header_start,
                data_start: AtomicU64::new(0),
                central_header_start: 0,
                external_attributes: permissions << 16,
                large_file: options.large_file,
                aes_mode: None,
            };
            write_local_file_header(writer, &file)?;

            let header_end = writer.stream_position()?;
            self.stats.start = header_end;
            *file.data_start.get_mut() = header_end;

            self.stats.bytes_written = 0;
            self.stats.hasher = Hasher::new();

            self.files.push(file);
        }
        if let Some(keys) = options.encrypt_with {
            let mut zipwriter = crate::zipcrypto::ZipCryptoWriter { writer: core::mem::replace(&mut self.inner, GenericZipWriter::Closed).unwrap(), buffer: vec![], keys };
            let mut crypto_header = [0u8; 12];

            zipwriter.write_all(&crypto_header)?;
            self.inner = GenericZipWriter::Storer(MaybeEncrypted::Encrypted(zipwriter));
        }
        Ok(())
    }

    fn finish_file(&mut self) -> ZipResult<()> {
        if self.writing_to_extra_field {
            // Implicitly calling [`ZipWriter::end_extra_data`] for empty files.
            self.end_extra_data()?;
        }
        self.inner.switch_to(CompressionMethod::Stored, None)?;
        match core::mem::replace(&mut self.inner, GenericZipWriter::Closed) {
            GenericZipWriter::Storer(MaybeEncrypted::Encrypted(writer)) => {
                let crc32 = self.stats.hasher.clone().finalize();
                self.inner = GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(writer.finish(crc32)?))
            }
            GenericZipWriter::Storer(w) => self.inner = GenericZipWriter::Storer(w),
            _ => unreachable!()
        }
        let writer = self.inner.get_plain();

        if !self.writing_raw {
            let file = match self.files.last_mut() {
                None => return Ok(()),
                Some(f) => f,
            };
            file.crc32 = self.stats.hasher.clone().finalize();
            file.uncompressed_size = self.stats.bytes_written;

            let file_end = writer.stream_position()?;
            file.compressed_size = file_end - self.stats.start;

            update_local_file_header(writer, file)?;
            writer.seek(io::SeekFrom::Start(file_end))?;
        }

        self.writing_to_file = false;
        self.writing_raw = false;
        Ok(())
    }

    /// Create a file in the archive and start writing its' contents.
    ///
    /// The data should be written using the [`io::Write`] implementation on this [`ZipWriter`]
    pub fn start_file<S>(&mut self, name: S, mut options: FileOptions) -> ZipResult<()>
    where
        S: Into<String>,
    {
        if options.permissions.is_none() {
            options.permissions = Some(0o644);
        }
        *options.permissions.as_mut().unwrap() |= 0o100000;
        self.start_entry(name, options, None)?;
        self.inner
            .switch_to(options.compression_method, options.compression_level)?;
        self.writing_to_file = true;
        Ok(())
    }

    /// Starts a file, taking a Path as argument.
    ///
    /// This function ensures that the '/' path separator is used. It also ignores all non 'Normal'
    /// Components, such as a starting '/' or '..' and '.'.
    #[deprecated(
        since = "0.5.7",
        note = "by stripping `..`s from the path, the meaning of paths can change. Use `start_file` instead."
    )]
    pub fn start_file_from_path(
        &mut self,
        path: &std::path::Path,
        options: FileOptions,
    ) -> ZipResult<()> {
        self.start_file(path_to_string(path), options)
    }

    /// Create an aligned file in the archive and start writing its' contents.
    ///
    /// Returns the number of padding bytes required to align the file.
    ///
    /// The data should be written using the [`io::Write`] implementation on this [`ZipWriter`]
    pub fn start_file_aligned<S>(
        &mut self,
        name: S,
        options: FileOptions,
        align: u16,
    ) -> Result<u64, ZipError>
    where
        S: Into<String>,
    {
        let data_start = self.start_file_with_extra_data(name, options)?;
        let align = align as u64;
        if align > 1 && data_start % align != 0 {
            let pad_length = (align - (data_start + 4) % align) % align;
            let pad = vec![0; pad_length as usize];
            self.write_all(b"za").map_err(ZipError::from)?; // 0x617a
            self.write_u16::<LittleEndian>(pad.len() as u16)
                .map_err(ZipError::from)?;
            self.write_all(&pad).map_err(ZipError::from)?;
            assert_eq!(self.end_local_start_central_extra_data()? % align, 0);
        }
        let extra_data_end = self.end_extra_data()?;
        Ok(extra_data_end - data_start)
    }

    /// Create a file in the archive and start writing its extra data first.
    ///
    /// Finish writing extra data and start writing file data with [`ZipWriter::end_extra_data`].
    /// Optionally, distinguish local from central extra data with
    /// [`ZipWriter::end_local_start_central_extra_data`].
    ///
    /// Returns the preliminary starting offset of the file data without any extra data allowing to
    /// align the file data by calculating a pad length to be prepended as part of the extra data.
    ///
    /// The data should be written using the [`io::Write`] implementation on this [`ZipWriter`]
    ///
    /// ```
    /// use byteorder::{LittleEndian, WriteBytesExt};
    /// use zip::{ZipArchive, ZipWriter, result::ZipResult};
    /// use zip::{write::FileOptions, CompressionMethod};
    /// use std::io::{Write, Cursor};
    ///
    /// # fn main() -> ZipResult<()> {
    /// let mut archive = Cursor::new(Vec::new());
    ///
    /// {
    ///     let mut zip = ZipWriter::new(&mut archive);
    ///     let options = FileOptions::default()
    ///         .compression_method(CompressionMethod::Stored);
    ///
    ///     zip.start_file_with_extra_data("identical_extra_data.txt", options)?;
    ///     let extra_data = b"local and central extra data";
    ///     zip.write_u16::<LittleEndian>(0xbeef)?;
    ///     zip.write_u16::<LittleEndian>(extra_data.len() as u16)?;
    ///     zip.write_all(extra_data)?;
    ///     zip.end_extra_data()?;
    ///     zip.write_all(b"file data")?;
    ///
    ///     let data_start = zip.start_file_with_extra_data("different_extra_data.txt", options)?;
    ///     let extra_data = b"local extra data";
    ///     zip.write_u16::<LittleEndian>(0xbeef)?;
    ///     zip.write_u16::<LittleEndian>(extra_data.len() as u16)?;
    ///     zip.write_all(extra_data)?;
    ///     let data_start = data_start as usize + 4 + extra_data.len() + 4;
    ///     let align = 64;
    ///     let pad_length = (align - data_start % align) % align;
    ///     assert_eq!(pad_length, 19);
    ///     zip.write_u16::<LittleEndian>(0xdead)?;
    ///     zip.write_u16::<LittleEndian>(pad_length as u16)?;
    ///     zip.write_all(&vec![0; pad_length])?;
    ///     let data_start = zip.end_local_start_central_extra_data()?;
    ///     assert_eq!(data_start as usize % align, 0);
    ///     let extra_data = b"central extra data";
    ///     zip.write_u16::<LittleEndian>(0xbeef)?;
    ///     zip.write_u16::<LittleEndian>(extra_data.len() as u16)?;
    ///     zip.write_all(extra_data)?;
    ///     zip.end_extra_data()?;
    ///     zip.write_all(b"file data")?;
    ///
    ///     zip.finish()?;
    /// }
    ///
    /// let mut zip = ZipArchive::new(archive)?;
    /// assert_eq!(&zip.by_index(0)?.extra_data()[4..], b"local and central extra data");
    /// assert_eq!(&zip.by_index(1)?.extra_data()[4..], b"central extra data");
    /// # Ok(())
    /// # }
    /// ```
    pub fn start_file_with_extra_data<S>(
        &mut self,
        name: S,
        mut options: FileOptions,
    ) -> ZipResult<u64>
    where
        S: Into<String>,
    {
        if options.permissions.is_none() {
            options.permissions = Some(0o644);
        }
        *options.permissions.as_mut().unwrap() |= 0o100000;
        self.start_entry(name, options, None)?;
        self.writing_to_file = true;
        self.writing_to_extra_field = true;
        Ok(self.files.last().unwrap().data_start.load())
    }

    /// End local and start central extra data. Requires [`ZipWriter::start_file_with_extra_data`].
    ///
    /// Returns the final starting offset of the file data.
    pub fn end_local_start_central_extra_data(&mut self) -> ZipResult<u64> {
        let data_start = self.end_extra_data()?;
        self.files.last_mut().unwrap().extra_field.clear();
        self.writing_to_extra_field = true;
        self.writing_to_central_extra_field_only = true;
        Ok(data_start)
    }

    /// End extra data and start file data. Requires [`ZipWriter::start_file_with_extra_data`].
    ///
    /// Returns the final starting offset of the file data.
    pub fn end_extra_data(&mut self) -> ZipResult<u64> {
        // Require `start_file_with_extra_data()`. Ensures `file` is some.
        if !self.writing_to_extra_field {
            return Err(ZipError::Io(io::Error::new(
                io::ErrorKind::Other,
                "Not writing to extra field",
            )));
        }
        let file = self.files.last_mut().unwrap();

        validate_extra_data(file)?;

        let data_start = file.data_start.get_mut();

        if !self.writing_to_central_extra_field_only {
            let writer = self.inner.get_plain();

            // Append extra data to local file header and keep it for central file header.
            writer.write_all(&file.extra_field)?;

            // Update final `data_start`.
            let header_end = *data_start + file.extra_field.len() as u64;
            self.stats.start = header_end;
            *data_start = header_end;

            // Update extra field length in local file header.
            let extra_field_length =
                if file.large_file { 20 } else { 0 } + file.extra_field.len() as u16;
            writer.seek(io::SeekFrom::Start(file.header_start + 28))?;
            writer.write_u16::<LittleEndian>(extra_field_length)?;
            writer.seek(io::SeekFrom::Start(header_end))?;

            self.inner
                .switch_to(file.compression_method, file.compression_level)?;
        }

        self.writing_to_extra_field = false;
        self.writing_to_central_extra_field_only = false;
        Ok(*data_start)
    }

    /// Add a new file using the already compressed data from a ZIP file being read and renames it, this
    /// allows faster copies of the `ZipFile` since there is no need to decompress and compress it again.
    /// Any `ZipFile` metadata is copied and not checked, for example the file CRC.

    /// ```no_run
    /// use std::fs::File;
    /// use std::io::{Read, Seek, Write};
    /// use zip::{ZipArchive, ZipWriter};
    ///
    /// fn copy_rename<R, W>(
    ///     src: &mut ZipArchive<R>,
    ///     dst: &mut ZipWriter<W>,
    /// ) -> zip::result::ZipResult<()>
    /// where
    ///     R: Read + Seek,
    ///     W: Write + Seek,
    /// {
    ///     // Retrieve file entry by name
    ///     let file = src.by_name("src_file.txt")?;
    ///
    ///     // Copy and rename the previously obtained file entry to the destination zip archive
    ///     dst.raw_copy_file_rename(file, "new_name.txt")?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn raw_copy_file_rename<S>(&mut self, mut file: ZipFile, name: S) -> ZipResult<()>
    where
        S: Into<String>,
    {
        let mut options = FileOptions::default()
            .large_file(file.compressed_size().max(file.size()) > spec::ZIP64_BYTES_THR)
            .last_modified_time(file.last_modified())
            .compression_method(file.compression());
        if let Some(perms) = file.unix_mode() {
            options = options.unix_permissions(perms);
        }

        let raw_values = ZipRawValues {
            crc32: file.crc32(),
            compressed_size: file.compressed_size(),
            uncompressed_size: file.size(),
        };

        self.start_entry(name, options, Some(raw_values))?;
        self.writing_to_file = true;
        self.writing_raw = true;

        io::copy(file.get_raw_reader(), self)?;

        Ok(())
    }

    /// Add a new file using the already compressed data from a ZIP file being read, this allows faster
    /// copies of the `ZipFile` since there is no need to decompress and compress it again. Any `ZipFile`
    /// metadata is copied and not checked, for example the file CRC.
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::{Read, Seek, Write};
    /// use zip::{ZipArchive, ZipWriter};
    ///
    /// fn copy<R, W>(src: &mut ZipArchive<R>, dst: &mut ZipWriter<W>) -> zip::result::ZipResult<()>
    /// where
    ///     R: Read + Seek,
    ///     W: Write + Seek,
    /// {
    ///     // Retrieve file entry by name
    ///     let file = src.by_name("src_file.txt")?;
    ///
    ///     // Copy the previously obtained file entry to the destination zip archive
    ///     dst.raw_copy_file(file)?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn raw_copy_file(&mut self, file: ZipFile) -> ZipResult<()> {
        let name = file.name().to_owned();
        self.raw_copy_file_rename(file, name)
    }

    /// Add a directory entry.
    ///
    /// As directories have no content, you must not call [`ZipWriter::write`] before adding a new file.
    pub fn add_directory<S>(&mut self, name: S, mut options: FileOptions) -> ZipResult<()>
    where
        S: Into<String>,
    {
        if options.permissions.is_none() {
            options.permissions = Some(0o755);
        }
        *options.permissions.as_mut().unwrap() |= 0o40000;
        options.compression_method = CompressionMethod::Stored;

        let name_as_string = name.into();
        // Append a slash to the filename if it does not end with it.
        let name_with_slash = match name_as_string.chars().last() {
            Some('/') | Some('\\') => name_as_string,
            _ => name_as_string + "/",
        };

        self.start_entry(name_with_slash, options, None)?;
        self.writing_to_file = false;
        Ok(())
    }

    /// Add a directory entry, taking a Path as argument.
    ///
    /// This function ensures that the '/' path separator is used. It also ignores all non 'Normal'
    /// Components, such as a starting '/' or '..' and '.'.
    #[deprecated(
        since = "0.5.7",
        note = "by stripping `..`s from the path, the meaning of paths can change. Use `add_directory` instead."
    )]
    pub fn add_directory_from_path(
        &mut self,
        path: &std::path::Path,
        options: FileOptions,
    ) -> ZipResult<()> {
        self.add_directory(path_to_string(path), options)
    }

    /// Finish the last file and write all other zip-structures
    ///
    /// This will return the writer, but one should normally not append any data to the end of the file.
    /// Note that the zipfile will also be finished on drop.
    pub fn finish(&mut self) -> ZipResult<W> {
        self.finalize()?;
        let inner = mem::replace(&mut self.inner, GenericZipWriter::Closed);
        Ok(inner.unwrap())
    }

    /// Add a symlink entry.
    ///
    /// The zip archive will contain an entry for path `name` which is a symlink to `target`.
    ///
    /// No validation or normalization of the paths is performed. For best results,
    /// callers should normalize `\` to `/` and ensure symlinks are relative to other
    /// paths within the zip archive.
    ///
    /// WARNING: not all zip implementations preserve symlinks on extract. Some zip
    /// implementations may materialize a symlink as a regular file, possibly with the
    /// content incorrectly set to the symlink target. For maximum portability, consider
    /// storing a regular file instead.
    pub fn add_symlink<N, T>(
        &mut self,
        name: N,
        target: T,
        mut options: FileOptions,
    ) -> ZipResult<()>
    where
        N: Into<String>,
        T: Into<String>,
    {
        if options.permissions.is_none() {
            options.permissions = Some(0o777);
        }
        *options.permissions.as_mut().unwrap() |= 0o120000;
        // The symlink target is stored as file content. And compressing the target path
        // likely wastes space. So always store.
        options.compression_method = CompressionMethod::Stored;

        self.start_entry(name, options, None)?;
        self.writing_to_file = true;
        self.write_all(target.into().as_bytes())?;
        self.writing_to_file = false;

        Ok(())
    }

    fn finalize(&mut self) -> ZipResult<()> {
        self.finish_file()?;

        {
            let writer = self.inner.get_plain();

            let central_start = writer.stream_position()?;
            for file in self.files.iter() {
                write_central_directory_header(writer, file)?;
            }
            let central_size = writer.stream_position()? - central_start;

            if self.files.len() > spec::ZIP64_ENTRY_THR
                || central_size.max(central_start) > spec::ZIP64_BYTES_THR
            {
                let zip64_footer = spec::Zip64CentralDirectoryEnd {
                    version_made_by: DEFAULT_VERSION as u16,
                    version_needed_to_extract: DEFAULT_VERSION as u16,
                    disk_number: 0,
                    disk_with_central_directory: 0,
                    number_of_files_on_this_disk: self.files.len() as u64,
                    number_of_files: self.files.len() as u64,
                    central_directory_size: central_size,
                    central_directory_offset: central_start,
                };

                zip64_footer.write(writer)?;

                let zip64_footer = spec::Zip64CentralDirectoryEndLocator {
                    disk_with_central_directory: 0,
                    end_of_central_directory_offset: central_start + central_size,
                    number_of_disks: 1,
                };

                zip64_footer.write(writer)?;
            }

            let number_of_files = self.files.len().min(spec::ZIP64_ENTRY_THR) as u16;
            let footer = spec::CentralDirectoryEnd {
                disk_number: 0,
                disk_with_central_directory: 0,
                zip_file_comment: self.comment.clone(),
                number_of_files_on_this_disk: number_of_files,
                number_of_files,
                central_directory_size: central_size.min(spec::ZIP64_BYTES_THR) as u32,
                central_directory_offset: central_start.min(spec::ZIP64_BYTES_THR) as u32,
            };

            footer.write(writer)?;
        }

        Ok(())
    }
}

impl<W: Write + io::Seek> Drop for ZipWriter<W> {
    fn drop(&mut self) {
        if !self.inner.is_closed() {
            if let Err(e) = self.finalize() {
                let _ = write!(io::stderr(), "ZipWriter drop failed: {e:?}");
            }
        }
    }
}

impl<W: Write + io::Seek> GenericZipWriter<W> {
    fn switch_to(
        &mut self,
        compression: CompressionMethod,
        compression_level: Option<i32>,
    ) -> ZipResult<()> {
        match self.current_compression() {
            Some(method) if method == compression => return Ok(()),
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ZipWriter was already closed",
                )
                .into())
            }
            _ => {}
        }

        let bare = match mem::replace(self, GenericZipWriter::Closed) {
            GenericZipWriter::Storer(w) => w,
            #[cfg(any(
                feature = "deflate",
                feature = "deflate-miniz",
                feature = "deflate-zlib"
            ))]
            GenericZipWriter::Deflater(w) => w.finish()?,
            #[cfg(feature = "bzip2")]
            GenericZipWriter::Bzip2(w) => w.finish()?,
            #[cfg(feature = "zstd")]
            GenericZipWriter::Zstd(w) => w.finish()?,
            GenericZipWriter::Closed => {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ZipWriter was already closed",
                )
                .into())
            }
        };

        *self = {
            #[allow(deprecated)]
            match compression {
                CompressionMethod::Stored => {
                    if compression_level.is_some() {
                        return Err(ZipError::UnsupportedArchive(
                            "Unsupported compression level",
                        ));
                    }

                    GenericZipWriter::Storer(bare)
                }
                #[cfg(any(
                    feature = "deflate",
                    feature = "deflate-miniz",
                    feature = "deflate-zlib"
                ))]
                CompressionMethod::Deflated => GenericZipWriter::Deflater(DeflateEncoder::new(
                    bare,
                    flate2::Compression::new(
                        clamp_opt(
                            compression_level
                                .unwrap_or(flate2::Compression::default().level() as i32),
                            deflate_compression_level_range(),
                        )
                        .ok_or(ZipError::UnsupportedArchive(
                            "Unsupported compression level",
                        ))? as u32,
                    ),
                )),
                #[cfg(feature = "bzip2")]
                CompressionMethod::Bzip2 => GenericZipWriter::Bzip2(BzEncoder::new(
                    bare,
                    bzip2::Compression::new(
                        clamp_opt(
                            compression_level
                                .unwrap_or(bzip2::Compression::default().level() as i32),
                            bzip2_compression_level_range(),
                        )
                        .ok_or(ZipError::UnsupportedArchive(
                            "Unsupported compression level",
                        ))? as u32,
                    ),
                )),
                CompressionMethod::AES => {
                    return Err(ZipError::UnsupportedArchive(
                        "AES compression is not supported for writing",
                    ))
                }
                #[cfg(feature = "zstd")]
                CompressionMethod::Zstd => GenericZipWriter::Zstd(
                    ZstdEncoder::new(
                        bare,
                        clamp_opt(
                            compression_level.unwrap_or(zstd::DEFAULT_COMPRESSION_LEVEL),
                            zstd::compression_level_range(),
                        )
                        .ok_or(ZipError::UnsupportedArchive(
                            "Unsupported compression level",
                        ))?,
                    )
                    .unwrap(),
                ),
                CompressionMethod::Unsupported(..) => {
                    return Err(ZipError::UnsupportedArchive("Unsupported compression"))
                }
            }
        };

        Ok(())
    }

    fn ref_mut(&mut self) -> Option<&mut dyn Write> {
        match *self {
            GenericZipWriter::Storer(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(any(
                feature = "deflate",
                feature = "deflate-miniz",
                feature = "deflate-zlib"
            ))]
            GenericZipWriter::Deflater(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(feature = "bzip2")]
            GenericZipWriter::Bzip2(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(feature = "zstd")]
            GenericZipWriter::Zstd(ref mut w) => Some(w as &mut dyn Write),
            GenericZipWriter::Closed => None,
        }
    }

    fn is_closed(&self) -> bool {
        matches!(*self, GenericZipWriter::Closed)
    }

    fn get_plain(&mut self) -> &mut W {
        match *self {
            GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(ref mut w)) => w,
            _ => panic!("Should have switched to stored and unencrypted beforehand"),
        }
    }

    fn current_compression(&self) -> Option<CompressionMethod> {
        match *self {
            GenericZipWriter::Storer(..) => Some(CompressionMethod::Stored),
            #[cfg(any(
                feature = "deflate",
                feature = "deflate-miniz",
                feature = "deflate-zlib"
            ))]
            GenericZipWriter::Deflater(..) => Some(CompressionMethod::Deflated),
            #[cfg(feature = "bzip2")]
            GenericZipWriter::Bzip2(..) => Some(CompressionMethod::Bzip2),
            #[cfg(feature = "zstd")]
            GenericZipWriter::Zstd(..) => Some(CompressionMethod::Zstd),
            GenericZipWriter::Closed => None,
        }
    }

    fn unwrap(self) -> W {
        match self {
            GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(w)) => w,
            _ => panic!("Should have switched to stored and unencrypted beforehand"),
        }
    }
}

#[cfg(any(
    feature = "deflate",
    feature = "deflate-miniz",
    feature = "deflate-zlib"
))]
fn deflate_compression_level_range() -> std::ops::RangeInclusive<i32> {
    let min = flate2::Compression::none().level() as i32;
    let max = flate2::Compression::best().level() as i32;
    min..=max
}

#[cfg(feature = "bzip2")]
fn bzip2_compression_level_range() -> std::ops::RangeInclusive<i32> {
    let min = bzip2::Compression::none().level() as i32;
    let max = bzip2::Compression::best().level() as i32;
    min..=max
}

#[cfg(any(
    feature = "deflate",
    feature = "deflate-miniz",
    feature = "deflate-zlib",
    feature = "bzip2",
    feature = "zstd"
))]
fn clamp_opt<T: Ord + Copy>(value: T, range: std::ops::RangeInclusive<T>) -> Option<T> {
    if range.contains(&value) {
        Some(value)
    } else {
        None
    }
}

fn write_local_file_header<T: Write>(writer: &mut T, file: &ZipFileData) -> ZipResult<()> {
    // local file header signature
    writer.write_u32::<LittleEndian>(spec::LOCAL_FILE_HEADER_SIGNATURE)?;
    // version needed to extract
    writer.write_u16::<LittleEndian>(file.version_needed())?;
    // general purpose bit flag
    let flag = if !file.file_name.is_ascii() {
        1u16 << 11
    } else {
        0
    } | if file.encrypted { 1u16 << 0 } else { 0 };
    writer.write_u16::<LittleEndian>(flag)?;
    // Compression method
    #[allow(deprecated)]
    writer.write_u16::<LittleEndian>(file.compression_method.to_u16())?;
    // last mod file time and last mod file date
    writer.write_u16::<LittleEndian>(file.last_modified_time.timepart())?;
    writer.write_u16::<LittleEndian>(file.last_modified_time.datepart())?;
    // crc-32
    writer.write_u32::<LittleEndian>(file.crc32)?;
    // compressed size and uncompressed size
    if file.large_file {
        writer.write_u32::<LittleEndian>(spec::ZIP64_BYTES_THR as u32)?;
        writer.write_u32::<LittleEndian>(spec::ZIP64_BYTES_THR as u32)?;
    } else {
        writer.write_u32::<LittleEndian>(file.compressed_size as u32)?;
        writer.write_u32::<LittleEndian>(file.uncompressed_size as u32)?;
    }
    // file name length
    writer.write_u16::<LittleEndian>(file.file_name.as_bytes().len() as u16)?;
    // extra field length
    let extra_field_length = if file.large_file { 20 } else { 0 } + file.extra_field.len() as u16;
    writer.write_u16::<LittleEndian>(extra_field_length)?;
    // file name
    writer.write_all(file.file_name.as_bytes())?;
    // zip64 extra field
    if file.large_file {
        write_local_zip64_extra_field(writer, file)?;
    }

    Ok(())
}

fn update_local_file_header<T: Write + io::Seek>(
    writer: &mut T,
    file: &ZipFileData,
) -> ZipResult<()> {
    const CRC32_OFFSET: u64 = 14;
    writer.seek(io::SeekFrom::Start(file.header_start + CRC32_OFFSET))?;
    writer.write_u32::<LittleEndian>(file.crc32)?;
    if file.large_file {
        update_local_zip64_extra_field(writer, file)?;
    } else {
        // check compressed size as well as it can also be slightly larger than uncompressed size
        if file.compressed_size > spec::ZIP64_BYTES_THR {
            return Err(ZipError::Io(io::Error::new(
                io::ErrorKind::Other,
                "Large file option has not been set",
            )));
        }
        writer.write_u32::<LittleEndian>(file.compressed_size as u32)?;
        // uncompressed size is already checked on write to catch it as soon as possible
        writer.write_u32::<LittleEndian>(file.uncompressed_size as u32)?;
    }
    Ok(())
}

fn write_central_directory_header<T: Write>(writer: &mut T, file: &ZipFileData) -> ZipResult<()> {
    // buffer zip64 extra field to determine its variable length
    let mut zip64_extra_field = [0; 28];
    let zip64_extra_field_length =
        write_central_zip64_extra_field(&mut zip64_extra_field.as_mut(), file)?;

    // central file header signature
    writer.write_u32::<LittleEndian>(spec::CENTRAL_DIRECTORY_HEADER_SIGNATURE)?;
    // version made by
    let version_made_by = (file.system as u16) << 8 | (file.version_made_by as u16);
    writer.write_u16::<LittleEndian>(version_made_by)?;
    // version needed to extract
    writer.write_u16::<LittleEndian>(file.version_needed())?;
    // general puprose bit flag
    let flag = if !file.file_name.is_ascii() {
        1u16 << 11
    } else {
        0
    } | if file.encrypted { 1u16 << 0 } else { 0 };
    writer.write_u16::<LittleEndian>(flag)?;
    // compression method
    #[allow(deprecated)]
    writer.write_u16::<LittleEndian>(file.compression_method.to_u16())?;
    // last mod file time + date
    writer.write_u16::<LittleEndian>(file.last_modified_time.timepart())?;
    writer.write_u16::<LittleEndian>(file.last_modified_time.datepart())?;
    // crc-32
    writer.write_u32::<LittleEndian>(file.crc32)?;
    // compressed size
    writer.write_u32::<LittleEndian>(file.compressed_size.min(spec::ZIP64_BYTES_THR) as u32)?;
    // uncompressed size
    writer.write_u32::<LittleEndian>(file.uncompressed_size.min(spec::ZIP64_BYTES_THR) as u32)?;
    // file name length
    writer.write_u16::<LittleEndian>(file.file_name.as_bytes().len() as u16)?;
    // extra field length
    writer.write_u16::<LittleEndian>(zip64_extra_field_length + file.extra_field.len() as u16)?;
    // file comment length
    writer.write_u16::<LittleEndian>(0)?;
    // disk number start
    writer.write_u16::<LittleEndian>(0)?;
    // internal file attribytes
    writer.write_u16::<LittleEndian>(0)?;
    // external file attributes
    writer.write_u32::<LittleEndian>(file.external_attributes)?;
    // relative offset of local header
    writer.write_u32::<LittleEndian>(file.header_start.min(spec::ZIP64_BYTES_THR) as u32)?;
    // file name
    writer.write_all(file.file_name.as_bytes())?;
    // zip64 extra field
    writer.write_all(&zip64_extra_field[..zip64_extra_field_length as usize])?;
    // extra field
    writer.write_all(&file.extra_field)?;
    // file comment
    // <none>

    Ok(())
}

fn validate_extra_data(file: &ZipFileData) -> ZipResult<()> {
    let mut data = file.extra_field.as_slice();

    if data.len() > spec::ZIP64_ENTRY_THR {
        return Err(ZipError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "Extra data exceeds extra field",
        )));
    }

    while !data.is_empty() {
        let left = data.len();
        if left < 4 {
            return Err(ZipError::Io(io::Error::new(
                io::ErrorKind::Other,
                "Incomplete extra data header",
            )));
        }
        let kind = data.read_u16::<LittleEndian>()?;
        let size = data.read_u16::<LittleEndian>()? as usize;
        let left = left - 4;

        if kind == 0x0001 {
            return Err(ZipError::Io(io::Error::new(
                io::ErrorKind::Other,
                "No custom ZIP64 extra data allowed",
            )));
        }

        #[cfg(not(feature = "unreserved"))]
        {
            if kind <= 31 || EXTRA_FIELD_MAPPING.iter().any(|&mapped| mapped == kind) {
                return Err(ZipError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Extra data header ID {kind:#06} requires crate feature \"unreserved\"",
                    ),
                )));
            }
        }

        if size > left {
            return Err(ZipError::Io(io::Error::new(
                io::ErrorKind::Other,
                "Extra data size exceeds extra field",
            )));
        }

        data = &data[size..];
    }

    Ok(())
}

fn write_local_zip64_extra_field<T: Write>(writer: &mut T, file: &ZipFileData) -> ZipResult<()> {
    // This entry in the Local header MUST include BOTH original
    // and compressed file size fields.
    writer.write_u16::<LittleEndian>(0x0001)?;
    writer.write_u16::<LittleEndian>(16)?;
    writer.write_u64::<LittleEndian>(file.uncompressed_size)?;
    writer.write_u64::<LittleEndian>(file.compressed_size)?;
    // Excluded fields:
    // u32: disk start number
    Ok(())
}

fn update_local_zip64_extra_field<T: Write + io::Seek>(
    writer: &mut T,
    file: &ZipFileData,
) -> ZipResult<()> {
    let zip64_extra_field = file.header_start + 30 + file.file_name.as_bytes().len() as u64;
    writer.seek(io::SeekFrom::Start(zip64_extra_field + 4))?;
    writer.write_u64::<LittleEndian>(file.uncompressed_size)?;
    writer.write_u64::<LittleEndian>(file.compressed_size)?;
    // Excluded fields:
    // u32: disk start number
    Ok(())
}

fn write_central_zip64_extra_field<T: Write>(writer: &mut T, file: &ZipFileData) -> ZipResult<u16> {
    // The order of the fields in the zip64 extended
    // information record is fixed, but the fields MUST
    // only appear if the corresponding Local or Central
    // directory record field is set to 0xFFFF or 0xFFFFFFFF.
    let mut size = 0;
    let uncompressed_size = file.uncompressed_size > spec::ZIP64_BYTES_THR;
    let compressed_size = file.compressed_size > spec::ZIP64_BYTES_THR;
    let header_start = file.header_start > spec::ZIP64_BYTES_THR;
    if uncompressed_size {
        size += 8;
    }
    if compressed_size {
        size += 8;
    }
    if header_start {
        size += 8;
    }
    if size > 0 {
        writer.write_u16::<LittleEndian>(0x0001)?;
        writer.write_u16::<LittleEndian>(size)?;
        size += 4;

        if uncompressed_size {
            writer.write_u64::<LittleEndian>(file.uncompressed_size)?;
        }
        if compressed_size {
            writer.write_u64::<LittleEndian>(file.compressed_size)?;
        }
        if header_start {
            writer.write_u64::<LittleEndian>(file.header_start)?;
        }
        // Excluded fields:
        // u32: disk start number
    }
    Ok(size)
}

fn path_to_string(path: &std::path::Path) -> String {
    let mut path_str = String::new();
    for component in path.components() {
        if let std::path::Component::Normal(os_str) = component {
            if !path_str.is_empty() {
                path_str.push('/');
            }
            path_str.push_str(&os_str.to_string_lossy());
        }
    }
    path_str
}

#[cfg(test)]
mod test {
    use super::{FileOptions, ZipWriter};
    use crate::compression::CompressionMethod;
    use crate::types::DateTime;
    use std::io;
    use std::io::Write;

    #[test]
    fn write_empty_zip() {
        let mut writer = ZipWriter::new(io::Cursor::new(Vec::new()));
        writer.set_comment("ZIP");
        let result = writer.finish().unwrap();
        assert_eq!(result.get_ref().len(), 25);
        assert_eq!(
            *result.get_ref(),
            [80, 75, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 90, 73, 80]
        );
    }

    #[test]
    fn unix_permissions_bitmask() {
        // unix_permissions() throws away upper bits.
        let options = FileOptions::default().unix_permissions(0o120777);
        assert_eq!(options.permissions, Some(0o777));
    }

    #[test]
    fn write_zip_dir() {
        let mut writer = ZipWriter::new(io::Cursor::new(Vec::new()));
        writer
            .add_directory(
                "test",
                FileOptions::default().last_modified_time(
                    DateTime::from_date_and_time(2018, 8, 15, 20, 45, 6).unwrap(),
                ),
            )
            .unwrap();
        assert!(writer
            .write(b"writing to a directory is not allowed, and will not write any data")
            .is_err());
        let result = writer.finish().unwrap();
        assert_eq!(result.get_ref().len(), 108);
        assert_eq!(
            *result.get_ref(),
            &[
                80u8, 75, 3, 4, 20, 0, 0, 0, 0, 0, 163, 165, 15, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 5, 0, 0, 0, 116, 101, 115, 116, 47, 80, 75, 1, 2, 46, 3, 20, 0, 0, 0, 0, 0,
                163, 165, 15, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 237, 65, 0, 0, 0, 0, 116, 101, 115, 116, 47, 80, 75, 5, 6, 0, 0, 0, 0, 1, 0,
                1, 0, 51, 0, 0, 0, 35, 0, 0, 0, 0, 0,
            ] as &[u8]
        );
    }

    #[test]
    fn write_symlink_simple() {
        let mut writer = ZipWriter::new(io::Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "name",
                "target",
                FileOptions::default().last_modified_time(
                    DateTime::from_date_and_time(2018, 8, 15, 20, 45, 6).unwrap(),
                ),
            )
            .unwrap();
        assert!(writer
            .write(b"writing to a symlink is not allowed and will not write any data")
            .is_err());
        let result = writer.finish().unwrap();
        assert_eq!(result.get_ref().len(), 112);
        assert_eq!(
            *result.get_ref(),
            &[
                80u8, 75, 3, 4, 20, 0, 0, 0, 0, 0, 163, 165, 15, 77, 252, 47, 111, 70, 6, 0, 0, 0,
                6, 0, 0, 0, 4, 0, 0, 0, 110, 97, 109, 101, 116, 97, 114, 103, 101, 116, 80, 75, 1,
                2, 46, 3, 20, 0, 0, 0, 0, 0, 163, 165, 15, 77, 252, 47, 111, 70, 6, 0, 0, 0, 6, 0,
                0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 161, 0, 0, 0, 0, 110, 97, 109, 101,
                80, 75, 5, 6, 0, 0, 0, 0, 1, 0, 1, 0, 50, 0, 0, 0, 40, 0, 0, 0, 0, 0
            ] as &[u8],
        );
    }

    #[test]
    fn write_symlink_wonky_paths() {
        let mut writer = ZipWriter::new(io::Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "directory\\link",
                "/absolute/symlink\\with\\mixed/slashes",
                FileOptions::default().last_modified_time(
                    DateTime::from_date_and_time(2018, 8, 15, 20, 45, 6).unwrap(),
                ),
            )
            .unwrap();
        assert!(writer
            .write(b"writing to a symlink is not allowed and will not write any data")
            .is_err());
        let result = writer.finish().unwrap();
        assert_eq!(result.get_ref().len(), 162);
        assert_eq!(
            *result.get_ref(),
            &[
                80u8, 75, 3, 4, 20, 0, 0, 0, 0, 0, 163, 165, 15, 77, 95, 41, 81, 245, 36, 0, 0, 0,
                36, 0, 0, 0, 14, 0, 0, 0, 100, 105, 114, 101, 99, 116, 111, 114, 121, 92, 108, 105,
                110, 107, 47, 97, 98, 115, 111, 108, 117, 116, 101, 47, 115, 121, 109, 108, 105,
                110, 107, 92, 119, 105, 116, 104, 92, 109, 105, 120, 101, 100, 47, 115, 108, 97,
                115, 104, 101, 115, 80, 75, 1, 2, 46, 3, 20, 0, 0, 0, 0, 0, 163, 165, 15, 77, 95,
                41, 81, 245, 36, 0, 0, 0, 36, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255,
                161, 0, 0, 0, 0, 100, 105, 114, 101, 99, 116, 111, 114, 121, 92, 108, 105, 110,
                107, 80, 75, 5, 6, 0, 0, 0, 0, 1, 0, 1, 0, 60, 0, 0, 0, 80, 0, 0, 0, 0, 0
            ] as &[u8],
        );
    }

    #[test]
    fn write_mimetype_zip() {
        let mut writer = ZipWriter::new(io::Cursor::new(Vec::new()));
        let options = FileOptions {
            compression_method: CompressionMethod::Stored,
            compression_level: None,
            last_modified_time: DateTime::default(),
            permissions: Some(33188),
            large_file: false,
            encrypt_with: None,
        };
        writer.start_file("mimetype", options).unwrap();
        writer
            .write_all(b"application/vnd.oasis.opendocument.text")
            .unwrap();
        let result = writer.finish().unwrap();

        assert_eq!(result.get_ref().len(), 153);
        let mut v = Vec::new();
        v.extend_from_slice(include_bytes!("../tests/data/mimetype.zip"));
        assert_eq!(result.get_ref(), &v);
    }

    #[test]
    fn path_to_string() {
        let mut path = std::path::PathBuf::new();
        #[cfg(windows)]
        path.push(r"C:\");
        #[cfg(unix)]
        path.push("/");
        path.push("windows");
        path.push("..");
        path.push(".");
        path.push("system32");
        let path_str = super::path_to_string(&path);
        assert_eq!(path_str, "windows/system32");
    }
}

#[cfg(not(feature = "unreserved"))]
const EXTRA_FIELD_MAPPING: [u16; 49] = [
    0x0001, 0x0007, 0x0008, 0x0009, 0x000a, 0x000c, 0x000d, 0x000e, 0x000f, 0x0014, 0x0015, 0x0016,
    0x0017, 0x0018, 0x0019, 0x0020, 0x0021, 0x0022, 0x0023, 0x0065, 0x0066, 0x4690, 0x07c8, 0x2605,
    0x2705, 0x2805, 0x334d, 0x4341, 0x4453, 0x4704, 0x470f, 0x4b46, 0x4c41, 0x4d49, 0x4f4c, 0x5356,
    0x5455, 0x554e, 0x5855, 0x6375, 0x6542, 0x7075, 0x756e, 0x7855, 0xa11e, 0xa220, 0xfd4a, 0x9901,
    0x9902,
];
