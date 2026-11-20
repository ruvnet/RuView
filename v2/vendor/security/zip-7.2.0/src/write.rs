//! Types for creating ZIP archives

#[cfg(feature = "aes-crypto")]
use crate::aes::AesWriter;
use crate::compression::CompressionMethod;
use crate::read::{parse_single_extra_field, Config, ZipArchive, ZipFile};
use crate::result::{invalid, ZipError, ZipResult};
use crate::spec::{self, FixedSizeBlock, Zip32CDEBlock};
use crate::types::ffi::S_IFLNK;
#[cfg(feature = "aes-crypto")]
use crate::types::AesMode;
use crate::types::{
    ffi, AesVendorVersion, DateTime, Zip64ExtraFieldBlock, ZipFileData, ZipLocalEntryBlock,
    ZipRawValues, MIN_VERSION,
};
use core::default::Default;
use core::fmt::{Debug, Formatter};
use core::marker::PhantomData;
use core::mem::{self, size_of};
#[cfg(feature = "deflate-zopfli")]
use core::num::NonZeroU64;
use core::str::{from_utf8, Utf8Error};
use crc32fast::Hasher;
use indexmap::IndexMap;
use std::borrow::ToOwned;
use std::io::{self, Read, Seek, Write};
use std::io::{BufReader, SeekFrom};
use std::io::{Cursor, ErrorKind};
use std::sync::Arc;

#[cfg(feature = "deflate-flate2")]
use flate2::{write::DeflateEncoder, Compression};

#[cfg(feature = "_bzip2_any")]
use bzip2::write::BzEncoder;

#[cfg(feature = "deflate-zopfli")]
use zopfli::Options;

#[cfg(feature = "deflate-zopfli")]
use std::io::BufWriter;
use std::path::Path;
#[cfg(feature = "zstd")]
use zstd::stream::write::Encoder as ZstdEncoder;
#[cfg(feature = "_bzip2_any")]
use GenericZipWriter::Bzip2;
#[cfg(feature = "deflate-flate2")]
use GenericZipWriter::Deflater;
#[cfg(feature = "ppmd")]
use GenericZipWriter::Ppmd;
#[cfg(feature = "xz")]
use GenericZipWriter::Xz;
#[cfg(feature = "zstd")]
use GenericZipWriter::Zstd;
#[cfg(feature = "deflate-zopfli")]
use GenericZipWriter::{BufferedZopfliDeflater, ZopfliDeflater};

// re-export from types
pub use crate::types::{FileOptions, SimpleFileOptions};

#[cfg_attr(feature = "aes-crypto", allow(clippy::large_enum_variant))]
enum MaybeEncrypted<W> {
    Unencrypted(W),
    #[cfg(feature = "aes-crypto")]
    Aes(AesWriter<W>),
    ZipCrypto(crate::zipcrypto::ZipCryptoWriter<W>),
}

impl<W> Debug for MaybeEncrypted<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Don't print W, since it may be a huge Vec<u8>
        f.write_str(match self {
            MaybeEncrypted::Unencrypted(_) => "Unencrypted",
            #[cfg(feature = "aes-crypto")]
            MaybeEncrypted::Aes(_) => "AES",
            MaybeEncrypted::ZipCrypto(_) => "ZipCrypto",
        })
    }
}

impl<W: Write> Write for MaybeEncrypted<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            MaybeEncrypted::Unencrypted(w) => w.write(buf),
            #[cfg(feature = "aes-crypto")]
            MaybeEncrypted::Aes(w) => w.write(buf),
            MaybeEncrypted::ZipCrypto(w) => w.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            MaybeEncrypted::Unencrypted(w) => w.flush(),
            #[cfg(feature = "aes-crypto")]
            MaybeEncrypted::Aes(w) => w.flush(),
            MaybeEncrypted::ZipCrypto(w) => w.flush(),
        }
    }
}

enum GenericZipWriter<W: Write + Seek> {
    Closed,
    Storer(MaybeEncrypted<W>),
    #[cfg(feature = "deflate-flate2")]
    Deflater(DeflateEncoder<MaybeEncrypted<W>>),
    #[cfg(feature = "deflate-zopfli")]
    ZopfliDeflater(zopfli::DeflateEncoder<MaybeEncrypted<W>>),
    #[cfg(feature = "deflate-zopfli")]
    BufferedZopfliDeflater(BufWriter<zopfli::DeflateEncoder<MaybeEncrypted<W>>>),
    #[cfg(feature = "_bzip2_any")]
    Bzip2(BzEncoder<MaybeEncrypted<W>>),
    #[cfg(feature = "zstd")]
    Zstd(ZstdEncoder<'static, MaybeEncrypted<W>>),
    #[cfg(feature = "xz")]
    Xz(Box<lzma_rust2::XzWriter<MaybeEncrypted<W>>>),
    #[cfg(feature = "ppmd")]
    Ppmd(Box<ppmd_rust::Ppmd8Encoder<MaybeEncrypted<W>>>),
}

impl<W: Write + Seek> Debug for GenericZipWriter<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Closed => f.write_str("Closed"),
            Storer(w) => f.write_fmt(format_args!("Storer({w:?})")),
            #[cfg(feature = "deflate-flate2")]
            Deflater(w) => f.write_fmt(format_args!("Deflater({:?})", w.get_ref())),
            #[cfg(feature = "deflate-zopfli")]
            ZopfliDeflater(_) => f.write_str("ZopfliDeflater"),
            #[cfg(feature = "deflate-zopfli")]
            BufferedZopfliDeflater(_) => f.write_str("BufferedZopfliDeflater"),
            #[cfg(feature = "_bzip2_any")]
            Bzip2(w) => f.write_fmt(format_args!("Bzip2({:?})", w.get_ref())),
            #[cfg(feature = "zstd")]
            Zstd(w) => f.write_fmt(format_args!("Zstd({:?})", w.get_ref())),
            #[cfg(feature = "xz")]
            Xz(w) => f.write_fmt(format_args!("Xz({:?})", w.inner())),
            #[cfg(feature = "ppmd")]
            Ppmd(_) => f.write_fmt(format_args!("Ppmd8Encoder")),
        }
    }
}

// Put the struct declaration in a private module to convince rustdoc to display ZipWriter nicely
pub(crate) mod zip_writer {
    use core::fmt::{Debug, Formatter};
    use std::io::{Seek, Write};

    use indexmap::IndexMap;

    use crate::{
        types::ZipFileData,
        write::{GenericZipWriter, ZipWriterStats},
    };

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
    /// use zip::write::SimpleFileOptions;
    ///
    /// // We use a cursor + vec here, though you'd normally use a `File`
    /// let mut cur = std::io::Cursor::new(Vec::new());
    /// let mut zip = ZipWriter::new(&mut cur);
    ///
    /// let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    /// zip.start_file("hello_world.txt", options)?;
    /// zip.write(b"Hello, World!")?;
    ///
    /// // Apply the changes you've made.
    /// // Dropping the `ZipWriter` will have the same effect, but may silently fail
    /// zip.finish()?;
    ///
    /// // raw zip data is available as a Vec<u8>
    /// let zip_bytes = cur.into_inner();
    ///
    /// # Ok(())
    /// # }
    /// # doit().unwrap();
    /// ```
    pub struct ZipWriter<W: Write + Seek> {
        pub(super) inner: GenericZipWriter<W>,
        pub(super) files: IndexMap<Box<str>, ZipFileData>,
        pub(super) stats: ZipWriterStats,
        pub(super) writing_to_file: bool,
        pub(super) writing_raw: bool,
        pub(super) comment: Box<[u8]>,
        pub(super) zip64_comment: Option<Box<[u8]>>,
        pub(super) flush_on_finish_file: bool,
        pub(super) seek_possible: bool,
        pub(crate) auto_large_file: bool,
    }

    impl<W: Write + Seek> Debug for ZipWriter<W> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!(
                "ZipWriter {{files: {:?}, stats: {:?}, writing_to_file: {}, writing_raw: {}, comment: {:?}, flush_on_finish_file: {}}}",
                self.files, self.stats, self.writing_to_file, self.writing_raw,
                self.comment, self.flush_on_finish_file))
        }
    }
}
#[doc(inline)]
pub use self::sealed::FileOptionExtension;
use crate::result::ZipError::UnsupportedArchive;
use crate::unstable::path_to_string;
use crate::unstable::LittleEndianWriteExt;
use crate::write::GenericZipWriter::{Closed, Storer};
use crate::zipcrypto::{EncryptWith, ZipCryptoKeys};
use crate::CompressionMethod::Stored;
pub use zip_writer::ZipWriter;

#[derive(Default, Debug)]
struct ZipWriterStats {
    hasher: Hasher,
    start: u64,
    bytes_written: u64,
}

mod sealed {
    use std::sync::Arc;

    use super::ExtendedFileOptions;

    pub trait Sealed {}
    /// File options Extensions
    #[doc(hidden)]
    pub trait FileOptionExtension: Default + Sealed {
        /// Extra Data
        fn extra_data(&self) -> Option<&Arc<Vec<u8>>>;
        /// Central Extra Data
        fn central_extra_data(&self) -> Option<&Arc<Vec<u8>>>;
    }
    impl Sealed for () {}
    impl FileOptionExtension for () {
        fn extra_data(&self) -> Option<&Arc<Vec<u8>>> {
            None
        }
        fn central_extra_data(&self) -> Option<&Arc<Vec<u8>>> {
            None
        }
    }
    impl Sealed for ExtendedFileOptions {}

    impl FileOptionExtension for ExtendedFileOptions {
        fn extra_data(&self) -> Option<&Arc<Vec<u8>>> {
            Some(&self.extra_data)
        }
        fn central_extra_data(&self) -> Option<&Arc<Vec<u8>>> {
            Some(&self.central_extra_data)
        }
    }
}

/// Adds Extra Data and Central Extra Data. It does not implement copy.
pub type FullFileOptions<'k> = FileOptions<'k, ExtendedFileOptions>;
/// The Extension for Extra Data and Central Extra Data
#[cfg_attr(feature = "_arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Clone, Default, Eq, PartialEq)]
pub struct ExtendedFileOptions {
    extra_data: Arc<Vec<u8>>,
    central_extra_data: Arc<Vec<u8>>,
}

impl ExtendedFileOptions {
    /// Adds an extra data field, unless we detect that it's invalid.
    pub fn add_extra_data<D: AsRef<[u8]>>(
        &mut self,
        header_id: u16,
        data: D,
        central_only: bool,
    ) -> ZipResult<()> {
        let data = data.as_ref();
        let len = data.len() + 4;
        if self.extra_data.len() + self.central_extra_data.len() + len > u16::MAX as usize {
            Err(invalid!("Extra data field would be longer than allowed"))
        } else {
            let field = if central_only {
                &mut self.central_extra_data
            } else {
                &mut self.extra_data
            };
            let vec = Arc::get_mut(field);
            let vec = match vec {
                Some(exclusive) => exclusive,
                None => {
                    *field = Arc::new(field.to_vec());
                    Arc::get_mut(field).unwrap()
                }
            };
            Self::add_extra_data_unchecked(vec, header_id, data)?;
            Self::validate_extra_data(vec, true)?;
            Ok(())
        }
    }

    pub(crate) fn add_extra_data_unchecked(
        vec: &mut Vec<u8>,
        header_id: u16,
        data: &[u8],
    ) -> Result<(), ZipError> {
        vec.reserve_exact(data.len() + 4);
        vec.write_u16_le(header_id)?;
        vec.write_u16_le(data.len() as u16)?;
        vec.write_all(data)?;
        Ok(())
    }

    fn validate_extra_data(data: &[u8], disallow_zip64: bool) -> ZipResult<()> {
        let len = data.len() as u64;
        if len == 0 {
            return Ok(());
        }
        if len > u16::MAX as u64 {
            return Err(ZipError::Io(io::Error::other(
                "Extra-data field can't exceed u16::MAX bytes",
            )));
        }
        let mut data = Cursor::new(data);
        let mut pos = data.position();
        while pos < len {
            if len - data.position() < 4 {
                return Err(ZipError::Io(io::Error::other(
                    "Extra-data field doesn't have room for ID and length",
                )));
            }
            #[cfg(not(feature = "unreserved"))]
            {
                use crate::unstable::LittleEndianReadExt;
                let header_id = data.read_u16_le()?;
                if EXTRA_FIELD_MAPPING.contains(&header_id) {
                    return Err(ZipError::Io(io::Error::other(
                        format!(
                            "Extra data header ID {header_id:#06} requires crate feature \"unreserved\"",
                        ),
                    )));
                }
                data.seek(SeekFrom::Current(-2))?;
            }
            parse_single_extra_field(&mut ZipFileData::default(), &mut data, pos, disallow_zip64)?;
            pos = data.position();
        }
        Ok(())
    }
}

impl Debug for ExtendedFileOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_fmt(format_args!("ExtendedFileOptions {{extra_data: vec!{:?}.into(), central_extra_data: vec!{:?}.into()}}",
        self.extra_data, self.central_extra_data))
    }
}

#[cfg(feature = "_arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for FileOptions<'a, ExtendedFileOptions> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut options = FullFileOptions {
            compression_method: CompressionMethod::arbitrary(u)?,
            compression_level: if bool::arbitrary(u)? {
                Some(u.int_in_range(0..=24)?)
            } else {
                None
            },
            last_modified_time: DateTime::arbitrary(u)?,
            permissions: Option::<u32>::arbitrary(u)?,
            large_file: bool::arbitrary(u)?,
            encrypt_with: Option::<EncryptWith>::arbitrary(u)?,
            alignment: u16::arbitrary(u)?,
            #[cfg(feature = "deflate-zopfli")]
            zopfli_buffer_size: None,
            ..Default::default()
        };
        #[cfg(feature = "deflate-zopfli")]
        if options.compression_method == CompressionMethod::Deflated && bool::arbitrary(u)? {
            options.zopfli_buffer_size =
                Some(if bool::arbitrary(u)? { 2 } else { 3 } << u.int_in_range(8..=20)?);
        }
        u.arbitrary_loop(Some(0), Some(10), |u| {
            options
                .add_extra_data(
                    u.int_in_range(2..=u16::MAX)?,
                    Box::<[u8]>::arbitrary(u)?,
                    bool::arbitrary(u)?,
                )
                .map_err(|_| arbitrary::Error::IncorrectFormat)?;
            Ok(core::ops::ControlFlow::Continue(()))
        })?;
        ZipWriter::new(Cursor::new(Vec::new()))
            .start_file("", options.clone())
            .map_err(|_| arbitrary::Error::IncorrectFormat)?;
        Ok(options)
    }
}

impl<T: FileOptionExtension> FileOptions<'_, T> {
    pub(crate) fn normalize(&mut self) {
        if !self.last_modified_time.is_valid() {
            self.last_modified_time = FileOptions::<T>::default().last_modified_time;
        }

        *self.permissions.get_or_insert(0o644) |= ffi::S_IFREG;
    }

    /// Indicates whether this file will be encrypted (whether with AES or ZipCrypto).
    pub const fn has_encryption(&self) -> bool {
        #[cfg(feature = "aes-crypto")]
        {
            self.encrypt_with.is_some() || self.aes_mode.is_some()
        }
        #[cfg(not(feature = "aes-crypto"))]
        {
            self.encrypt_with.is_some()
        }
    }

    /// Set the compression method for the new file
    ///
    /// The default is [`CompressionMethod::Deflated`] if it is enabled. If not,
    /// [`CompressionMethod::Bzip2`] is the default if it is enabled. If neither `bzip2` nor `deflate`
    /// is enabled, [`CompressionMethod::Stored`] becomes the default and files are written uncompressed.
    #[must_use]
    pub const fn compression_method(mut self, method: CompressionMethod) -> Self {
        self.compression_method = method;
        self
    }

    /// Set the compression level for the new file
    ///
    /// `None` value specifies default compression level.
    ///
    /// Range of values depends on compression method:
    /// * `Deflated`: 10 - 264 for Zopfli, 0 - 9 for other encoders. Default is 24 if Zopfli is the
    ///   only encoder, or 6 otherwise.
    /// * `Bzip2`: 0 - 9. Default is 6
    /// * `Zstd`: -7 - 22, with zero being mapped to default level. Default is 3
    /// * others: only `None` is allowed
    #[must_use]
    pub const fn compression_level(mut self, level: Option<i64>) -> Self {
        self.compression_level = level;
        self
    }

    /// Set the last modified time
    ///
    /// The default is the current timestamp if the 'time' feature is enabled, and 1980-01-01
    /// otherwise
    #[must_use]
    pub const fn last_modified_time(mut self, mod_time: DateTime) -> Self {
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
    pub const fn unix_permissions(mut self, mode: u32) -> Self {
        self.permissions = Some(mode & 0o777);
        self
    }

    /// Set whether the new file's compressed and uncompressed size is less than 4 GiB.
    ///
    /// If set to `false` and the file exceeds the limit, an I/O error is thrown and the file is
    /// aborted. If set to `true`, readers will require ZIP64 support and if the file does not
    /// exceed the limit, 20 B are wasted. The default is `false`.
    #[must_use]
    pub const fn large_file(mut self, large: bool) -> Self {
        self.large_file = large;
        self
    }

    pub(crate) fn with_deprecated_encryption(self, password: &[u8]) -> FileOptions<'static, T> {
        FileOptions {
            encrypt_with: Some(EncryptWith::ZipCrypto(
                ZipCryptoKeys::derive(password),
                PhantomData,
            )),
            ..self
        }
    }

    /// Set the AES encryption parameters.
    #[cfg(feature = "aes-crypto")]
    pub fn with_aes_encryption(self, mode: AesMode, password: &str) -> FileOptions<'_, T> {
        FileOptions {
            encrypt_with: Some(EncryptWith::Aes { mode, password }),
            ..self
        }
    }

    /// Sets the size of the buffer used to hold the next block that Zopfli will compress. The
    /// larger the buffer, the more effective the compression, but the more memory is required.
    /// A value of `None` indicates no buffer, which is recommended only when all non-empty writes
    /// are larger than about 32 KiB.
    #[must_use]
    #[cfg(feature = "deflate-zopfli")]
    pub const fn with_zopfli_buffer(mut self, size: Option<usize>) -> Self {
        self.zopfli_buffer_size = size;
        self
    }

    /// Returns the compression level currently set.
    pub const fn get_compression_level(&self) -> Option<i64> {
        self.compression_level
    }
    /// Sets the alignment to the given number of bytes.
    #[must_use]
    pub const fn with_alignment(mut self, alignment: u16) -> Self {
        self.alignment = alignment;
        self
    }
}
impl FileOptions<'_, ExtendedFileOptions> {
    /// Adds an extra data field.
    pub fn add_extra_data<D: AsRef<[u8]>>(
        &mut self,
        header_id: u16,
        data: D,
        central_only: bool,
    ) -> ZipResult<()> {
        self.extended_options
            .add_extra_data(header_id, data, central_only)
    }

    /// Removes the extra data fields.
    #[must_use]
    pub fn clear_extra_data(mut self) -> Self {
        if !self.extended_options.extra_data.is_empty() {
            self.extended_options.extra_data = Arc::new(vec![]);
        }
        if !self.extended_options.central_extra_data.is_empty() {
            self.extended_options.central_extra_data = Arc::new(vec![]);
        }
        self
    }
}
impl FileOptions<'static, ()> {
    /// Constructs a const FileOptions object.
    ///
    /// Note: This value is different than the return value of [`FileOptions::default()`]:
    ///
    /// - The `last_modified_time` is [`DateTime::DEFAULT`]. This corresponds to 1980-01-01 00:00:00
    pub const DEFAULT: Self = Self {
        compression_method: CompressionMethod::DEFAULT,
        compression_level: None,
        last_modified_time: DateTime::DEFAULT,
        large_file: false,
        permissions: None,
        encrypt_with: None,
        extended_options: (),
        alignment: 1,
        #[cfg(feature = "deflate-zopfli")]
        zopfli_buffer_size: Some(1 << 15),
        #[cfg(feature = "aes-crypto")]
        aes_mode: None,
    };
}

impl<T: FileOptionExtension> Default for FileOptions<'_, T> {
    /// Construct a new FileOptions object
    fn default() -> Self {
        Self {
            compression_method: Default::default(),
            compression_level: None,
            last_modified_time: DateTime::default_for_write(),
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: T::default(),
            alignment: 1,
            #[cfg(feature = "deflate-zopfli")]
            zopfli_buffer_size: Some(1 << 15),
            #[cfg(feature = "aes-crypto")]
            aes_mode: None,
        }
    }
}

impl<W: Write + Seek> Write for ZipWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.writing_to_file {
            return Err(io::Error::other("No file has been started"));
        }
        if buf.is_empty() {
            return Ok(0);
        }
        match self.inner.ref_mut() {
            Some(ref mut w) => {
                let write_result = w.write(buf);
                if let Ok(count) = write_result {
                    self.stats.update(&buf[..count]);
                    if self.stats.bytes_written > spec::ZIP64_BYTES_THR
                        && !self.files.last_mut().unwrap().1.large_file
                    {
                        let _ = self.abort_file();
                        return Err(io::Error::other("Large file option has not been set"));
                    }
                }
                write_result
            }
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "write(): ZipWriter was already closed",
            )),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.ref_mut() {
            Some(ref mut w) => w.flush(),
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "flush(): ZipWriter was already closed",
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

impl<A: Read + Write + Seek> ZipWriter<A> {
    /// Initializes the archive from an existing ZIP archive, making it ready for append.
    ///
    /// This uses a default configuration to initially read the archive.
    pub fn new_append(readwriter: A) -> ZipResult<ZipWriter<A>> {
        Self::new_append_with_config(Default::default(), readwriter)
    }

    /// Initializes the archive from an existing ZIP archive, making it ready for append.
    ///
    /// This uses the given read configuration to initially read the archive.
    pub fn new_append_with_config(config: Config, mut readwriter: A) -> ZipResult<ZipWriter<A>> {
        readwriter.seek(SeekFrom::Start(0))?;
        let shared = ZipArchive::get_metadata(config, &mut readwriter)?;

        Ok(ZipWriter {
            inner: Storer(MaybeEncrypted::Unencrypted(readwriter)),
            files: shared.files,
            stats: Default::default(),
            writing_to_file: false,
            comment: shared.comment,
            zip64_comment: shared.zip64_comment,
            writing_raw: true, // avoid recomputing the last file's header
            flush_on_finish_file: false,
            seek_possible: true,
            auto_large_file: false,
        })
    }

    /// `flush_on_finish_file` is designed to support a streaming `inner` that may unload flushed
    /// bytes. It flushes a file's header and body once it starts writing another file. A ZipWriter
    /// will not try to seek back into where a previous file was written unless
    /// either [`ZipWriter::abort_file`] is called while [`ZipWriter::is_writing_file`] returns
    /// false, or [`ZipWriter::deep_copy_file`] is called. In the latter case, it will only need to
    /// read previously-written files and not overwrite them.
    ///
    /// Note: when using an `inner` that cannot overwrite flushed bytes, do not wrap it in a
    /// [BufWriter], because that has a [Seek::seek] method that implicitly calls
    /// [BufWriter::flush], and ZipWriter needs to seek backward to update each file's header with
    /// the size and checksum after writing the body.
    ///
    /// This setting is false by default.
    pub fn set_flush_on_finish_file(&mut self, flush_on_finish_file: bool) {
        self.flush_on_finish_file = flush_on_finish_file;
    }
}

impl<A: Read + Write + Seek> ZipWriter<A> {
    /// Adds another copy of a file already in this archive. This will produce a larger but more
    /// widely-compatible archive compared to [Self::shallow_copy_file]. Does not copy alignment.
    pub fn deep_copy_file(&mut self, src_name: &str, dest_name: &str) -> ZipResult<()> {
        self.finish_file()?;
        if src_name == dest_name || self.files.contains_key(dest_name) {
            return Err(invalid!("That file already exists"));
        }
        let write_position = self.inner.get_plain().stream_position()?;
        let src_index = self.index_by_name(src_name)?;
        let src_data = &mut self.files[src_index];
        let src_data_start = src_data.data_start(self.inner.get_plain())?;
        debug_assert!(src_data_start <= write_position);
        let mut compressed_size = src_data.compressed_size;
        if compressed_size > (write_position - src_data_start) {
            compressed_size = write_position - src_data_start;
            src_data.compressed_size = compressed_size;
        }
        let mut reader = BufReader::new(self.inner.get_plain());
        reader.seek(SeekFrom::Start(src_data_start))?;
        let mut copy = vec![0; compressed_size as usize];
        reader.take(compressed_size).read_exact(&mut copy)?;
        self.inner
            .get_plain()
            .seek(SeekFrom::Start(write_position))?;
        let mut new_data = src_data.clone();
        let dest_name_raw = dest_name.as_bytes();
        new_data.file_name = dest_name.into();
        new_data.file_name_raw = dest_name_raw.into();
        new_data.is_utf8 = !dest_name.is_ascii();
        new_data.header_start = write_position;
        let extra_data_start = write_position
            + size_of::<ZipLocalEntryBlock>() as u64
            + new_data.file_name_raw.len() as u64;
        new_data.extra_data_start = Some(extra_data_start);
        if let Some(extra) = &src_data.extra_field {
            let stripped = strip_alignment_extra_field(extra);
            if !stripped.is_empty() {
                new_data.extra_field = Some(stripped.into());
            } else {
                new_data.extra_field = None;
            }
        }

        let mut data_start = extra_data_start;
        if let Some(extra) = &new_data.extra_field {
            data_start += extra.len() as u64;
        }
        new_data.data_start.take();
        new_data.data_start.get_or_init(|| data_start);
        new_data.central_header_start = 0;
        let block = new_data.local_block()?;
        let index = self.insert_file_data(new_data)?;
        let new_data = &self.files[index];
        let result: io::Result<()> = {
            let plain_writer = self.inner.get_plain();
            block.write(plain_writer)?;
            plain_writer.write_all(&new_data.file_name_raw)?;
            if let Some(data) = &new_data.extra_field {
                plain_writer.write_all(data)?;
            }
            debug_assert_eq!(data_start, plain_writer.stream_position()?);
            self.writing_to_file = true;
            plain_writer.write_all(&copy)?;
            if self.flush_on_finish_file {
                plain_writer.flush()?;
            }
            Ok(())
        };
        self.ok_or_abort_file(result)?;
        self.writing_to_file = false;
        Ok(())
    }

    /// Like `deep_copy_file`, but uses Path arguments.
    ///
    /// This function ensures that the '/' path separator is used and normalizes `.` and `..`. It
    /// ignores any `..` or Windows drive letter that would produce a path outside the ZIP file's
    /// root.
    pub fn deep_copy_file_from_path<T: AsRef<Path>, U: AsRef<Path>>(
        &mut self,
        src_path: T,
        dest_path: U,
    ) -> ZipResult<()> {
        let src = path_to_string(src_path);
        let dest = path_to_string(dest_path);
        self.deep_copy_file(&src, &dest)
    }

    /// Write the zip file into the backing stream, then produce a readable archive of that data.
    ///
    /// This method avoids parsing the central directory records at the end of the stream for
    /// a slight performance improvement over running [`ZipArchive::new()`] on the output of
    /// [`Self::finish()`].
    ///
    ///```
    /// # fn main() -> Result<(), zip::result::ZipError> {
    /// # #[cfg(any(feature = "deflate-flate2", not(feature = "_deflate-any")))]
    /// # {
    /// use std::io::{Cursor, Read, Write};
    /// use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
    ///
    /// let buf = Cursor::new(Vec::new());
    /// let mut zip = ZipWriter::new(buf);
    /// let options = SimpleFileOptions::default();
    /// zip.start_file("a.txt", options)?;
    /// zip.write_all(b"hello\n")?;
    ///
    /// let mut zip = zip.finish_into_readable()?;
    /// let mut s: String = String::new();
    /// zip.by_name("a.txt")?.read_to_string(&mut s)?;
    /// assert_eq!(s, "hello\n");
    /// # }
    /// # Ok(())
    /// # }
    ///```
    pub fn finish_into_readable(mut self) -> ZipResult<ZipArchive<A>> {
        let central_start = self.finalize()?;
        let inner = mem::replace(&mut self.inner, Closed).unwrap();
        let comment = mem::take(&mut self.comment);
        let zip64_comment = mem::take(&mut self.zip64_comment);
        let files = mem::take(&mut self.files);

        let archive =
            ZipArchive::from_finalized_writer(files, comment, zip64_comment, inner, central_start)?;
        Ok(archive)
    }
}

impl<W: Write + Seek> ZipWriter<W> {
    /// Initializes the archive.
    ///
    /// Before writing to this object, the [`ZipWriter::start_file`] function should be called.
    /// After a successful write, the file remains open for writing. After a failed write, call
    /// [`ZipWriter::is_writing_file`] to determine if the file remains open.
    pub fn new(inner: W) -> ZipWriter<W> {
        ZipWriter {
            inner: Storer(MaybeEncrypted::Unencrypted(inner)),
            files: IndexMap::new(),
            stats: Default::default(),
            writing_to_file: false,
            writing_raw: false,
            comment: Box::new([]),
            zip64_comment: None,
            flush_on_finish_file: false,
            seek_possible: true,
            auto_large_file: false,
        }
    }

    /// Set automatically large file to true if needed
    pub fn set_auto_large_file(mut self) -> Self {
        self.auto_large_file = true;
        self
    }

    /// Returns true if a file is currently open for writing.
    pub const fn is_writing_file(&self) -> bool {
        self.writing_to_file && !self.inner.is_closed()
    }

    /// Set ZIP archive comment.
    pub fn set_comment<S>(&mut self, comment: S)
    where
        S: Into<Box<str>>,
    {
        self.set_raw_comment(comment.into().into_boxed_bytes())
    }

    /// Set ZIP archive comment.
    ///
    /// This sets the raw bytes of the comment. The comment
    /// is typically expected to be encoded in UTF-8.
    pub fn set_raw_comment(&mut self, comment: Box<[u8]>) {
        let max_comment_len = u16::MAX as usize; // 65,535
        if comment.len() > max_comment_len {
            self.set_raw_zip64_comment(Some(comment));
            self.comment = Box::new([]);
        } else {
            self.comment = comment;
            self.set_raw_zip64_comment(None);
        }
    }

    /// Get ZIP archive comment.
    pub fn get_comment(&mut self) -> Result<&str, Utf8Error> {
        from_utf8(self.get_raw_comment())
    }

    /// Get ZIP archive comment.
    ///
    /// This returns the raw bytes of the comment. The comment
    /// is typically expected to be encoded in UTF-8.
    pub const fn get_raw_comment(&self) -> &[u8] {
        &self.comment
    }

    /// Set ZIP64 archive comment.
    pub fn set_zip64_comment<S>(&mut self, comment: Option<S>)
    where
        S: Into<Box<str>>,
    {
        self.set_raw_zip64_comment(comment.map(|v| v.into().into_boxed_bytes()))
    }

    /// Set ZIP64 archive comment.
    ///
    /// This sets the raw bytes of the comment. The comment
    /// is typically expected to be encoded in UTF-8.
    pub fn set_raw_zip64_comment(&mut self, comment: Option<Box<[u8]>>) {
        self.zip64_comment = comment;
    }

    /// Get ZIP64 archive comment.
    pub fn get_zip64_comment(&mut self) -> Option<Result<&str, Utf8Error>> {
        self.get_raw_zip64_comment().map(from_utf8)
    }

    /// Get ZIP archive comment.
    ///
    /// This returns the raw bytes of the comment. The comment
    /// is typically expected to be encoded in UTF-8.
    pub fn get_raw_zip64_comment(&self) -> Option<&[u8]> {
        self.zip64_comment.as_deref()
    }

    /// Set the file length and crc32 manually.
    ///
    /// # Safety
    ///
    /// This overwrites the internal crc32 calculation. It should only be used in case
    /// the underlying [Write] is written independently and you need to adjust the zip metadata.
    pub unsafe fn set_file_metadata(&mut self, length: u64, crc32: u32) -> ZipResult<()> {
        if !self.writing_to_file {
            return Err(ZipError::Io(io::Error::other("No file has been started")));
        }
        self.stats.hasher = Hasher::new_with_initial_len(crc32, length);
        self.stats.bytes_written = length;
        Ok(())
    }

    fn ok_or_abort_file<T, E: Into<ZipError>>(&mut self, result: Result<T, E>) -> ZipResult<T> {
        match result {
            Err(e) => {
                let _ = self.abort_file();
                Err(e.into())
            }
            Ok(t) => Ok(t),
        }
    }

    /// Start a new file for with the requested options.
    fn start_entry<S: ToString, T: FileOptionExtension>(
        &mut self,
        name: S,
        mut options: FileOptions<T>,
        raw_values: Option<ZipRawValues>,
    ) -> ZipResult<()> {
        self.finish_file()?;

        let header_start = self.inner.get_plain().stream_position()?;
        let raw_values = raw_values.unwrap_or(ZipRawValues {
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
        });

        // Check if we're close to the 4GB boundary and force ZIP64 if needed
        // This ensures we properly handle appending to files close to 4GB
        if header_start > spec::ZIP64_BYTES_THR {
            // Files that start on or past the 4GiB boundary are always ZIP64
            options.large_file = true;
        }

        let mut extra_data = match options.extended_options.extra_data() {
            Some(data) => data.to_vec(),
            None => vec![],
        };
        let central_extra_data = options.extended_options.central_extra_data();
        if let Some(zip64_block) =
            Zip64ExtraFieldBlock::maybe_new(options.large_file, 0, 0, header_start)
        {
            let mut new_extra_data = zip64_block.serialize().into_vec();
            new_extra_data.append(&mut extra_data);
            extra_data = new_extra_data;
        }

        // Figure out the underlying compression_method and aes mode when using
        // AES encryption.
        let (compression_method, aes_mode) = match options.encrypt_with {
            // Preserve AES method for raw copies without needing a password
            #[cfg(feature = "aes-crypto")]
            None if options.aes_mode.is_some() => (CompressionMethod::Aes, options.aes_mode),
            #[cfg(feature = "aes-crypto")]
            Some(EncryptWith::Aes { mode, .. }) => (
                CompressionMethod::Aes,
                Some((mode, AesVendorVersion::Ae2, options.compression_method)),
            ),
            _ => (options.compression_method, None),
        };

        // Write AES encryption extra data.
        #[allow(unused_mut)]
        let mut aes_extra_data_start = 0;
        #[cfg(feature = "aes-crypto")]
        if let Some((mode, vendor, underlying)) = aes_mode {
            // For raw copies of AES entries, write the correct AES extra data immediately
            let mut body = [0; 7];
            [body[0], body[1]] = (vendor as u16).to_le_bytes(); // vendor version (1 or 2)
            [body[2], body[3]] = *b"AE"; // vendor id
            body[4] = mode as u8; // strength
            [body[5], body[6]] = underlying.serialize_to_u16().to_le_bytes(); // real compression method
            aes_extra_data_start = extra_data.len() as u64;
            ExtendedFileOptions::add_extra_data_unchecked(&mut extra_data, 0x9901, &body)?;
        }
        let header_end =
            header_start + size_of::<ZipLocalEntryBlock>() as u64 + name.to_string().len() as u64;

        if options.alignment > 1 {
            let extra_data_end = header_end + extra_data.len() as u64;
            let align = options.alignment as u64;
            let unaligned_header_bytes = extra_data_end % align;
            if unaligned_header_bytes != 0 {
                let mut pad_length = (align - unaligned_header_bytes) as usize;
                while pad_length < 6 {
                    pad_length += align as usize;
                }
                let new_len = extra_data.len() + pad_length;
                if new_len > u16::MAX as usize {
                    // Alignment is impossible without exceeding extra field size limits.
                    // Skip alignment.
                } else {
                    // Add an extra field to the extra_data, per APPNOTE 4.6.11
                    let mut pad_body = vec![0; pad_length - 4];
                    debug_assert!(pad_body.len() >= 2);
                    [pad_body[0], pad_body[1]] = options.alignment.to_le_bytes();
                    ExtendedFileOptions::add_extra_data_unchecked(
                        &mut extra_data,
                        0xa11e,
                        &pad_body,
                    )?;
                    debug_assert_eq!((extra_data.len() as u64 + header_end) % align, 0);
                }
            }
        }
        let extra_data_len = extra_data.len();
        if let Some(data) = central_extra_data {
            if extra_data_len + data.len() > u16::MAX as usize {
                return Err(invalid!(
                    "Extra data and central extra data must be less than 64KiB when combined"
                ));
            }
            ExtendedFileOptions::validate_extra_data(data, true)?;
        }
        let mut file = ZipFileData::initialize_local_block(
            name,
            &options,
            raw_values,
            header_start,
            None,
            aes_extra_data_start,
            compression_method,
            aes_mode,
            &extra_data,
        );
        file.using_data_descriptor =
            !self.seek_possible || matches!(options.encrypt_with, Some(EncryptWith::ZipCrypto(..)));
        file.version_made_by = file.version_made_by.max(file.version_needed() as u8);
        file.extra_data_start = Some(header_end);
        let index = self.insert_file_data(file)?;
        self.writing_to_file = true;
        let result: ZipResult<()> = {
            ExtendedFileOptions::validate_extra_data(&extra_data, false)?;
            let file = &mut self.files[index];
            let block = file.local_block()?;
            let writer = self.inner.get_plain();
            block.write(writer)?;
            // file name
            writer.write_all(&file.file_name_raw)?;
            if extra_data_len > 0 {
                writer.write_all(&extra_data)?;
                file.extra_field = Some(extra_data.into());
            }
            Ok(())
        };
        self.ok_or_abort_file(result)?;
        let writer = self.inner.get_plain();
        self.stats.start = writer.stream_position()?;
        match options.encrypt_with {
            #[cfg(feature = "aes-crypto")]
            Some(EncryptWith::Aes { mode, password }) => {
                let aeswriter = AesWriter::new(
                    mem::replace(&mut self.inner, Closed).unwrap(),
                    mode,
                    password.as_bytes(),
                )?;
                self.inner = Storer(MaybeEncrypted::Aes(aeswriter));
            }
            Some(EncryptWith::ZipCrypto(keys, ..)) => {
                let file = &mut self.files[index];
                // With ZipCrypto, we _need_ to use a data descriptor so that
                // we can initialize the stream properly.
                let mut zipwriter = crate::zipcrypto::ZipCryptoWriter {
                    writer: mem::replace(&mut self.inner, Closed).unwrap(),
                    keys,
                };
                self.stats.start = zipwriter.writer.stream_position()?;
                // crypto_header is counted as part of the data
                let mut crypto_header = [0u8; 12];
                // The last two bytes of the header should either be the CRC of
                // the file _OR_ the last timepart of the last modified time if
                // a data descriptor is used.
                //
                // However, this header is encrypted, and ZipCrypto is not a
                // seekable encryption algorithm - an earlier plaintext byte
                // will have an impact on the later values.
                //
                // This makes it impossible to write the file without first
                // calculating its CRC. To avoid having to read the file twice
                // or keeping the entire file in memory, we force all ZipCrypto
                // files to use a data descriptor. This way, we can use the
                // local_modified_time as a password check byte instead of the
                // CRC.
                crypto_header[10..=11].copy_from_slice(
                    &file
                        .last_modified_time
                        .unwrap_or_else(DateTime::default_for_write)
                        .timepart()
                        .to_le_bytes(),
                );
                let result = zipwriter.write_all(&crypto_header);
                self.ok_or_abort_file(result)?;
                self.inner = Storer(MaybeEncrypted::ZipCrypto(zipwriter));
            }
            None => {}
        }
        let file = &mut self.files[index];
        debug_assert!(file.data_start.get().is_none());
        file.data_start.get_or_init(|| self.stats.start);
        self.stats.bytes_written = 0;
        self.stats.hasher = Hasher::new();
        Ok(())
    }

    fn insert_file_data(&mut self, file: ZipFileData) -> ZipResult<usize> {
        if self.files.contains_key(&file.file_name) {
            return Err(invalid!("Duplicate filename: {}", file.file_name));
        }
        let (index, _) = self.files.insert_full(file.file_name.clone(), file);
        Ok(index)
    }

    fn finish_file(&mut self) -> ZipResult<()> {
        if !self.writing_to_file {
            return Ok(());
        }

        let make_plain_writer = self.inner.prepare_next_writer(
            Stored,
            None,
            #[cfg(feature = "deflate-zopfli")]
            None,
        )?;
        self.inner.switch_to(make_plain_writer)?;
        self.switch_to_non_encrypting_writer()?;
        let writer = self.inner.get_plain();

        if !self.writing_raw {
            let file = match self.files.last_mut() {
                None => return Ok(()),
                Some((_, f)) => f,
            };
            file.uncompressed_size = self.stats.bytes_written;

            let file_end = writer.stream_position()?;
            debug_assert!(file_end >= self.stats.start);
            file.compressed_size = file_end - self.stats.start;

            if !file.using_data_descriptor {
                // Not using a data descriptor means the underlying writer
                // supports seeking, so we can go back and update the AES Extra
                // Data header to use AE1 for large files.
                update_aes_extra_data(writer, file, self.stats.bytes_written)?;
            }

            let crc = !matches!(file.aes_mode, Some((_, AesVendorVersion::Ae2, _)));

            file.crc32 = if crc {
                self.stats.hasher.clone().finalize()
            } else {
                0
            };

            if file.using_data_descriptor {
                file.write_data_descriptor(writer, self.auto_large_file)?;
            } else {
                update_local_file_header(writer, file)?;
                writer.seek(SeekFrom::Start(file_end))?;
            }
        }
        if self.flush_on_finish_file {
            let result = writer.flush();
            self.ok_or_abort_file(result)?;
        }

        self.writing_to_file = false;
        Ok(())
    }

    fn switch_to_non_encrypting_writer(&mut self) -> Result<(), ZipError> {
        match mem::replace(&mut self.inner, Closed) {
            #[cfg(feature = "aes-crypto")]
            Storer(MaybeEncrypted::Aes(writer)) => {
                self.inner = Storer(MaybeEncrypted::Unencrypted(writer.finish()?));
            }
            Storer(MaybeEncrypted::ZipCrypto(writer)) => {
                self.inner = Storer(MaybeEncrypted::Unencrypted(writer.finish()?))
            }
            Storer(MaybeEncrypted::Unencrypted(w)) => {
                self.inner = Storer(MaybeEncrypted::Unencrypted(w))
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// Removes the file currently being written from the archive if there is one, or else removes
    /// the file most recently written.
    pub fn abort_file(&mut self) -> ZipResult<()> {
        let (_, last_file) = self.files.pop().ok_or(ZipError::FileNotFound)?;
        let make_plain_writer = self.inner.prepare_next_writer(
            Stored,
            None,
            #[cfg(feature = "deflate-zopfli")]
            None,
        )?;
        self.inner.switch_to(make_plain_writer)?;
        self.switch_to_non_encrypting_writer()?;
        // Make sure this is the last file, and that no shallow copies of it remain; otherwise we'd
        // overwrite a valid file and corrupt the archive
        let rewind_safe: bool = match last_file.data_start.get() {
            None => self.files.is_empty(),
            Some(last_file_start) => self.files.values().all(|file| {
                file.data_start
                    .get()
                    .is_some_and(|start| start < last_file_start)
            }),
        };
        if rewind_safe {
            self.inner
                .get_plain()
                .seek(SeekFrom::Start(last_file.header_start))?;
        }
        self.writing_to_file = false;
        Ok(())
    }

    /// Create a file in the archive and start writing its' contents. The file must not have the
    /// same name as a file already in the archive.
    ///
    /// The data should be written using the [`Write`] implementation on this [`ZipWriter`]
    pub fn start_file<S: ToString, T: FileOptionExtension>(
        &mut self,
        name: S,
        mut options: FileOptions<T>,
    ) -> ZipResult<()> {
        options.normalize();
        let make_new_self = self.inner.prepare_next_writer(
            options.compression_method,
            options.compression_level,
            #[cfg(feature = "deflate-zopfli")]
            options.zopfli_buffer_size,
        )?;
        self.start_entry(name, options, None)?;
        let result = self.inner.switch_to(make_new_self);
        self.ok_or_abort_file(result)?;
        self.writing_raw = false;
        Ok(())
    }

    /* TODO: link to/use Self::finish_into_readable() from https://github.com/zip-rs/zip/pull/400 in
     * this docstring. */
    /// Copy over the entire contents of another archive verbatim.
    ///
    /// This method extracts file metadata from the `source` archive, then simply performs a single
    /// big [`io::copy()`](io::copy) to transfer all the actual file contents without any
    /// decompression or decryption. This is more performant than the equivalent operation of
    /// calling [`Self::raw_copy_file()`] for each entry from the `source` archive in sequence.
    ///
    ///```
    /// # fn main() -> Result<(), zip::result::ZipError> {
    /// # #[cfg(any(feature = "deflate-flate2", not(feature = "_deflate-any")))]
    /// # {
    /// use std::io::{Cursor, Write, Read};
    /// use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
    ///
    /// let buf = Cursor::new(Vec::new());
    /// let mut zip = ZipWriter::new(buf);
    /// zip.start_file("a.txt", SimpleFileOptions::default())?;
    /// zip.write_all(b"hello\n")?;
    /// let src = ZipArchive::new(zip.finish()?)?;
    ///
    /// let buf = Cursor::new(Vec::new());
    /// let mut zip = ZipWriter::new(buf);
    /// zip.start_file("b.txt", SimpleFileOptions::default())?;
    /// zip.write_all(b"hey\n")?;
    /// let src2 = ZipArchive::new(zip.finish()?)?;
    ///
    /// let buf = Cursor::new(Vec::new());
    ///
    /// let mut zip = ZipWriter::new(buf);
    /// zip.merge_archive(src)?;
    /// zip.merge_archive(src2)?;
    /// let mut result = ZipArchive::new(zip.finish()?)?;
    ///
    /// let mut s: String = String::new();
    /// result.by_name("a.txt")?.read_to_string(&mut s)?;
    /// assert_eq!(s, "hello\n");
    /// s.clear();
    /// result.by_name("b.txt")?.read_to_string(&mut s)?;
    /// assert_eq!(s, "hey\n");
    /// # }
    /// # Ok(())
    /// # }
    ///```
    pub fn merge_archive<R>(&mut self, mut source: ZipArchive<R>) -> ZipResult<()>
    where
        R: Read + Seek,
    {
        self.finish_file()?;

        /* Ensure we accept the file contents on faith (and avoid overwriting the data).
         * See raw_copy_file_rename(). */
        self.writing_to_file = true;
        self.writing_raw = true;

        let writer = self.inner.get_plain();
        /* Get the file entries from the source archive. */
        let new_files = source.merge_contents(writer)?;

        /* These file entries are now ours! */
        self.files.extend(new_files);

        Ok(())
    }

    /// Starts a file, taking a Path as argument.
    ///
    /// This function ensures that the '/' path separator is used and normalizes `.` and `..`. It
    /// ignores any `..` or Windows drive letter that would produce a path outside the ZIP file's
    /// root.
    pub fn start_file_from_path<E: FileOptionExtension, P: AsRef<Path>>(
        &mut self,
        path: P,
        options: FileOptions<E>,
    ) -> ZipResult<()> {
        self.start_file(path_to_string(path), options)
    }

    /// Add a new file using the already compressed data from a ZIP file being read and renames it, this
    /// allows faster copies of the `ZipFile` since there is no need to decompress and compress it again.
    /// Any `ZipFile` metadata is copied and not checked, for example the file CRC.
    ///
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
    pub fn raw_copy_file_rename<R: Read, S: ToString>(
        &mut self,
        file: ZipFile<R>,
        name: S,
    ) -> ZipResult<()> {
        let options = file.options();
        self.raw_copy_file_rename_internal(file, name, options)
    }

    fn raw_copy_file_rename_internal<R: Read, S: ToString>(
        &mut self,
        mut file: ZipFile<R>,
        name: S,
        options: SimpleFileOptions,
    ) -> ZipResult<()> {
        let raw_values = ZipRawValues {
            crc32: file.crc32(),
            compressed_size: file.compressed_size(),
            uncompressed_size: file.size(),
        };

        self.start_entry(name, options, Some(raw_values))?;
        self.writing_to_file = true;
        self.writing_raw = true;

        io::copy(&mut file.take_raw_reader()?, self)?;
        self.finish_file()
    }

    /// Like `raw_copy_file_to_path`, but uses Path arguments.
    ///
    /// This function ensures that the '/' path separator is used and normalizes `.` and `..`. It
    /// ignores any `..` or Windows drive letter that would produce a path outside the ZIP file's
    /// root.
    pub fn raw_copy_file_to_path<R: Read, P: AsRef<Path>>(
        &mut self,
        file: ZipFile<R>,
        path: P,
    ) -> ZipResult<()> {
        self.raw_copy_file_rename(file, path_to_string(path))
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
    pub fn raw_copy_file<R: Read>(&mut self, file: ZipFile<R>) -> ZipResult<()> {
        let name = file.name().to_owned();
        self.raw_copy_file_rename(file, name)
    }

    /// Add a new file using the already compressed data from a ZIP file being read and set the last
    /// modified date and unix mode. This allows faster copies of the `ZipFile` since there is no need
    /// to decompress and compress it again. Any `ZipFile` metadata other than the last modified date
    /// and the unix mode is copied and not checked, for example the file CRC.
    ///
    /// ```no_run
    /// use std::io::{Read, Seek, Write};
    /// use zip::{DateTime, ZipArchive, ZipWriter};
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
    ///     dst.raw_copy_file_touch(file, DateTime::default(), Some(0o644))?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn raw_copy_file_touch<R: Read>(
        &mut self,
        file: ZipFile<R>,
        last_modified_time: DateTime,
        unix_mode: Option<u32>,
    ) -> ZipResult<()> {
        let name = file.name().to_owned();

        let mut options = file.options();

        options = options.last_modified_time(last_modified_time);

        if let Some(perms) = unix_mode {
            options = options.unix_permissions(perms);
        }

        options.normalize();

        self.raw_copy_file_rename_internal(file, name, options)
    }

    /// Add a directory entry.
    ///
    /// As directories have no content, you must not call [`ZipWriter::write`] before adding a new file.
    pub fn add_directory<S, T: FileOptionExtension>(
        &mut self,
        name: S,
        mut options: FileOptions<T>,
    ) -> ZipResult<()>
    where
        S: Into<String>,
    {
        if options.permissions.is_none() {
            options.permissions = Some(0o755);
        }
        *options.permissions.as_mut().unwrap() |= 0o40000;
        options.compression_method = Stored;
        options.encrypt_with = None;

        let name_as_string = name.into();
        // Append a slash to the filename if it does not end with it.
        let name_with_slash = match name_as_string.chars().last() {
            Some('/') | Some('\\') => name_as_string,
            _ => name_as_string + "/",
        };

        self.start_entry(name_with_slash, options, None)?;
        self.writing_to_file = false;
        self.switch_to_non_encrypting_writer()?;
        Ok(())
    }

    /// Add a directory entry, taking a Path as argument.
    ///
    /// This function ensures that the '/' path separator is used and normalizes `.` and `..`. It
    /// ignores any `..` or Windows drive letter that would produce a path outside the ZIP file's
    /// root.
    pub fn add_directory_from_path<T: FileOptionExtension, P: AsRef<Path>>(
        &mut self,
        path: P,
        options: FileOptions<T>,
    ) -> ZipResult<()> {
        self.add_directory(path_to_string(path), options)
    }

    /// Finish the last file and write all other zip-structures
    ///
    /// This will return the writer, but one should normally not append any data to the end of the file.
    /// Note that the zipfile will also be finished on drop.
    pub fn finish(mut self) -> ZipResult<W> {
        let _central_start = self.finalize()?;
        let inner = mem::replace(&mut self.inner, Closed);
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
    pub fn add_symlink<N: ToString, T: ToString, E: FileOptionExtension>(
        &mut self,
        name: N,
        target: T,
        mut options: FileOptions<E>,
    ) -> ZipResult<()> {
        if options.permissions.is_none() {
            options.permissions = Some(0o777);
        }
        *options.permissions.as_mut().unwrap() |= S_IFLNK;
        // The symlink target is stored as file content. And compressing the target path
        // likely wastes space. So always store.
        options.compression_method = Stored;

        self.start_entry(name, options, None)?;
        self.writing_to_file = true;
        let result = self.write_all(target.to_string().as_bytes());
        self.ok_or_abort_file(result)?;
        self.writing_raw = false;
        self.finish_file()?;

        Ok(())
    }

    /// Add a symlink entry, taking Paths to the location and target as arguments.
    ///
    /// This function ensures that the '/' path separator is used and normalizes `.` and `..`. It
    /// ignores any `..` or Windows drive letter that would produce a path outside the ZIP file's
    /// root.
    pub fn add_symlink_from_path<P: AsRef<Path>, T: AsRef<Path>, E: FileOptionExtension>(
        &mut self,
        path: P,
        target: T,
        options: FileOptions<E>,
    ) -> ZipResult<()> {
        self.add_symlink(path_to_string(path), path_to_string(target), options)
    }

    fn finalize(&mut self) -> ZipResult<u64> {
        self.finish_file()?;

        let mut central_start = self.write_central_and_footer()?;
        let writer = self.inner.get_plain();
        let footer_end = writer.stream_position()?;
        let archive_end = writer.seek(SeekFrom::End(0))?;
        if footer_end < archive_end {
            // Data from an aborted file is past the end of the footer.

            // Overwrite the magic so the footer is no longer valid.
            writer.seek(SeekFrom::Start(central_start))?;
            writer.write_u32_le(0)?;
            writer.seek(SeekFrom::Start(
                footer_end - size_of::<Zip32CDEBlock>() as u64 - self.comment.len() as u64,
            ))?;
            writer.write_u32_le(0)?;

            // Rewrite the footer at the actual end.
            let central_and_footer_size = footer_end - central_start;
            writer.seek(SeekFrom::End(-(central_and_footer_size as i64)))?;
            central_start = self.write_central_and_footer()?;
            debug_assert!(self.inner.get_plain().stream_position()? == archive_end);
        }

        Ok(central_start)
    }

    fn write_central_and_footer(&mut self) -> Result<u64, ZipError> {
        let writer = self.inner.get_plain();

        let mut version_needed = MIN_VERSION as u16;
        let central_start = writer.stream_position()?;
        for file in self.files.values() {
            write_central_directory_header(writer, file)?;
            version_needed = version_needed.max(file.version_needed());
        }
        let central_size = writer.stream_position()? - central_start;
        let is64 = self.files.len() > spec::ZIP64_ENTRY_THR
            || central_size.max(central_start) > spec::ZIP64_BYTES_THR
            || self.zip64_comment.is_some();

        if is64 {
            let comment = self.zip64_comment.clone().unwrap_or_default();

            let zip64_footer = spec::Zip64CentralDirectoryEnd {
                record_size: comment.len() as u64 + 44,
                version_made_by: version_needed,
                version_needed_to_extract: version_needed,
                disk_number: 0,
                disk_with_central_directory: 0,
                number_of_files_on_this_disk: self.files.len() as u64,
                number_of_files: self.files.len() as u64,
                central_directory_size: central_size,
                central_directory_offset: central_start,
                extensible_data_sector: comment,
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
        let footer = spec::Zip32CentralDirectoryEnd {
            disk_number: 0,
            disk_with_central_directory: 0,
            zip_file_comment: self.comment.clone(),
            number_of_files_on_this_disk: number_of_files,
            number_of_files,
            central_directory_size: central_size.min(spec::ZIP64_BYTES_THR) as u32,
            central_directory_offset: central_start.min(spec::ZIP64_BYTES_THR) as u32,
        };

        footer.write(writer)?;
        Ok(central_start)
    }

    fn index_by_name(&self, name: &str) -> ZipResult<usize> {
        self.files.get_index_of(name).ok_or(ZipError::FileNotFound)
    }

    /// Adds another entry to the central directory referring to the same content as an existing
    /// entry. The file's local-file header will still refer to it by its original name, so
    /// unzipping the file will technically be unspecified behavior. [ZipArchive] ignores the
    /// filename in the local-file header and treat the central directory as authoritative. However,
    /// some other software (e.g. Minecraft) will refuse to extract a file copied this way.
    pub fn shallow_copy_file(&mut self, src_name: &str, dest_name: &str) -> ZipResult<()> {
        self.finish_file()?;
        if src_name == dest_name {
            return Err(invalid!("Trying to copy a file to itself"));
        }
        let src_index = self.index_by_name(src_name)?;
        let mut dest_data = self.files[src_index].to_owned();
        dest_data.file_name = dest_name.to_string().into();
        dest_data.file_name_raw = dest_name.to_string().into_bytes().into();
        dest_data.central_header_start = 0;
        self.insert_file_data(dest_data)?;

        Ok(())
    }

    /// Like `shallow_copy_file`, but uses Path arguments.
    ///
    /// This function ensures that the '/' path separator is used and normalizes `.` and `..`. It
    /// ignores any `..` or Windows drive letter that would produce a path outside the ZIP file's
    /// root.
    pub fn shallow_copy_file_from_path<T: AsRef<Path>, U: AsRef<Path>>(
        &mut self,
        src_path: T,
        dest_path: U,
    ) -> ZipResult<()> {
        self.shallow_copy_file(&path_to_string(src_path), &path_to_string(dest_path))
    }
}

impl<W: Write> ZipWriter<StreamWriter<W>> {
    /// Creates a writer that doesn't require the inner writer to implement [Seek], but where
    /// operations that would overwrite previously-written bytes or cause subsequent operations to
    /// do so (such as `abort_file`) will always return an error.
    pub fn new_stream(inner: W) -> ZipWriter<StreamWriter<W>> {
        ZipWriter {
            inner: Storer(MaybeEncrypted::Unencrypted(StreamWriter::new(inner))),
            files: IndexMap::new(),
            stats: Default::default(),
            writing_to_file: false,
            writing_raw: false,
            comment: Box::new([]),
            zip64_comment: None,
            flush_on_finish_file: false,
            seek_possible: false,
            auto_large_file: false,
        }
    }
}

impl<W: Write + Seek> Drop for ZipWriter<W> {
    fn drop(&mut self) {
        if !self.inner.is_closed() {
            if let Err(e) = self.finalize() {
                let _ = write!(
                    io::stderr(),
                    "ZipWriter::drop: failed to finalize archive: {e:?}"
                );
            }
        }
    }
}

type SwitchWriterFunction<W> = Box<dyn FnOnce(MaybeEncrypted<W>) -> ZipResult<GenericZipWriter<W>>>;

impl<W: Write + Seek> GenericZipWriter<W> {
    fn prepare_next_writer(
        &self,
        compression: CompressionMethod,
        compression_level: Option<i64>,
        #[cfg(feature = "deflate-zopfli")] zopfli_buffer_size: Option<usize>,
    ) -> ZipResult<SwitchWriterFunction<W>> {
        if let Closed = self {
            return Err(
                io::Error::new(io::ErrorKind::BrokenPipe, "ZipWriter was already closed").into(),
            );
        }

        match compression {
            Stored => {
                if compression_level.is_some() {
                    Err(UnsupportedArchive("Unsupported compression level"))
                } else {
                    Ok(Box::new(|bare| Ok(Storer(bare))))
                }
            }
            #[allow(unreachable_code)]
            #[cfg(feature = "_deflate-any")]
            CompressionMethod::Deflated => {
                let default = crate::cfg_if_expr! {
                    i64:
                    #[cfg(feature = "deflate-flate2")] => Compression::default().level() as i64,
                    #[cfg(feature = "deflate-zopfli")] => 24,
                    _ => compile_error!("could not calculate default: unknown deflate variant"),
                };

                let level = validate_value_in_range(
                    compression_level.unwrap_or(default),
                    deflate_compression_level_range(),
                )
                .ok_or(UnsupportedArchive("Unsupported compression level"))?
                    as u32;

                #[cfg(feature = "deflate-zopfli")]
                {
                    macro_rules! deflate_zopfli_and_return {
                        ($bare:expr, $best_non_zopfli:expr) => {{
                            let options = Options {
                                iteration_count: NonZeroU64::try_from(
                                    (level - $best_non_zopfli) as u64,
                                )
                                .unwrap(),
                                ..Default::default()
                            };
                            Ok(Box::new(move |bare| {
                                Ok(match zopfli_buffer_size {
                                    Some(size) => BufferedZopfliDeflater(BufWriter::with_capacity(
                                        size,
                                        zopfli::DeflateEncoder::new(
                                            options,
                                            Default::default(),
                                            bare,
                                        ),
                                    )),
                                    None => ZopfliDeflater(zopfli::DeflateEncoder::new(
                                        options,
                                        Default::default(),
                                        bare,
                                    )),
                                })
                            }))
                        }};
                    }

                    crate::cfg_if! {
                        if #[cfg(feature = "deflate-flate2")] {
                            let best_non_zopfli = Compression::best().level();
                            if level > best_non_zopfli {
                                return deflate_zopfli_and_return!(bare, best_non_zopfli);
                            }
                        } else {
                            let best_non_zopfli = 9;
                            return deflate_zopfli_and_return!(bare, best_non_zopfli);
                        }
                    }
                }
                crate::cfg_if_expr! {
                    ZipResult<SwitchWriterFunction<W>>:
                    #[cfg(feature = "deflate-flate2")] => Ok(Box::new(move |bare| {
                        Ok(GenericZipWriter::Deflater(DeflateEncoder::new(
                            bare,
                            Compression::new(level),
                        )))
                    })),
                    _ => unreachable!("deflate writer: have no fallback for this case")
                }
            }
            #[cfg(feature = "deflate64")]
            CompressionMethod::Deflate64 => {
                Err(UnsupportedArchive("Compressing Deflate64 is not supported"))
            }
            #[cfg(feature = "_bzip2_any")]
            CompressionMethod::Bzip2 => {
                let level = validate_value_in_range(
                    compression_level.unwrap_or(bzip2::Compression::default().level() as i64),
                    bzip2_compression_level_range(),
                )
                .ok_or(UnsupportedArchive("Unsupported compression level"))?
                    as u32;
                Ok(Box::new(move |bare| {
                    Ok(Bzip2(BzEncoder::new(bare, bzip2::Compression::new(level))))
                }))
            }
            CompressionMethod::AES => Err(UnsupportedArchive(
                "AES encryption is enabled through FileOptions::with_aes_encryption",
            )),
            #[cfg(feature = "zstd")]
            CompressionMethod::Zstd => {
                let level = validate_value_in_range(
                    compression_level.unwrap_or(zstd::DEFAULT_COMPRESSION_LEVEL as i64),
                    zstd::compression_level_range(),
                )
                .ok_or(UnsupportedArchive("Unsupported compression level"))?;
                Ok(Box::new(move |bare| {
                    Ok(Zstd(
                        ZstdEncoder::new(bare, level as i32).map_err(ZipError::Io)?,
                    ))
                }))
            }
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Shrink => Err(ZipError::UnsupportedArchive(
                "Shrink compression unsupported",
            )),
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Reduce(_) => Err(ZipError::UnsupportedArchive(
                "Reduce compression unsupported",
            )),
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Implode => Err(ZipError::UnsupportedArchive(
                "Implode compression unsupported",
            )),
            #[cfg(feature = "lzma")]
            CompressionMethod::Lzma => {
                Err(UnsupportedArchive("LZMA isn't supported for compression"))
            }
            #[cfg(feature = "xz")]
            CompressionMethod::Xz => {
                let level = validate_value_in_range(compression_level.unwrap_or(6), 0..=9)
                    .ok_or(UnsupportedArchive("Unsupported compression level"))?
                    as u32;
                Ok(Box::new(move |bare| {
                    Ok(Xz(Box::new(
                        lzma_rust2::XzWriter::new(bare, lzma_rust2::XzOptions::with_preset(level))
                            .map_err(ZipError::Io)?,
                    )))
                }))
            }
            #[cfg(feature = "ppmd")]
            CompressionMethod::Ppmd => {
                const ORDERS: [u32; 10] = [0, 4, 5, 6, 7, 8, 9, 10, 11, 12];

                let level = validate_value_in_range(compression_level.unwrap_or(7), 1..=9)
                    .ok_or(UnsupportedArchive("Unsupported compression level"))?
                    as u32;

                let order = ORDERS[level as usize];
                let memory_size = 1 << (level + 19);
                let memory_size_mb = memory_size / 1024 / 1024;

                Ok(Box::new(move |mut bare| {
                    let parameter: u16 = (order as u16 - 1)
                        + ((memory_size_mb - 1) << 4) as u16
                        + ((ppmd_rust::RestoreMethod::Restart as u16) << 12);

                    bare.write_all(&parameter.to_le_bytes())
                        .map_err(ZipError::Io)?;

                    let encoder = ppmd_rust::Ppmd8Encoder::new(
                        bare,
                        order,
                        memory_size,
                        ppmd_rust::RestoreMethod::Restart,
                    )
                    .map_err(|error| match error {
                        ppmd_rust::Error::RangeDecoderInitialization => ZipError::InvalidArchive(
                            "PPMd range coder initialization failed".into(),
                        ),
                        ppmd_rust::Error::InvalidParameter => {
                            ZipError::InvalidArchive("Invalid PPMd parameter".into())
                        }
                        ppmd_rust::Error::IoError(io_error) => ZipError::Io(io_error),
                        ppmd_rust::Error::MemoryAllocation => ZipError::Io(io::Error::new(
                            ErrorKind::OutOfMemory,
                            "PPMd could not allocate memory",
                        )),
                    })?;

                    Ok(Ppmd(Box::new(encoder)))
                }))
            }
            #[allow(deprecated)]
            CompressionMethod::Unsupported(..) => {
                Err(UnsupportedArchive("Unsupported compression"))
            }
        }
    }

    fn switch_to(&mut self, make_new_self: SwitchWriterFunction<W>) -> ZipResult<()> {
        let bare = match mem::replace(self, Closed) {
            Storer(w) => w,
            #[cfg(feature = "deflate-flate2")]
            Deflater(w) => w.finish()?,
            #[cfg(feature = "deflate-zopfli")]
            ZopfliDeflater(w) => w.finish()?,
            #[cfg(feature = "deflate-zopfli")]
            BufferedZopfliDeflater(w) => w
                .into_inner()
                .map_err(|e| ZipError::Io(e.into_error()))?
                .finish()?,
            #[cfg(feature = "_bzip2_any")]
            Bzip2(w) => w.finish()?,
            #[cfg(feature = "zstd")]
            Zstd(w) => w.finish()?,
            #[cfg(feature = "xz")]
            Xz(w) => w.finish()?,
            #[cfg(feature = "ppmd")]
            Ppmd(w) => {
                // ZIP needs to encode an end marker (7z for example doesn't encode one).
                w.finish(true)?
            }
            Closed => {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ZipWriter was already closed",
                )
                .into());
            }
        };
        *self = make_new_self(bare)?;
        Ok(())
    }

    fn ref_mut(&mut self) -> Option<&mut dyn Write> {
        match self {
            Storer(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(feature = "deflate-flate2")]
            Deflater(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(feature = "deflate-zopfli")]
            ZopfliDeflater(w) => Some(w as &mut dyn Write),
            #[cfg(feature = "deflate-zopfli")]
            BufferedZopfliDeflater(w) => Some(w as &mut dyn Write),
            #[cfg(feature = "_bzip2_any")]
            Bzip2(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(feature = "zstd")]
            Zstd(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(feature = "xz")]
            Xz(ref mut w) => Some(w as &mut dyn Write),
            #[cfg(feature = "ppmd")]
            Ppmd(ref mut w) => Some(w as &mut dyn Write),
            Closed => None,
        }
    }

    const fn is_closed(&self) -> bool {
        matches!(*self, Closed)
    }

    fn get_plain(&mut self) -> &mut W {
        match *self {
            Storer(MaybeEncrypted::Unencrypted(ref mut w)) => w,
            _ => panic!("Should have switched to stored and unencrypted beforehand"),
        }
    }

    fn unwrap(self) -> W {
        match self {
            Storer(MaybeEncrypted::Unencrypted(w)) => w,
            _ => panic!("Should have switched to stored and unencrypted beforehand"),
        }
    }
}

#[cfg(feature = "_deflate-any")]
fn deflate_compression_level_range() -> std::ops::RangeInclusive<i64> {
    let min = crate::cfg_if_expr! {
        i64:
        #[cfg(feature = "deflate-flate2")] => Compression::fast().level() as i64,
        #[cfg(feature = "deflate-zopfli")] => 1,
        _ => compile_error!("min: unknown deflate variant"),
    };

    let max = crate::cfg_if_expr! {
        i64:
        #[cfg(feature = "deflate-zopfli")] => 264,
        #[cfg(feature = "deflate-flate2")] => Compression::best().level() as i64,
        _ => compile_error!("max: unknown deflate variant"),
    };

    min..=max
}

#[cfg(feature = "_bzip2_any")]
fn bzip2_compression_level_range() -> std::ops::RangeInclusive<i64> {
    let min = bzip2::Compression::fast().level() as i64;
    let max = bzip2::Compression::best().level() as i64;
    min..=max
}

#[cfg(any(
    feature = "_deflate-any",
    feature = "_bzip2_any",
    feature = "ppmd",
    feature = "xz",
    feature = "zstd",
))]
fn validate_value_in_range<T: Ord + Copy, U: Ord + Copy + TryFrom<T>>(
    value: T,
    range: std::ops::RangeInclusive<U>,
) -> Option<T> {
    if range.contains(&value.try_into().ok()?) {
        Some(value)
    } else {
        None
    }
}

fn update_aes_extra_data<W: Write + Seek>(
    writer: &mut W,
    file: &mut ZipFileData,
    bytes_written: u64,
) -> ZipResult<()> {
    let Some((aes_mode, version, compression_method)) = &mut file.aes_mode else {
        return Ok(());
    };

    // We prefer using AE-1 which provides an extra CRC check, but for small files we
    // switch to AE-2 to prevent being able to use the CRC value to to reconstruct the
    // unencrypted contents.
    //
    // We can only do this when the underlying writer supports
    // seek operations, so we gate this behind using_data_descriptor.
    //
    // C.f. https://www.winzip.com/en/support/aes-encryption/#crc-faq
    *version = if bytes_written < 20 {
        AesVendorVersion::Ae2
    } else {
        AesVendorVersion::Ae1
    };

    let extra_data_start = file.extra_data_start.unwrap();

    writer.seek(SeekFrom::Start(
        extra_data_start + file.aes_extra_data_start,
    ))?;

    let mut buf = Vec::new();

    /* TODO: implement this using the Block trait! */
    // Extra field header ID.
    buf.write_u16_le(0x9901)?;
    // Data size.
    buf.write_u16_le(7)?;
    // Integer version number.
    buf.write_u16_le(*version as u16)?;
    // Vendor ID.
    buf.write_all(b"AE")?;
    // AES encryption strength.
    buf.write_all(&[*aes_mode as u8])?;
    // Real compression method.
    buf.write_u16_le(compression_method.serialize_to_u16())?;

    writer.write_all(&buf)?;

    let aes_extra_data_start = file.aes_extra_data_start as usize;
    let extra_field = Arc::get_mut(file.extra_field.as_mut().unwrap()).unwrap();
    extra_field[aes_extra_data_start..aes_extra_data_start + buf.len()].copy_from_slice(&buf);

    Ok(())
}

fn update_local_file_header<T: Write + Seek>(
    writer: &mut T,
    file: &mut ZipFileData,
) -> ZipResult<()> {
    const CRC32_OFFSET: u64 = 14;
    writer.seek(SeekFrom::Start(file.header_start + CRC32_OFFSET))?;
    writer.write_u32_le(file.crc32)?;
    if file.large_file {
        writer.write_u32_le(spec::ZIP64_BYTES_THR as u32)?;
        writer.write_u32_le(spec::ZIP64_BYTES_THR as u32)?;

        update_local_zip64_extra_field(writer, file)?;

        file.compressed_size = spec::ZIP64_BYTES_THR;
        file.uncompressed_size = spec::ZIP64_BYTES_THR;
    } else {
        // check compressed size as well as it can also be slightly larger than uncompressed size
        if file.compressed_size > spec::ZIP64_BYTES_THR {
            return Err(ZipError::Io(io::Error::other(
                "Large file option has not been set",
            )));
        }
        writer.write_u32_le(file.compressed_size as u32)?;
        // uncompressed size is already checked on write to catch it as soon as possible
        writer.write_u32_le(file.uncompressed_size as u32)?;
    }
    Ok(())
}

fn write_central_directory_header<T: Write>(writer: &mut T, file: &ZipFileData) -> ZipResult<()> {
    let mut block = file.block()?;
    let stripped_extra = if let Some(extra) = &file.extra_field {
        strip_alignment_extra_field(extra)
    } else {
        Vec::new()
    };
    let central_len = file.central_extra_field_len();
    let zip64_len = if let Some(zip64) = file.zip64_extra_field_block() {
        zip64.full_size()
    } else {
        0
    };
    block.extra_field_length = (zip64_len + stripped_extra.len() + central_len)
        .try_into()
        .map_err(|_| invalid!("Extra field length in central directory exceeds 64KiB"))?;

    block.write(writer)?;
    // file name
    writer.write_all(&file.file_name_raw)?;
    // extra field
    if let Some(zip64_extra_field) = &file.zip64_extra_field_block() {
        writer.write_all(&zip64_extra_field.serialize())?;
    }
    if !stripped_extra.is_empty() {
        writer.write_all(&stripped_extra)?;
    }
    if let Some(central_extra_field) = &file.central_extra_field {
        writer.write_all(central_extra_field)?;
    }
    // file comment
    writer.write_all(file.file_comment.as_bytes())?;

    Ok(())
}

fn strip_alignment_extra_field(extra_field: &[u8]) -> Vec<u8> {
    let mut new_extra = Vec::with_capacity(extra_field.len());
    let mut cursor = 0;
    while cursor + 4 <= extra_field.len() {
        let tag = u16::from_le_bytes([extra_field[cursor], extra_field[cursor + 1]]);
        let len = u16::from_le_bytes([extra_field[cursor + 2], extra_field[cursor + 3]]) as usize;
        if cursor + 4 + len > extra_field.len() {
            new_extra.extend_from_slice(&extra_field[cursor..]);
            break;
        }

        if tag != 0xa11e {
            new_extra.extend_from_slice(&extra_field[cursor..cursor + 4 + len]);
        }
        cursor += 4 + len;
    }
    if cursor < extra_field.len() {
        new_extra.extend_from_slice(&extra_field[cursor..]);
    }
    new_extra
}

fn update_local_zip64_extra_field<T: Write + Seek>(
    writer: &mut T,
    file: &mut ZipFileData,
) -> ZipResult<()> {
    let block = file.zip64_extra_field_block().ok_or(invalid!(
        "Attempted to update a nonexistent ZIP64 extra field"
    ))?;

    let zip64_extra_field_start = file.header_start
        + size_of::<ZipLocalEntryBlock>() as u64
        + file.file_name_raw.len() as u64;

    writer.seek(SeekFrom::Start(zip64_extra_field_start))?;
    let block = block.serialize();
    writer.write_all(&block)?;

    let extra_field = Arc::get_mut(file.extra_field.as_mut().unwrap()).unwrap();
    extra_field[..block.len()].copy_from_slice(&block);

    Ok(())
}

/// Wrapper around a [Write] implementation that implements the [Seek] trait, but where seeking
/// returns an error unless it's a no-op.
pub struct StreamWriter<W: Write> {
    inner: W,
    bytes_written: u64,
}

impl<W: Write> StreamWriter<W> {
    /// Creates an instance wrapping the provided inner writer.
    pub fn new(inner: W) -> StreamWriter<W> {
        Self {
            inner,
            bytes_written: 0,
        }
    }

    /// Consumes this wrapper, returning the underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for StreamWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.inner.write(buf)?;
        self.bytes_written += bytes_written as u64;
        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write> Seek for StreamWriter<W> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Current(0) | SeekFrom::End(0) => return Ok(self.bytes_written),
            SeekFrom::Start(x) => {
                if x == self.bytes_written {
                    return Ok(self.bytes_written);
                }
            }
            _ => {}
        }
        Err(io::Error::new(
            ErrorKind::Unsupported,
            "seek is not supported",
        ))
    }
}

#[cfg(not(feature = "unreserved"))]
const EXTRA_FIELD_MAPPING: [u16; 43] = [
    0x0007, 0x0008, 0x0009, 0x000a, 0x000c, 0x000d, 0x000e, 0x000f, 0x0014, 0x0015, 0x0016, 0x0017,
    0x0018, 0x0019, 0x0020, 0x0021, 0x0022, 0x0023, 0x0065, 0x0066, 0x4690, 0x07c8, 0x2605, 0x2705,
    0x2805, 0x334d, 0x4341, 0x4453, 0x4704, 0x470f, 0x4b46, 0x4c41, 0x4d49, 0x4f4c, 0x5356, 0x554e,
    0x5855, 0x6542, 0x756e, 0x7855, 0xa220, 0xfd4a, 0x9902,
];

#[cfg(test)]
#[allow(unknown_lints)] // needless_update is new in clippy pre 1.29.0
#[allow(clippy::needless_update)] // So we can use the same FileOptions decls with and without zopfli_buffer_size
#[allow(clippy::octal_escapes)] // many false positives in converted fuzz cases
mod test {
    use super::{ExtendedFileOptions, FileOptions, FullFileOptions, ZipWriter};
    use crate::compression::CompressionMethod;
    use crate::result::ZipResult;
    use crate::types::{DateTime, System};
    use crate::write::EncryptWith::ZipCrypto;
    use crate::write::SimpleFileOptions;
    use crate::zipcrypto::ZipCryptoKeys;
    use crate::CompressionMethod::Stored;
    use crate::ZipArchive;
    #[cfg(feature = "deflate-flate2")]
    use std::io::Read;
    use std::io::{Cursor, Write};
    use std::marker::PhantomData;
    use std::path::PathBuf;

    const SYSTEM_BYTE: u8 = if cfg!(windows) {
        System::Dos
    } else {
        System::Unix
    } as u8;

    #[test]
    fn write_empty_zip() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
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
        let options = SimpleFileOptions::default().unix_permissions(0o120777);
        assert_eq!(options.permissions, Some(0o777));
    }

    #[test]
    fn write_zip_dir() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_directory(
                "test",
                SimpleFileOptions::default().last_modified_time(
                    DateTime::from_date_and_time(2018, 8, 15, 20, 45, 6).unwrap(),
                ),
            )
            .unwrap();
        assert!(writer
            .write(b"writing to a directory is not allowed, and will not write any data")
            .is_err());
        let result = writer.finish().unwrap();
        assert_eq!(result.get_ref().len(), 108);
        const DOS_DIR_ATTRIBUTES_BYTE: u8 = if cfg!(windows) { 0x10 } else { 0 };
        #[rustfmt::skip]
        assert_eq!(
            *result.get_ref(),
            &[
                80u8, 75, 3, 4, 20, 0, 0, 0, 0, 0, 163, 165, 15, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 5, 0, 0, 0, 116, 101, 115, 116, 47, 80, 75, 1, 2, 20, SYSTEM_BYTE, 20, 0, 0, 0, 0, 0,
                163, 165, 15, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                DOS_DIR_ATTRIBUTES_BYTE, 0, 237, 65, 0, 0, 0, 0, 116, 101, 115, 116, 47, 80, 75, 5, 6, 0, 0, 0, 0, 1, 0,
                1, 0, 51, 0, 0, 0, 35, 0, 0, 0, 0, 0,
            ] as &[u8]
        );
    }

    #[test]
    fn write_symlink_simple() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "name",
                "target",
                SimpleFileOptions::default().last_modified_time(
                    DateTime::from_date_and_time(2018, 8, 15, 20, 45, 6).unwrap(),
                ),
            )
            .unwrap();
        assert!(writer
            .write(b"writing to a symlink is not allowed and will not write any data")
            .is_err());
        let result = writer.finish().unwrap();
        assert_eq!(result.get_ref().len(), 112);
        #[rustfmt::skip]
        assert_eq!(
            *result.get_ref(),
            &[
                80u8, 75, 3, 4, 10, 0, 0, 0, 0, 0, 163, 165, 15, 77, 252, 47, 111, 70, 6, 0, 0, 0,
                6, 0, 0, 0, 4, 0, 0, 0, 110, 97, 109, 101, 116, 97, 114, 103, 101, 116, 80, 75, 1,
                2, 10, System::Unix as u8, 10, 0, 0, 0, 0, 0, 163, 165, 15, 77, 252, 47, 111, 70, 6, 0, 0, 0, 6, 0,
                0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 161, 0, 0, 0, 0, 110, 97, 109, 101,
                80, 75, 5, 6, 0, 0, 0, 0, 1, 0, 1, 0, 50, 0, 0, 0, 40, 0, 0, 0, 0, 0
            ] as &[u8],
        );
    }

    #[test]
    fn test_path_normalization() {
        let mut path = PathBuf::new();
        path.push("foo");
        path.push("bar");
        path.push("..");
        path.push(".");
        path.push("example.txt");
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file_from_path(path, SimpleFileOptions::default())
            .unwrap();
        let archive = writer.finish_into_readable().unwrap();
        assert_eq!(Some("foo/example.txt"), archive.name_for_index(0));
    }

    #[test]
    fn write_symlink_wonky_paths() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "directory\\link",
                "/absolute/symlink\\with\\mixed/slashes",
                SimpleFileOptions::default().last_modified_time(
                    DateTime::from_date_and_time(2018, 8, 15, 20, 45, 6).unwrap(),
                ),
            )
            .unwrap();
        assert!(writer
            .write(b"writing to a symlink is not allowed and will not write any data")
            .is_err());
        let result = writer.finish().unwrap();
        assert_eq!(result.get_ref().len(), 162);
        #[rustfmt::skip]
        assert_eq!(
            *result.get_ref(),
            &[
                80u8, 75, 3, 4, 10, 0, 0, 0, 0, 0, 163, 165, 15, 77, 95, 41, 81, 245, 36, 0, 0, 0,
                36, 0, 0, 0, 14, 0, 0, 0, 100, 105, 114, 101, 99, 116, 111, 114, 121, 92, 108, 105,
                110, 107, 47, 97, 98, 115, 111, 108, 117, 116, 101, 47, 115, 121, 109, 108, 105,
                110, 107, 92, 119, 105, 116, 104, 92, 109, 105, 120, 101, 100, 47, 115, 108, 97,
                115, 104, 101, 115, 80, 75, 1, 2, 10, System::Unix as u8, 10, 0, 0, 0, 0, 0, 163, 165, 15, 77, 95,
                41, 81, 245, 36, 0, 0, 0, 36, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255,
                161, 0, 0, 0, 0, 100, 105, 114, 101, 99, 116, 111, 114, 121, 92, 108, 105, 110,
                107, 80, 75, 5, 6, 0, 0, 0, 0, 1, 0, 1, 0, 60, 0, 0, 0, 80, 0, 0, 0, 0, 0
            ] as &[u8],
        );
    }

    #[test]
    fn write_mimetype_zip() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::default(),
            permissions: Some(33188),
            large_file: false,
            encrypt_with: None,
            extended_options: (),
            alignment: 1,
            #[cfg(feature = "deflate-zopfli")]
            zopfli_buffer_size: None,
            #[cfg(feature = "aes-crypto")]
            aes_mode: None,
        };
        writer.start_file("mimetype", options).unwrap();
        writer
            .write_all(b"application/vnd.oasis.opendocument.text")
            .unwrap();
        let result = writer.finish().unwrap();

        assert_eq!(result.get_ref().len(), 153);
        #[rustfmt::skip]
        assert_eq!(result.get_ref(), &[80, 75, 3, 4, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 94, 198, 50,
            12, 39, 0, 0, 0, 39, 0, 0, 0, 8, 0, 0, 0, 109, 105, 109, 101, 116, 121, 112, 101, 97,
            112, 112, 108, 105, 99, 97, 116, 105, 111, 110, 47, 118, 110, 100, 46, 111, 97, 115,
            105, 115, 46, 111, 112, 101, 110, 100, 111, 99, 117, 109, 101, 110, 116, 46, 116, 101,
            120, 116, 80, 75, 1, 2, 10, SYSTEM_BYTE, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 94, 198, 50,
            12, 39, 0, 0, 0, 39, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 129, 0, 0, 0, 0,
            109, 105, 109, 101, 116, 121, 112, 101, 80, 75, 5, 6, 0, 0, 0, 0, 1, 0, 1, 0, 54, 0, 0,
            0, 77, 0, 0, 0, 0, 0]);
    }

    #[cfg(feature = "deflate-flate2")]
    const RT_TEST_TEXT: &str = "And I can't stop thinking about the moments that I lost to you\
                            And I can't stop thinking of things I used to do\
                            And I can't stop making bad decisions\
                            And I can't stop eating stuff you make me chew\
                            I put on a smile like you wanna see\
                            Another day goes by that I long to be like you";
    #[cfg(feature = "deflate-flate2")]
    const RT_TEST_FILENAME: &str = "subfolder/sub-subfolder/can't_stop.txt";
    #[cfg(feature = "deflate-flate2")]
    const SECOND_FILENAME: &str = "different_name.xyz";
    #[cfg(feature = "deflate-flate2")]
    const THIRD_FILENAME: &str = "third_name.xyz";

    #[test]
    fn write_non_utf8() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::default(),
            permissions: Some(33188),
            large_file: false,
            encrypt_with: None,
            extended_options: (),
            alignment: 1,
            #[cfg(feature = "deflate-zopfli")]
            zopfli_buffer_size: None,
            #[cfg(feature = "aes-crypto")]
            aes_mode: None,
        };

        // GB18030
        // "中文" = [214, 208, 206, 196]
        let filename = unsafe { String::from_utf8_unchecked(vec![214, 208, 206, 196]) };
        writer.start_file(filename, options).unwrap();
        writer.write_all(b"encoding GB18030").unwrap();

        // SHIFT_JIS
        // "日文" = [147, 250, 149, 182]
        let filename = unsafe { String::from_utf8_unchecked(vec![147, 250, 149, 182]) };
        writer.start_file(filename, options).unwrap();
        writer.write_all(b"encoding SHIFT_JIS").unwrap();
        let result = writer.finish().unwrap();

        assert_eq!(result.get_ref().len(), 224);
        #[rustfmt::skip]
        assert_eq!(result.get_ref(), &[80, 75, 3, 4, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 29, 183, 125,
            75, 16, 0, 0, 0, 16, 0, 0, 0, 4, 0, 0, 0, 214, 208, 206, 196, 101, 110, 99, 111, 100,
            105, 110, 103, 32, 71, 66, 49, 56, 48, 51, 48, 80, 75, 3, 4, 10, 0, 0, 0, 0, 0, 0, 0,
            33, 0, 75, 110, 14, 242, 18, 0, 0, 0, 18, 0, 0, 0, 4, 0, 0, 0, 147, 250, 149, 182, 101,
            110, 99, 111, 100, 105, 110, 103, 32, 83, 72, 73, 70, 84, 95, 74, 73, 83, 80, 75, 1, 2,
            10, SYSTEM_BYTE, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 29, 183, 125, 75, 16, 0, 0, 0, 16, 0,
            0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 129, 0, 0, 0, 0, 214, 208, 206, 196, 80,
            75, 1, 2, 10, SYSTEM_BYTE, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 75, 110, 14, 242, 18, 0, 0, 0, 18, 0,
            0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 129, 50, 0, 0, 0, 147, 250, 149, 182, 80,
            75, 5, 6, 0, 0, 0, 0, 2, 0, 2, 0, 100, 0, 0, 0, 102, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn path_to_string() {
        let mut path = PathBuf::new();
        #[cfg(windows)]
        path.push(r"C:\");
        #[cfg(unix)]
        path.push("/");
        path.push("windows");
        path.push("..");
        path.push(".");
        path.push("system32");
        let path_str = super::path_to_string(&path);
        assert_eq!(&*path_str, "system32");
    }

    #[test]
    #[cfg(feature = "deflate-flate2")]
    fn test_shallow_copy() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = FileOptions {
            compression_method: CompressionMethod::default(),
            compression_level: None,
            last_modified_time: DateTime::default(),
            permissions: Some(33188),
            large_file: false,
            encrypt_with: None,
            extended_options: (),
            alignment: 0,
            #[cfg(feature = "deflate-zopfli")]
            zopfli_buffer_size: None,
            #[cfg(feature = "aes-crypto")]
            aes_mode: None,
        };
        writer.start_file(RT_TEST_FILENAME, options).unwrap();
        writer.write_all(RT_TEST_TEXT.as_ref()).unwrap();
        writer
            .shallow_copy_file(RT_TEST_FILENAME, SECOND_FILENAME)
            .unwrap();
        writer
            .shallow_copy_file(RT_TEST_FILENAME, SECOND_FILENAME)
            .expect_err("Duplicate filename");
        let zip = writer.finish().unwrap();
        let mut writer = ZipWriter::new_append(zip).unwrap();
        writer
            .shallow_copy_file(SECOND_FILENAME, SECOND_FILENAME)
            .expect_err("Duplicate filename");
        let mut reader = writer.finish_into_readable().unwrap();
        let mut file_names: Vec<&str> = reader.file_names().collect();
        file_names.sort();
        let mut expected_file_names = vec![RT_TEST_FILENAME, SECOND_FILENAME];
        expected_file_names.sort();
        assert_eq!(file_names, expected_file_names);
        let mut first_file_content = String::new();
        reader
            .by_name(RT_TEST_FILENAME)
            .unwrap()
            .read_to_string(&mut first_file_content)
            .unwrap();
        assert_eq!(first_file_content, RT_TEST_TEXT);
        let mut second_file_content = String::new();
        reader
            .by_name(SECOND_FILENAME)
            .unwrap()
            .read_to_string(&mut second_file_content)
            .unwrap();
        assert_eq!(second_file_content, RT_TEST_TEXT);
    }

    #[test]
    #[cfg(feature = "deflate-flate2")]
    fn test_deep_copy() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = FileOptions {
            compression_method: CompressionMethod::default(),
            compression_level: None,
            last_modified_time: DateTime::default(),
            permissions: Some(33188),
            large_file: false,
            encrypt_with: None,
            extended_options: (),
            alignment: 0,
            #[cfg(feature = "deflate-zopfli")]
            zopfli_buffer_size: None,
            #[cfg(feature = "aes-crypto")]
            aes_mode: None,
        };
        writer.start_file(RT_TEST_FILENAME, options).unwrap();
        writer.write_all(RT_TEST_TEXT.as_ref()).unwrap();
        writer
            .deep_copy_file(RT_TEST_FILENAME, SECOND_FILENAME)
            .unwrap();
        let zip = writer.finish().unwrap().into_inner();
        zip.iter().copied().for_each(|x| print!("{x:02x}"));
        println!();
        let mut writer = ZipWriter::new_append(Cursor::new(zip)).unwrap();
        writer
            .deep_copy_file(RT_TEST_FILENAME, THIRD_FILENAME)
            .unwrap();
        let zip = writer.finish().unwrap();
        let mut reader = ZipArchive::new(zip).unwrap();
        let mut file_names: Vec<&str> = reader.file_names().collect();
        file_names.sort();
        let mut expected_file_names = vec![RT_TEST_FILENAME, SECOND_FILENAME, THIRD_FILENAME];
        expected_file_names.sort();
        assert_eq!(file_names, expected_file_names);
        let mut first_file_content = String::new();
        reader
            .by_name(RT_TEST_FILENAME)
            .unwrap()
            .read_to_string(&mut first_file_content)
            .unwrap();
        assert_eq!(first_file_content, RT_TEST_TEXT);
        let mut second_file_content = String::new();
        reader
            .by_name(SECOND_FILENAME)
            .unwrap()
            .read_to_string(&mut second_file_content)
            .unwrap();
        assert_eq!(second_file_content, RT_TEST_TEXT);
    }

    #[test]
    fn duplicate_filenames() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("foo/bar/test", SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all("The quick brown 🦊 jumps over the lazy 🐕".as_bytes())
            .unwrap();
        writer
            .start_file("foo/bar/test", SimpleFileOptions::default())
            .expect_err("Expected duplicate filename not to be allowed");
    }

    #[test]
    fn test_filename_looks_like_zip64_locator() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "PK\u{6}\u{7}\0\0\0\u{11}\0\0\0\0\0\0\0\0\0\0\0\0",
                SimpleFileOptions::default(),
            )
            .unwrap();
        let zip = writer.finish().unwrap();
        let _ = ZipArchive::new(zip).unwrap();
    }

    #[test]
    fn test_filename_looks_like_zip64_locator_2() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "PK\u{6}\u{6}\0\0\0\0\0\0\0\0\0\0PK\u{6}\u{7}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                SimpleFileOptions::default(),
            )
            .unwrap();
        let zip = writer.finish().unwrap();
        let _ = ZipArchive::new(zip).unwrap();
    }

    #[test]
    fn test_filename_looks_like_zip64_locator_2a() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "PK\u{6}\u{6}PK\u{6}\u{7}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                SimpleFileOptions::default(),
            )
            .unwrap();
        let zip = writer.finish().unwrap();
        let _ = ZipArchive::new(zip).unwrap();
    }

    #[test]
    fn test_filename_looks_like_zip64_locator_3() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("\0PK\u{6}\u{6}", SimpleFileOptions::default())
            .unwrap();
        writer
            .start_file(
                "\0\u{4}\0\0PK\u{6}\u{7}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{3}",
                SimpleFileOptions::default(),
            )
            .unwrap();
        let zip = writer.finish().unwrap();
        let _ = ZipArchive::new(zip).unwrap();
    }

    #[test]
    fn test_filename_looks_like_zip64_locator_4() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("PK\u{6}\u{6}", SimpleFileOptions::default())
            .unwrap();
        writer
            .start_file("\0\0\0\0\0\0", SimpleFileOptions::default())
            .unwrap();
        writer
            .start_file("\0", SimpleFileOptions::default())
            .unwrap();
        writer.start_file("", SimpleFileOptions::default()).unwrap();
        writer
            .start_file("\0\0", SimpleFileOptions::default())
            .unwrap();
        writer
            .start_file(
                "\0\0\0PK\u{6}\u{7}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                SimpleFileOptions::default(),
            )
            .unwrap();
        let zip = writer.finish().unwrap();
        let _ = ZipArchive::new(zip).unwrap();
    }

    #[test]
    fn test_filename_looks_like_zip64_locator_5() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_directory("", SimpleFileOptions::default().with_alignment(21))
            .unwrap();
        let mut writer = ZipWriter::new_append(writer.finish().unwrap()).unwrap();
        writer.shallow_copy_file("/", "").unwrap();
        writer.shallow_copy_file("", "\0").unwrap();
        writer.shallow_copy_file("\0", "PK\u{6}\u{6}").unwrap();
        let mut writer = ZipWriter::new_append(writer.finish().unwrap()).unwrap();
        writer
            .start_file("\0\0\0\0\0\0", SimpleFileOptions::default())
            .unwrap();
        let mut writer = ZipWriter::new_append(writer.finish().unwrap()).unwrap();
        writer
            .start_file(
                "#PK\u{6}\u{7}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                SimpleFileOptions::default(),
            )
            .unwrap();
        let zip = writer.finish().unwrap();
        let _ = ZipArchive::new(zip).unwrap();
        Ok(())
    }

    #[test]
    #[cfg(feature = "deflate-flate2")]
    fn remove_shallow_copy_keeps_original() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("original", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(RT_TEST_TEXT.as_bytes()).unwrap();
        writer
            .shallow_copy_file("original", "shallow_copy")
            .unwrap();
        writer.abort_file().unwrap();
        let mut zip = ZipArchive::new(writer.finish().unwrap()).unwrap();
        let mut file = zip.by_name("original").unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(RT_TEST_TEXT.as_bytes(), contents);
        Ok(())
    }

    #[test]
    fn remove_encrypted_file() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let first_file_options = SimpleFileOptions::default()
            .with_alignment(65535)
            .with_deprecated_encryption(b"Password");
        writer.start_file("", first_file_options).unwrap();
        writer.abort_file().unwrap();
        let zip = writer.finish().unwrap();
        let mut writer = ZipWriter::new(zip);
        writer.start_file("", SimpleFileOptions::default()).unwrap();
        Ok(())
    }

    #[test]
    fn remove_encrypted_aligned_symlink() -> ZipResult<()> {
        let mut options = SimpleFileOptions::default();
        options = options.with_deprecated_encryption(b"Password");
        options.alignment = 65535;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.add_symlink("", "s\t\0\0ggggg\0\0", options).unwrap();
        writer.abort_file().unwrap();
        let zip = writer.finish().unwrap();
        let mut writer = ZipWriter::new_append(zip).unwrap();
        writer.start_file("", SimpleFileOptions::default()).unwrap();
        Ok(())
    }

    #[cfg(feature = "deflate-zopfli")]
    #[test]
    fn zopfli_empty_write() -> ZipResult<()> {
        let mut options = SimpleFileOptions::default();
        options = options
            .compression_method(CompressionMethod::default())
            .compression_level(Some(264));
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.start_file("", options).unwrap();
        writer.write_all(&[]).unwrap();
        writer.write_all(&[]).unwrap();
        Ok(())
    }

    #[test]
    fn crash_with_no_features() -> ZipResult<()> {
        const ORIGINAL_FILE_NAME: &str = "PK\u{6}\u{6}\0\0\0\0\0\0\0\0\0\u{2}g\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\0\0PK\u{6}\u{7}\0\0\0\0\0\0\0\0\0\0\0\0\u{7}\0\t'";
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let mut options = SimpleFileOptions::default();
        options = options.with_alignment(3584).compression_method(Stored);
        writer.start_file(ORIGINAL_FILE_NAME, options)?;
        let archive = writer.finish()?;
        let mut writer = ZipWriter::new_append(archive)?;
        writer.shallow_copy_file(ORIGINAL_FILE_NAME, "\u{6}\\")?;
        writer.finish()?;
        Ok(())
    }

    #[test]
    fn test_alignment() {
        let page_size = 4096;
        let options = SimpleFileOptions::default()
            .compression_method(Stored)
            .with_alignment(page_size);
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let contents = b"sleeping";
        let () = zip.start_file("sleep", options).unwrap();
        let _count = zip.write(&contents[..]).unwrap();
        let mut zip = zip.finish_into_readable().unwrap();
        let file = zip.by_index(0).unwrap();
        assert_eq!(file.name(), "sleep");
        assert_eq!(file.data_start(), u64::from(page_size));
    }

    #[test]
    fn test_alignment_2() {
        let page_size = 4096;
        let mut data = Vec::new();
        {
            let options = SimpleFileOptions::default()
                .compression_method(Stored)
                .with_alignment(page_size);
            let mut zip = ZipWriter::new(Cursor::new(&mut data));
            let contents = b"sleeping";
            let () = zip.start_file("sleep", options).unwrap();
            let _count = zip.write(&contents[..]).unwrap();
        }
        assert_eq!(data[4096..4104], b"sleeping"[..]);
        {
            let mut zip = ZipArchive::new(Cursor::new(&mut data)).unwrap();
            let file = zip.by_index(0).unwrap();
            assert_eq!(file.name(), "sleep");
            assert_eq!(file.data_start(), u64::from(page_size));
        }
    }

    #[test]
    fn test_crash_short_read() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let comment = vec![
            1, 80, 75, 5, 6, 237, 237, 237, 237, 237, 237, 237, 237, 44, 255, 191, 255, 255, 255,
            255, 255, 255, 255, 255, 16,
        ]
        .into_boxed_slice();
        writer.set_raw_comment(comment);
        let options = SimpleFileOptions::default()
            .compression_method(Stored)
            .with_alignment(11823);
        writer.start_file("", options).unwrap();
        writer.write_all(&[255, 255, 44, 255, 0]).unwrap();
        let written = writer.finish().unwrap();
        let _ = ZipWriter::new_append(written).unwrap();
    }

    #[cfg(all(feature = "_deflate-any", feature = "aes-crypto"))]
    #[test]
    fn test_fuzz_failure_2024_05_08() -> ZipResult<()> {
        let mut first_writer = ZipWriter::new(Cursor::new(Vec::new()));
        let mut second_writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default()
            .compression_method(Stored)
            .with_alignment(46036);
        second_writer.add_symlink("\0", "", options)?;
        let second_archive = second_writer.finish_into_readable()?.into_inner();
        let mut second_writer = ZipWriter::new_append(second_archive)?;
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .large_file(true)
            .with_alignment(46036)
            .with_aes_encryption(crate::AesMode::Aes128, "\0\0");
        second_writer.add_symlink("", "", options)?;
        let second_archive = second_writer.finish_into_readable()?.into_inner();
        let mut second_writer = ZipWriter::new_append(second_archive)?;
        let options = SimpleFileOptions::default().compression_method(Stored);
        second_writer.start_file(" ", options)?;
        let second_archive = second_writer.finish_into_readable()?;
        first_writer.merge_archive(second_archive)?;
        let _ = ZipArchive::new(first_writer.finish()?)?;
        Ok(())
    }

    #[cfg(all(feature = "_bzip2_any", not(miri)))]
    #[test]
    fn test_fuzz_failure_2024_06_08() -> ZipResult<()> {
        use crate::write::ExtendedFileOptions;
        use CompressionMethod::Bzip2;

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        const SYMLINK_PATH: &str = "PK\u{6}\u{6}K\u{6}\u{6}\u{6}\0\0\0\0\u{18}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\u{1}\0\0PK\u{1}\u{2},\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0PK\u{1}\u{2},\0\0\0\0\0\0\0\0\0\0l\0\0\0\0\0\0PK\u{6}\u{7}P\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0";
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let options = FileOptions {
                compression_method: Bzip2,
                compression_level: None,
                last_modified_time: DateTime::from_date_and_time(1980, 5, 20, 21, 0, 57)?,
                permissions: None,
                large_file: false,
                encrypt_with: None,
                extended_options: ExtendedFileOptions {
                    extra_data: vec![].into(),
                    central_extra_data: vec![].into(),
                },
                alignment: 2048,
                ..Default::default()
            };
            writer.add_symlink_from_path(SYMLINK_PATH, "||\0\0\0\0", options)?;
            writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
            writer.deep_copy_file_from_path(SYMLINK_PATH, "")?;
            writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
            writer.abort_file()?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        writer.deep_copy_file_from_path(SYMLINK_PATH, "foo")?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_short_extra_data() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![99, 0, 15, 0, 207].into(),
            },
            ..Default::default()
        };
        assert!(writer.start_file_from_path("", options).is_err());
    }

    #[test]
    #[cfg(not(feature = "unreserved"))]
    fn test_invalid_extra_data() -> ZipResult<()> {
        use crate::write::ExtendedFileOptions;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(1980, 1, 4, 6, 54, 0)?,
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![
                    7, 0, 15, 0, 207, 117, 177, 117, 112, 2, 0, 255, 255, 131, 255, 255, 255, 80,
                    185,
                ]
                .into(),
            },
            alignment: 32787,
            ..Default::default()
        };
        assert!(writer.start_file_from_path("", options).is_err());
        Ok(())
    }

    #[test]
    #[cfg(not(feature = "unreserved"))]
    fn test_invalid_extra_data_unreserved() {
        use crate::write::ExtendedFileOptions;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2021, 8, 8, 1, 0, 29).unwrap(),
            permissions: None,
            large_file: true,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![
                    1, 41, 4, 0, 1, 255, 245, 117, 117, 112, 5, 0, 80, 255, 149, 255, 247,
                ]
                .into(),
            },
            alignment: 4103,
            ..Default::default()
        };
        assert!(writer.start_file_from_path("", options).is_err());
    }

    #[cfg(feature = "deflate64")]
    #[test]
    fn test_fuzz_crash_2024_06_13a() -> ZipResult<()> {
        use crate::write::ExtendedFileOptions;
        use CompressionMethod::Deflate64;

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Deflate64,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2039, 4, 17, 6, 18, 19)?,
            permissions: None,
            large_file: true,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 4,
            ..Default::default()
        };
        writer.add_directory_from_path("", options)?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_13b() -> ZipResult<()> {
        use crate::write::ExtendedFileOptions;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let options = FileOptions {
                compression_method: Stored,
                compression_level: None,
                last_modified_time: DateTime::from_date_and_time(1980, 4, 14, 6, 11, 54)?,
                permissions: None,
                large_file: false,
                encrypt_with: None,
                extended_options: ExtendedFileOptions {
                    extra_data: vec![].into(),
                    central_extra_data: vec![].into(),
                },
                alignment: 185,
                ..Default::default()
            };
            writer.add_symlink_from_path("", "", options)?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        writer.deep_copy_file_from_path("", "_copy")?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_14() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let options = FullFileOptions {
                compression_method: Stored,
                large_file: true,
                alignment: 93,
                ..Default::default()
            };
            writer.start_file_from_path("\0", options)?;
            writer = ZipWriter::new_append(writer.finish()?)?;
            writer.deep_copy_file_from_path("\0", "")?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        writer.deep_copy_file_from_path("", "copy")?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_14a() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2083, 5, 30, 21, 45, 35)?,
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 2565,
            ..Default::default()
        };
        writer.add_symlink_from_path("", "", options)?;
        writer.abort_file()?;
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::default(),
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 0,
            ..Default::default()
        };
        writer.start_file_from_path("", options)?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[allow(deprecated)]
    #[test]
    fn test_fuzz_crash_2024_06_14b() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2078, 3, 6, 12, 48, 58)?,
            permissions: None,
            large_file: true,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 65521,
            ..Default::default()
        };
        writer.start_file_from_path("\u{4}\0@\n//\u{c}", options)?;
        writer = ZipWriter::new_append(writer.finish()?)?;
        writer.abort_file()?;
        let options = FileOptions {
            compression_method: CompressionMethod::Unsupported(65535),
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2055, 10, 2, 11, 48, 49)?,
            permissions: None,
            large_file: true,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![255, 255, 1, 0, 255, 0, 0, 0, 0].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 65535,
            ..Default::default()
        };
        writer.add_directory_from_path("", options)?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_14c() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let options = FileOptions {
                compression_method: Stored,
                compression_level: None,
                last_modified_time: DateTime::from_date_and_time(2060, 4, 6, 13, 13, 3)?,
                permissions: None,
                large_file: true,
                encrypt_with: None,
                extended_options: ExtendedFileOptions {
                    extra_data: vec![].into(),
                    central_extra_data: vec![].into(),
                },
                alignment: 0,
                ..Default::default()
            };
            writer.start_file_from_path("\0", options)?;
            writer.write_all(&([]))?;
            writer = ZipWriter::new_append(writer.finish()?)?;
            writer.deep_copy_file_from_path("\0", "")?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        writer.deep_copy_file_from_path("", "_copy")?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[cfg(all(feature = "_deflate-any", feature = "aes-crypto"))]
    #[test]
    fn test_fuzz_crash_2024_06_14d() -> ZipResult<()> {
        use crate::write::EncryptWith::Aes;
        use crate::AesMode::Aes256;
        use CompressionMethod::Deflated;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Deflated,
            compression_level: Some(5),
            last_modified_time: DateTime::from_date_and_time(2107, 4, 8, 15, 54, 19)?,
            permissions: None,
            large_file: true,
            encrypt_with: Some(Aes {
                mode: Aes256,
                password: "",
            }),
            extended_options: ExtendedFileOptions {
                extra_data: vec![2, 0, 1, 0, 0].into(),
                central_extra_data: vec![
                    35, 229, 2, 0, 41, 41, 231, 44, 2, 0, 52, 233, 82, 201, 0, 0, 3, 0, 2, 0, 233,
                    255, 3, 0, 2, 0, 26, 154, 38, 251, 0, 0,
                ]
                .into(),
            },
            alignment: 65535,
            ..Default::default()
        };
        assert!(writer.add_directory_from_path("", options).is_err());
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_14e() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(1988, 1, 1, 1, 6, 26)?,
            permissions: None,
            large_file: true,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![76, 0, 1, 0, 0, 2, 0, 0, 0].into(),
                central_extra_data: vec![
                    1, 149, 1, 0, 255, 3, 0, 0, 0, 2, 255, 0, 0, 12, 65, 1, 0, 0, 67, 149, 0, 0,
                    76, 149, 2, 0, 149, 149, 67, 149, 0, 0,
                ]
                .into(),
            },
            alignment: 65535,
            ..Default::default()
        };
        assert!(writer.add_directory_from_path("", options).is_err());
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[allow(deprecated)]
    #[test]
    fn test_fuzz_crash_2024_06_17() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let sub_writer = {
                let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                writer.set_flush_on_finish_file(false);
                let sub_writer = {
                    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                    writer.set_flush_on_finish_file(false);
                    let sub_writer = {
                        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                        writer.set_flush_on_finish_file(false);
                        let sub_writer = {
                            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                            writer.set_flush_on_finish_file(false);
                            let sub_writer = {
                                let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                                writer.set_flush_on_finish_file(false);
                                let sub_writer = {
                                    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                                    writer.set_flush_on_finish_file(false);
                                    let sub_writer = {
                                        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                                        writer.set_flush_on_finish_file(false);
                                        let sub_writer = {
                                            let mut writer =
                                                ZipWriter::new(Cursor::new(Vec::new()));
                                            writer.set_flush_on_finish_file(false);
                                            let options = FileOptions {
                                                compression_method: CompressionMethod::Unsupported(
                                                    65535,
                                                ),
                                                compression_level: Some(5),
                                                last_modified_time: DateTime::from_date_and_time(
                                                    2107, 2, 8, 15, 0, 0,
                                                )?,
                                                permissions: None,
                                                large_file: true,
                                                encrypt_with: Some(ZipCrypto(
                                                    ZipCryptoKeys::of(
                                                        0x63ff, 0xc62d3103, 0xfffe00ea,
                                                    ),
                                                    PhantomData,
                                                )),
                                                extended_options: ExtendedFileOptions {
                                                    extra_data: vec![].into(),
                                                    central_extra_data: vec![].into(),
                                                },
                                                alignment: 255,
                                                ..Default::default()
                                            };
                                            writer.add_symlink_from_path("1\0PK\u{6}\u{6}\u{b}\u{6}\u{6}\u{6}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\u{b}\0\0PK\u{1}\u{2},\0\0\0\0\0\0\0\0\0\0\0\u{10}\0\0\0K\u{6}\u{6}\0\0\0\0\0\0\0\0PK\u{2}\u{6}", "", options)?;
                                            writer = ZipWriter::new_append(
                                                writer.finish_into_readable()?.into_inner(),
                                            )?;
                                            writer
                                        };
                                        writer.merge_archive(sub_writer.finish_into_readable()?)?;
                                        writer = ZipWriter::new_append(
                                            writer.finish_into_readable()?.into_inner(),
                                        )?;
                                        let options = FileOptions {
                                            compression_method: Stored,
                                            compression_level: None,
                                            last_modified_time: DateTime::from_date_and_time(
                                                1992, 7, 3, 0, 0, 0,
                                            )?,
                                            permissions: None,
                                            large_file: true,
                                            encrypt_with: None,
                                            extended_options: ExtendedFileOptions {
                                                extra_data: vec![].into(),
                                                central_extra_data: vec![].into(),
                                            },
                                            alignment: 43,
                                            ..Default::default()
                                        };
                                        writer.start_file_from_path(
                                            "\0\0\0\u{3}\0\u{1a}\u{1a}\u{1a}\u{1a}\u{1a}\u{1a}",
                                            options,
                                        )?;
                                        let options = FileOptions {
                                            compression_method: Stored,
                                            compression_level: None,
                                            last_modified_time: DateTime::from_date_and_time(
                                                2006, 3, 27, 2, 24, 26,
                                            )?,
                                            permissions: None,
                                            large_file: false,
                                            encrypt_with: None,
                                            extended_options: ExtendedFileOptions {
                                                extra_data: vec![].into(),
                                                central_extra_data: vec![].into(),
                                            },
                                            alignment: 26,
                                            ..Default::default()
                                        };
                                        writer.start_file_from_path("\0K\u{6}\u{6}\0PK\u{6}\u{7}PK\u{6}\u{6}\0\0\0\0\0\0\0\0PK\u{2}\u{6}", options)?;
                                        writer = ZipWriter::new_append(
                                            writer.finish_into_readable()?.into_inner(),
                                        )?;
                                        let options = FileOptions {
                                            compression_method: Stored,
                                            compression_level: Some(17),
                                            last_modified_time: DateTime::from_date_and_time(
                                                2103, 4, 10, 23, 15, 18,
                                            )?,
                                            permissions: Some(3284386755),
                                            large_file: true,
                                            encrypt_with: Some(ZipCrypto(
                                                ZipCryptoKeys::of(
                                                    0x8888c5bf, 0x88888888, 0xff888888,
                                                ),
                                                PhantomData,
                                            )),
                                            extended_options: ExtendedFileOptions {
                                                extra_data: vec![3, 0, 1, 0, 255, 144, 136, 0, 0]
                                                    .into(),
                                                central_extra_data: vec![].into(),
                                            },
                                            alignment: 65535,
                                            ..Default::default()
                                        };
                                        writer.add_symlink_from_path("", "\nu", options)?;
                                        writer = ZipWriter::new_append(writer.finish()?)?;
                                        writer
                                    };
                                    writer.merge_archive(sub_writer.finish_into_readable()?)?;
                                    writer = ZipWriter::new_append(
                                        writer.finish_into_readable()?.into_inner(),
                                    )?;
                                    writer
                                };
                                writer.merge_archive(sub_writer.finish_into_readable()?)?;
                                writer = ZipWriter::new_append(writer.finish()?)?;
                                writer
                            };
                            writer.merge_archive(sub_writer.finish_into_readable()?)?;
                            writer =
                                ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
                            writer.abort_file()?;
                            let options = FileOptions {
                                compression_method: CompressionMethod::Unsupported(49603),
                                compression_level: Some(20),
                                last_modified_time: DateTime::from_date_and_time(
                                    2047, 4, 14, 3, 15, 14,
                                )?,
                                permissions: Some(3284386755),
                                large_file: true,
                                encrypt_with: Some(ZipCrypto(
                                    ZipCryptoKeys::of(0xc3, 0x0, 0x0),
                                    PhantomData,
                                )),
                                extended_options: ExtendedFileOptions {
                                    extra_data: vec![].into(),
                                    central_extra_data: vec![].into(),
                                },
                                alignment: 0,
                                ..Default::default()
                            };
                            writer.add_directory_from_path("", options)?;
                            writer.deep_copy_file_from_path("/", "")?;
                            writer.shallow_copy_file_from_path("", "copy")?;
                            assert!(writer.shallow_copy_file_from_path("", "copy").is_err());
                            assert!(writer.shallow_copy_file_from_path("", "copy").is_err());
                            assert!(writer.shallow_copy_file_from_path("", "copy").is_err());
                            assert!(writer.shallow_copy_file_from_path("", "copy").is_err());
                            assert!(writer.shallow_copy_file_from_path("", "copy").is_err());
                            assert!(writer.shallow_copy_file_from_path("", "copy").is_err());
                            writer
                        };
                        writer.merge_archive(sub_writer.finish_into_readable()?)?;
                        writer
                    };
                    writer.merge_archive(sub_writer.finish_into_readable()?)?;
                    writer
                };
                writer.merge_archive(sub_writer.finish_into_readable()?)?;
                writer
            };
            writer.merge_archive(sub_writer.finish_into_readable()?)?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_17a() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        const PATH_1: &str = "\0I\01\0P\0\0\u{2}\0\0\u{1a}\u{1a}\u{1a}\u{1a}\u{1b}\u{1a}UT\u{5}\0\0\u{1a}\u{1a}\u{1a}\u{1a}UT\u{5}\0\u{1}\0\u{1a}\u{1a}\u{1a}UT\t\0uc\u{5}\0\0\0\0\u{7f}\u{7f}\u{7f}\u{7f}PK\u{6}";
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let sub_writer = {
                let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                writer.set_flush_on_finish_file(false);
                let options = FileOptions {
                    compression_method: Stored,
                    compression_level: None,
                    last_modified_time: DateTime::from_date_and_time(1981, 1, 1, 0, 24, 21)?,
                    permissions: Some(16908288),
                    large_file: false,
                    encrypt_with: None,
                    extended_options: ExtendedFileOptions {
                        extra_data: vec![].into(),
                        central_extra_data: vec![].into(),
                    },
                    alignment: 20555,
                    ..Default::default()
                };
                writer.start_file_from_path(
                    "\0\u{7}\u{1}\0\0\0\0\0\0\0\0\u{1}\0\0PK\u{1}\u{2};",
                    options,
                )?;
                writer.write_all(
                    &([
                        255, 255, 255, 255, 253, 253, 253, 203, 203, 203, 253, 253, 253, 253, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 249, 191, 225, 225,
                        241, 197,
                    ]),
                )?;
                writer.write_all(
                    &([
                        197, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 75, 0,
                    ]),
                )?;
                writer
            };
            writer.merge_archive(sub_writer.finish_into_readable()?)?;
            writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
            let options = FileOptions {
                compression_method: Stored,
                compression_level: None,
                last_modified_time: DateTime::from_date_and_time(1980, 11, 14, 10, 46, 47)?,
                permissions: None,
                large_file: false,
                encrypt_with: None,
                extended_options: ExtendedFileOptions {
                    extra_data: vec![].into(),
                    central_extra_data: vec![].into(),
                },
                alignment: 0,
                ..Default::default()
            };
            writer.start_file_from_path(PATH_1, options)?;
            writer.deep_copy_file_from_path(PATH_1, "eee\u{6}\0\0\0\0\0\0\0\0\0\0\0$\0\0\0\0\0\0\u{7f}\u{7f}PK\u{6}\u{6}K\u{6}\u{6}\u{6}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\u{1}\0\0PK\u{1}\u{1e},\0\0\0\0\0\0\0\0\0\0\0\u{8}\0*\0\0\u{1}PK\u{6}\u{7}PK\u{6}\u{6}\0\0\0\0\0\0\0\0}K\u{2}\u{6}")?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        writer.deep_copy_file_from_path(PATH_1, "")?;
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        writer.shallow_copy_file_from_path("", "copy")?;
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    #[allow(clippy::octal_escapes)]
    #[cfg(all(feature = "_bzip2_any", not(miri)))]
    fn test_fuzz_crash_2024_06_17b() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let sub_writer = {
                let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                writer.set_flush_on_finish_file(false);
                let sub_writer = {
                    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                    writer.set_flush_on_finish_file(false);
                    let sub_writer = {
                        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                        writer.set_flush_on_finish_file(false);
                        let sub_writer = {
                            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                            writer.set_flush_on_finish_file(false);
                            let sub_writer = {
                                let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                                writer.set_flush_on_finish_file(false);
                                let sub_writer = {
                                    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                                    writer.set_flush_on_finish_file(false);
                                    let sub_writer = {
                                        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                                        writer.set_flush_on_finish_file(false);
                                        let options = FileOptions {
                                            compression_method: Stored,
                                            compression_level: None,
                                            last_modified_time: DateTime::from_date_and_time(
                                                1981, 1, 1, 0, 0, 21,
                                            )?,
                                            permissions: Some(16908288),
                                            large_file: false,
                                            encrypt_with: None,
                                            extended_options: ExtendedFileOptions {
                                                extra_data: vec![].into(),
                                                central_extra_data: vec![].into(),
                                            },
                                            alignment: 20555,
                                            ..Default::default()
                                        };
                                        writer.start_file_from_path("\0\u{7}\u{1}\0\0\0\0\0\0\0\0\u{1}\0\0PK\u{1}\u{2};\u{1a}\u{18}\u{1a}UT\t.........................\0u", options)?;
                                        writer
                                    };
                                    writer.merge_archive(sub_writer.finish_into_readable()?)?;
                                    let options = FileOptions {
                                        compression_method: CompressionMethod::Bzip2,
                                        compression_level: Some(5),
                                        last_modified_time: DateTime::from_date_and_time(
                                            2055, 7, 7, 3, 6, 6,
                                        )?,
                                        permissions: None,
                                        large_file: false,
                                        encrypt_with: None,
                                        extended_options: ExtendedFileOptions {
                                            extra_data: vec![].into(),
                                            central_extra_data: vec![].into(),
                                        },
                                        alignment: 0,
                                        ..Default::default()
                                    };
                                    writer.start_file_from_path("\0\0\0\0..\0\0\0\0\0\u{7f}\u{7f}PK\u{6}\u{6}K\u{6}\u{6}\u{6}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\u{1}\0\0PK\u{1}\u{1e},\0\0\0\0\0\0\0\0\0\0\0\u{8}\0*\0\0\u{1}PK\u{6}\u{7}PK\u{6}\u{6}\0\0\0\0\0\0\0\0}K\u{2}\u{6}", options)?;
                                    writer = ZipWriter::new_append(
                                        writer.finish_into_readable()?.into_inner(),
                                    )?;
                                    writer
                                };
                                writer.merge_archive(sub_writer.finish_into_readable()?)?;
                                writer = ZipWriter::new_append(
                                    writer.finish_into_readable()?.into_inner(),
                                )?;
                                writer
                            };
                            writer.merge_archive(sub_writer.finish_into_readable()?)?;
                            writer =
                                ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
                            writer
                        };
                        writer.merge_archive(sub_writer.finish_into_readable()?)?;
                        writer =
                            ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
                        writer
                    };
                    writer.merge_archive(sub_writer.finish_into_readable()?)?;
                    writer
                };
                writer.merge_archive(sub_writer.finish_into_readable()?)?;
                writer
            };
            writer.merge_archive(sub_writer.finish_into_readable()?)?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_18() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_raw_comment(Box::<[u8]>::from([
            80, 75, 5, 6, 255, 255, 255, 255, 255, 255, 80, 75, 5, 6, 255, 255, 255, 255, 255, 255,
            13, 0, 13, 13, 13, 13, 13, 255, 255, 255, 255, 255, 255, 255, 255,
        ]));
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            writer.set_raw_comment(Box::new([]));
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        writer = ZipWriter::new_append(writer.finish()?)?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_18a() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        writer.set_raw_comment(Box::<[u8]>::from([]));
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let sub_writer = {
                let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                writer.set_flush_on_finish_file(false);
                let sub_writer = {
                    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                    writer.set_flush_on_finish_file(false);
                    let options = FullFileOptions {
                        compression_method: Stored,
                        compression_level: None,
                        last_modified_time: DateTime::from_date_and_time(2107, 4, 8, 14, 0, 19)?,
                        permissions: None,
                        large_file: false,
                        encrypt_with: None,
                        extended_options: ExtendedFileOptions {
                            extra_data: vec![
                                182, 180, 1, 0, 180, 182, 74, 0, 0, 200, 0, 0, 0, 2, 0, 0, 0,
                            ]
                            .into(),
                            central_extra_data: vec![].into(),
                        },
                        alignment: 1542,
                        ..Default::default()
                    };
                    writer.start_file_from_path("\0\0PK\u{6}\u{6}K\u{6}PK\u{3}\u{4}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\u{1}\u{1}\0PK\u{1}\u{2},\0\0\0\0\0\0\0\0\0\0\0P\u{7}\u{4}/.\0KP\0\0;\0\0\0\u{1e}\0\0\0\0\0\0\0\0\0\0\0\0\0", options)?;
                    let finished = writer.finish_into_readable()?;
                    assert_eq!(1, finished.file_names().count());
                    writer = ZipWriter::new_append(finished.into_inner())?;
                    let options = FullFileOptions {
                        compression_method: Stored,
                        compression_level: Some(5),
                        last_modified_time: DateTime::from_date_and_time(2107, 4, 1, 0, 0, 0)?,
                        permissions: None,
                        large_file: false,
                        encrypt_with: Some(ZipCrypto(
                            ZipCryptoKeys::of(0x0, 0x62e4b50, 0x100),
                            PhantomData,
                        )),
                        ..Default::default()
                    };
                    writer.add_symlink_from_path(
                        "\0K\u{6}\0PK\u{6}\u{7}PK\u{6}\u{6}\0\0\0\0\0\0\0\0PK\u{2}\u{6}",
                        "\u{8}\0\0\0\0/\0",
                        options,
                    )?;
                    let finished = writer.finish_into_readable()?;
                    assert_eq!(2, finished.file_names().count());
                    writer = ZipWriter::new_append(finished.into_inner())?;
                    assert_eq!(2, writer.files.len());
                    writer
                };
                let finished = sub_writer.finish_into_readable()?;
                assert_eq!(2, finished.file_names().count());
                writer.merge_archive(finished)?;
                writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
                writer
            };
            writer.merge_archive(sub_writer.finish_into_readable()?)?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[cfg(all(feature = "_bzip2_any", feature = "aes-crypto", not(miri)))]
    #[test]
    fn test_fuzz_crash_2024_06_18b() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(true);
        writer.set_raw_comment([0].into());
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        assert_eq!(writer.get_raw_comment()[0], 0);
        let options = FileOptions {
            compression_method: CompressionMethod::Bzip2,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2009, 6, 3, 13, 37, 39)?,
            permissions: Some(2644352413),
            large_file: true,
            encrypt_with: Some(crate::write::EncryptWith::Aes {
                mode: crate::AesMode::Aes256,
                password: "",
            }),
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 255,
            ..Default::default()
        };
        writer.add_symlink_from_path("", "", options)?;
        writer.deep_copy_file_from_path("", "PK\u{5}\u{6}\0\0\0\0\0\0\0\0\0\0\0\0\0\u{4}\0\0\0")?;
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        assert_eq!(writer.get_raw_comment()[0], 0);
        writer.deep_copy_file_from_path(
            "PK\u{5}\u{6}\0\0\0\0\0\0\0\0\0\0\0\0\0\u{4}\0\0\0",
            "\u{2}yy\u{5}qu\0",
        )?;
        let finished = writer.finish()?;
        let archive = ZipArchive::new(finished.clone())?;
        assert_eq!(archive.comment(), [0]);
        writer = ZipWriter::new_append(finished)?;
        assert_eq!(writer.get_raw_comment()[0], 0);
        let _ = writer.finish_into_readable()?;
        Ok(())
    }

    #[test]
    fn test_fuzz_crash_2024_06_19() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(1980, 3, 1, 19, 55, 58)?,
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 256,
            ..Default::default()
        };
        writer.start_file_from_path(
            "\0\0\0PK\u{5}\u{6}\0\0\0\0\u{1}\0\u{12}\u{6}\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\0",
            options,
        )?;
        writer.set_flush_on_finish_file(false);
        writer.shallow_copy_file_from_path(
            "\0\0\0PK\u{5}\u{6}\0\0\0\0\u{1}\0\u{12}\u{6}\0\0\0\0\0\u{1}\0\0\0\0\0\0\0\0\0",
            "",
        )?;
        writer.set_flush_on_finish_file(false);
        writer.deep_copy_file_from_path("", "copy")?;
        writer.abort_file()?;
        writer.set_flush_on_finish_file(false);
        writer.set_raw_comment([255, 0].into());
        writer.abort_file()?;
        assert_eq!(writer.get_raw_comment(), [255, 0]);
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        assert_eq!(writer.get_raw_comment(), [255, 0]);
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::default(),
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            ..Default::default()
        };
        writer.start_file_from_path("", options)?;
        assert_eq!(writer.get_raw_comment(), [255, 0]);
        let archive = writer.finish_into_readable()?;
        assert_eq!(archive.comment(), [255, 0]);
        Ok(())
    }

    #[test]
    fn fuzz_crash_2024_06_21() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FullFileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(1980, 2, 1, 0, 0, 0)?,
            permissions: None,
            large_file: false,
            encrypt_with: None,
            ..Default::default()
        };
        const LONG_PATH: &str = "\0@PK\u{6}\u{6}\u{7}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0@/\0\0\00ΝPK\u{5}\u{6}O\0\u{10}\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0@PK\u{6}\u{7}\u{6}\0/@\0\0\0\0\0\0\0\0 \0\0";
        writer.start_file_from_path(LONG_PATH, options)?;
        writer = ZipWriter::new_append(writer.finish()?)?;
        writer.deep_copy_file_from_path(LONG_PATH, "oo\0\0\0")?;
        writer.abort_file()?;
        writer.set_raw_comment([33].into());
        let archive = writer.finish_into_readable()?;
        writer = ZipWriter::new_append(archive.into_inner())?;
        assert!(writer.get_raw_comment().starts_with(&[33]));
        let archive = writer.finish_into_readable()?;
        assert!(archive.comment().starts_with(&[33]));
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "_bzip2_any", not(miri)))]
    fn fuzz_crash_2024_07_17() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: CompressionMethod::Bzip2,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2095, 2, 16, 21, 0, 1)?,
            permissions: Some(84238341),
            large_file: true,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![117, 99, 6, 0, 0, 0, 0, 0, 0, 0, 2, 1, 0, 0, 2, 0, 0, 0].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 65535,
            ..Default::default()
        };
        writer.start_file_from_path("", options)?;
        //writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        writer.deep_copy_file_from_path("", "copy")?;
        let _ = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        Ok(())
    }

    #[test]
    fn fuzz_crash_2024_07_19() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(1980, 6, 1, 0, 34, 47)?,
            permissions: None,
            large_file: true,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 45232,
            ..Default::default()
        };
        writer.add_directory_from_path("", options)?;
        writer.deep_copy_file_from_path("/", "")?;
        writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        writer.deep_copy_file_from_path("", "copy")?;
        let _ = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        Ok(())
    }

    #[test]
    #[cfg(feature = "aes-crypto")]
    fn fuzz_crash_2024_07_19a() -> ZipResult<()> {
        use crate::write::EncryptWith::Aes;
        use crate::AesMode::Aes128;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(false);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2107, 6, 5, 13, 0, 21)?,
            permissions: None,
            large_file: true,
            encrypt_with: Some(Aes {
                mode: Aes128,
                password: "",
            }),
            extended_options: ExtendedFileOptions {
                extra_data: vec![3, 0, 4, 0, 209, 53, 53, 8, 2, 61, 0, 0].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 65535,
            ..Default::default()
        };
        writer.start_file_from_path("", options)?;
        let _ = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        Ok(())
    }

    #[test]
    fn fuzz_crash_2024_07_20() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_flush_on_finish_file(true);
        let options = FileOptions {
            compression_method: Stored,
            compression_level: None,
            last_modified_time: DateTime::from_date_and_time(2041, 8, 2, 19, 38, 0)?,
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_data: vec![].into(),
                central_extra_data: vec![].into(),
            },
            alignment: 0,
            ..Default::default()
        };
        writer.add_directory_from_path("\0\0\0\0\0\0\07黻", options)?;
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.set_flush_on_finish_file(false);
            let options = FileOptions {
                compression_method: Stored,
                compression_level: None,
                last_modified_time: DateTime::default(),
                permissions: None,
                large_file: false,
                encrypt_with: None,
                extended_options: ExtendedFileOptions {
                    extra_data: vec![].into(),
                    central_extra_data: vec![].into(),
                },
                alignment: 4,
                ..Default::default()
            };
            writer.add_directory_from_path("\0\0\0黻", options)?;
            writer = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
            writer.abort_file()?;
            let options = FileOptions {
                compression_method: Stored,
                compression_level: None,
                last_modified_time: DateTime::from_date_and_time(1980, 1, 1, 0, 7, 0)?,
                permissions: Some(2663103419),
                large_file: false,
                encrypt_with: None,
                extended_options: ExtendedFileOptions {
                    extra_data: vec![].into(),
                    central_extra_data: vec![].into(),
                },
                alignment: 32256,
                ..Default::default()
            };
            writer.add_directory_from_path("\0", options)?;
            writer = ZipWriter::new_append(writer.finish()?)?;
            writer
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        let _ = ZipWriter::new_append(writer.finish_into_readable()?.into_inner())?;
        Ok(())
    }

    #[test]
    fn fuzz_crash_2024_07_21() -> ZipResult<()> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let sub_writer = {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer.add_directory_from_path(
                "",
                FileOptions {
                    compression_method: Stored,
                    compression_level: None,
                    last_modified_time: DateTime::from_date_and_time(2105, 8, 1, 15, 0, 0)?,
                    permissions: None,
                    large_file: false,
                    encrypt_with: None,
                    extended_options: ExtendedFileOptions {
                        extra_data: vec![].into(),
                        central_extra_data: vec![].into(),
                    },
                    alignment: 0,
                    ..Default::default()
                },
            )?;
            writer.abort_file()?;
            let mut writer = ZipWriter::new_append(writer.finish()?)?;
            writer.add_directory_from_path(
                "",
                FileOptions {
                    compression_method: Stored,
                    compression_level: None,
                    last_modified_time: DateTime::default(),
                    permissions: None,
                    large_file: false,
                    encrypt_with: None,
                    extended_options: ExtendedFileOptions {
                        extra_data: vec![].into(),
                        central_extra_data: vec![].into(),
                    },
                    alignment: 16,
                    ..Default::default()
                },
            )?;
            ZipWriter::new_append(writer.finish()?)?
        };
        writer.merge_archive(sub_writer.finish_into_readable()?)?;
        let writer = ZipWriter::new_append(writer.finish()?)?;
        let _ = writer.finish_into_readable()?;

        Ok(())
    }
}
