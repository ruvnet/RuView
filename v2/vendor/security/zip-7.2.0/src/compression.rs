//! Possible ZIP compression methods.

use crate::cfg_if_expr;
use core::fmt;
use std::io;

#[allow(deprecated)]
/// Identifies the storage format used to compress a file within a ZIP archive.
///
/// Each file's compression method is stored alongside it, allowing the
/// contents to be read without context.
///
/// When creating ZIP files, you may choose the method to use with
/// [`crate::write::FileOptions::compression_method`]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "_arbitrary", derive(arbitrary::Arbitrary))]
#[non_exhaustive]
pub enum CompressionMethod {
    /// Store the file as is
    Stored,
    /// Compress the file using Deflate
    #[cfg(feature = "_deflate-any")]
    Deflated,
    /// Compress the file using Deflate64.
    /// Decoding deflate64 is supported but encoding deflate64 is not supported.
    #[cfg(feature = "deflate64")]
    Deflate64,
    /// Compress the file using BZIP2
    #[cfg(feature = "_bzip2_any")]
    Bzip2,
    /// Encrypted using AES.
    ///
    /// The actual compression method has to be taken from the AES extra data field
    /// or from `ZipFileData`.
    #[cfg(feature = "aes-crypto")]
    Aes,
    /// Compress the file using ZStandard
    #[cfg(feature = "zstd")]
    Zstd,
    /// Compress the file using LZMA
    #[cfg(feature = "lzma")]
    Lzma,
    #[cfg(feature = "legacy-zip")]
    /// Method 1 Shrink
    Shrink,
    #[cfg(feature = "legacy-zip")]
    /// Reduce (Method 2-5)
    Reduce(u8),
    #[cfg(feature = "legacy-zip")]
    /// Method 6 Implode/explode
    Implode,
    /// Compress the file using XZ
    #[cfg(feature = "xz")]
    Xz,
    /// Compress the file using PPMd
    #[cfg(feature = "ppmd")]
    Ppmd,
    /// Unsupported compression method
    #[cfg_attr(
        not(any(fuzzing, feature = "_arbitrary")),
        deprecated(since = "0.5.7", note = "use the constants instead")
    )]
    Unsupported(u16),
}
#[allow(deprecated, missing_docs)]
/// All compression methods defined for the ZIP format
impl CompressionMethod {
    pub const STORE: Self = CompressionMethod::Stored;
    #[cfg(feature = "legacy-zip")]
    pub const SHRINK: Self = CompressionMethod::Shrink;
    #[cfg(not(feature = "legacy-zip"))]
    /// Legacy compression method (enable feature `legacy-zip` to get support)
    pub const SHRINK: Self = CompressionMethod::Unsupported(1);
    #[cfg(feature = "legacy-zip")]
    /// Legacy compression method
    pub const REDUCE_1: Self = CompressionMethod::Reduce(1);
    #[cfg(not(feature = "legacy-zip"))]
    /// Legacy compression method (enable feature `legacy-zip` to get support)
    pub const REDUCE_1: Self = CompressionMethod::Unsupported(2);
    #[cfg(feature = "legacy-zip")]
    /// Legacy compression method
    pub const REDUCE_2: Self = CompressionMethod::Reduce(2);
    #[cfg(not(feature = "legacy-zip"))]
    /// Legacy compression method (enable feature `legacy-zip` to get support)
    pub const REDUCE_2: Self = CompressionMethod::Unsupported(3);
    #[cfg(feature = "legacy-zip")]
    /// Legacy compression method
    pub const REDUCE_3: Self = CompressionMethod::Reduce(3);
    #[cfg(not(feature = "legacy-zip"))]
    /// Legacy compression method (enable feature `legacy-zip` to get support)
    pub const REDUCE_3: Self = CompressionMethod::Unsupported(4);
    #[cfg(feature = "legacy-zip")]
    /// Legacy compression method
    pub const REDUCE_4: Self = CompressionMethod::Reduce(4);
    #[cfg(not(feature = "legacy-zip"))]
    /// Legacy compression method (enable feature `legacy-zip` to get support)
    pub const REDUCE_4: Self = CompressionMethod::Unsupported(5);
    #[cfg(feature = "legacy-zip")]
    /// Legacy compression method
    pub const IMPLODE: Self = CompressionMethod::Implode;
    #[cfg(not(feature = "legacy-zip"))]
    /// Legacy compression method (enable feature `legacy-zip` to get support)
    pub const IMPLODE: Self = CompressionMethod::Unsupported(6);
    #[cfg(feature = "_deflate-any")]
    pub const DEFLATE: Self = CompressionMethod::Deflated;
    #[cfg(not(feature = "_deflate-any"))]
    pub const DEFLATE: Self = CompressionMethod::Unsupported(8);
    #[cfg(feature = "deflate64")]
    pub const DEFLATE64: Self = CompressionMethod::Deflate64;
    #[cfg(not(feature = "deflate64"))]
    pub const DEFLATE64: Self = CompressionMethod::Unsupported(9);
    pub const PKWARE_IMPLODE: Self = CompressionMethod::Unsupported(10);
    #[cfg(feature = "_bzip2_any")]
    pub const BZIP2: Self = CompressionMethod::Bzip2;
    #[cfg(not(feature = "_bzip2_any"))]
    pub const BZIP2: Self = CompressionMethod::Unsupported(12);
    #[cfg(not(feature = "lzma"))]
    pub const LZMA: Self = CompressionMethod::Unsupported(14);
    #[cfg(feature = "lzma")]
    pub const LZMA: Self = CompressionMethod::Lzma;
    pub const IBM_ZOS_CMPSC: Self = CompressionMethod::Unsupported(16);
    pub const IBM_TERSE: Self = CompressionMethod::Unsupported(18);
    pub const ZSTD_DEPRECATED: Self = CompressionMethod::Unsupported(20);
    #[cfg(feature = "zstd")]
    pub const ZSTD: Self = CompressionMethod::Zstd;
    #[cfg(not(feature = "zstd"))]
    pub const ZSTD: Self = CompressionMethod::Unsupported(93);
    pub const MP3: Self = CompressionMethod::Unsupported(94);
    #[cfg(feature = "xz")]
    pub const XZ: Self = CompressionMethod::Xz;
    #[cfg(not(feature = "xz"))]
    pub const XZ: Self = CompressionMethod::Unsupported(95);
    pub const JPEG: Self = CompressionMethod::Unsupported(96);
    pub const WAVPACK: Self = CompressionMethod::Unsupported(97);
    #[cfg(feature = "ppmd")]
    pub const PPMD: Self = CompressionMethod::Ppmd;
    #[cfg(not(feature = "ppmd"))]
    pub const PPMD: Self = CompressionMethod::Unsupported(98);
    #[cfg(feature = "aes-crypto")]
    pub const AES: Self = CompressionMethod::Aes;
    #[cfg(not(feature = "aes-crypto"))]
    pub const AES: Self = CompressionMethod::Unsupported(99);

    #[cfg(feature = "_deflate-any")]
    pub const DEFAULT: Self = CompressionMethod::Deflated;

    #[cfg(all(not(feature = "_deflate-any"), feature = "_bzip2_any"))]
    pub const DEFAULT: Self = CompressionMethod::Bzip2;

    #[cfg(all(not(feature = "_deflate-any"), not(feature = "_bzip2_any")))]
    pub const DEFAULT: Self = CompressionMethod::Stored;
}
impl CompressionMethod {
    pub(crate) const fn parse_from_u16(val: u16) -> Self {
        match val {
            0 => CompressionMethod::Stored,
            #[cfg(feature = "legacy-zip")]
            1 => CompressionMethod::Shrink,
            #[cfg(feature = "legacy-zip")]
            2 => CompressionMethod::Reduce(1),
            #[cfg(feature = "legacy-zip")]
            3 => CompressionMethod::Reduce(2),
            #[cfg(feature = "legacy-zip")]
            4 => CompressionMethod::Reduce(3),
            #[cfg(feature = "legacy-zip")]
            5 => CompressionMethod::Reduce(4),
            #[cfg(feature = "legacy-zip")]
            6 => CompressionMethod::Implode,
            #[cfg(feature = "_deflate-any")]
            8 => CompressionMethod::Deflated,
            #[cfg(feature = "deflate64")]
            9 => CompressionMethod::Deflate64,
            #[cfg(feature = "_bzip2_any")]
            12 => CompressionMethod::Bzip2,
            #[cfg(feature = "lzma")]
            14 => CompressionMethod::Lzma,
            #[cfg(feature = "xz")]
            95 => CompressionMethod::Xz,
            #[cfg(feature = "zstd")]
            93 => CompressionMethod::Zstd,
            #[cfg(feature = "ppmd")]
            98 => CompressionMethod::Ppmd,
            #[cfg(feature = "aes-crypto")]
            99 => CompressionMethod::Aes,
            #[allow(deprecated)]
            v => CompressionMethod::Unsupported(v),
        }
    }

    /// Converts a u16 to its corresponding CompressionMethod
    #[deprecated(
        since = "0.5.7",
        note = "use a constant to construct a compression method"
    )]
    pub const fn from_u16(val: u16) -> CompressionMethod {
        Self::parse_from_u16(val)
    }

    pub(crate) const fn serialize_to_u16(self) -> u16 {
        match self {
            CompressionMethod::Stored => 0,
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Shrink => 1,
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Reduce(n) => 1 + n as u16,
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Implode => 6,

            #[cfg(feature = "_deflate-any")]
            CompressionMethod::Deflated => 8,
            #[cfg(feature = "deflate64")]
            CompressionMethod::Deflate64 => 9,
            #[cfg(feature = "_bzip2_any")]
            CompressionMethod::Bzip2 => 12,
            #[cfg(feature = "aes-crypto")]
            CompressionMethod::Aes => 99,
            #[cfg(feature = "zstd")]
            CompressionMethod::Zstd => 93,
            #[cfg(feature = "lzma")]
            CompressionMethod::Lzma => 14,
            #[cfg(feature = "xz")]
            CompressionMethod::Xz => 95,
            #[cfg(feature = "ppmd")]
            CompressionMethod::Ppmd => 98,
            #[allow(deprecated)]
            CompressionMethod::Unsupported(v) => v,
        }
    }

    /// Converts a CompressionMethod to a u16
    #[deprecated(
        since = "0.5.7",
        note = "to match on other compression methods, use a constant"
    )]
    pub const fn to_u16(self) -> u16 {
        self.serialize_to_u16()
    }
}

impl Default for CompressionMethod {
    fn default() -> Self {
        cfg_if_expr! {
            #[cfg(feature = "_deflate-any")] => CompressionMethod::Deflated,
            _ => CompressionMethod::Stored
        }
    }
}

impl fmt::Display for CompressionMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Just duplicate what the Debug format looks like, i.e, the enum key:
        write!(f, "{self:?}")
    }
}

/// The compression methods which have been implemented.
pub const SUPPORTED_COMPRESSION_METHODS: &[CompressionMethod] = &[
    CompressionMethod::Stored,
    #[cfg(feature = "_deflate-any")]
    CompressionMethod::Deflated,
    #[cfg(feature = "deflate64")]
    CompressionMethod::Deflate64,
    #[cfg(feature = "_bzip2_any")]
    CompressionMethod::Bzip2,
    #[cfg(feature = "zstd")]
    CompressionMethod::Zstd,
    #[cfg(feature = "xz")]
    CompressionMethod::Xz,
    #[cfg(feature = "ppmd")]
    CompressionMethod::Ppmd,
];

pub(crate) enum Decompressor<R: io::BufRead> {
    Stored(R),
    #[cfg(feature = "deflate-flate2")]
    Deflated(flate2::bufread::DeflateDecoder<R>),
    #[cfg(feature = "deflate64")]
    Deflate64(deflate64::Deflate64Decoder<R>),
    #[cfg(feature = "_bzip2_any")]
    Bzip2(bzip2::bufread::BzDecoder<R>),
    #[cfg(feature = "zstd")]
    Zstd(zstd::Decoder<'static, R>),
    #[cfg(feature = "lzma")]
    Lzma(Lzma<R>),
    #[cfg(feature = "legacy-zip")]
    Shrink(crate::legacy::shrink::ShrinkDecoder<R>),
    #[cfg(feature = "legacy-zip")]
    Reduce(crate::legacy::reduce::ReduceDecoder<R>),
    #[cfg(feature = "legacy-zip")]
    Implode(crate::legacy::implode::ImplodeDecoder<R>),
    #[cfg(feature = "xz")]
    Xz(Box<lzma_rust2::XzReader<R>>),
    #[cfg(feature = "ppmd")]
    Ppmd(Ppmd<R>),
}

#[cfg(feature = "lzma")]
pub(crate) enum Lzma<R: io::BufRead> {
    Uninitialized {
        reader: Option<R>,
        uncompressed_size: u64,
    },
    Initialized(Box<lzma_rust2::LzmaReader<R>>),
}

#[cfg(feature = "ppmd")]
pub(crate) enum Ppmd<R: io::BufRead> {
    Uninitialized(Option<R>),
    Initialized(Box<ppmd_rust::Ppmd8Decoder<R>>),
}

impl<R: io::BufRead> io::Read for Decompressor<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Decompressor::Stored(r) => r.read(buf),
            #[cfg(feature = "deflate-flate2")]
            Decompressor::Deflated(r) => r.read(buf),
            #[cfg(feature = "deflate64")]
            Decompressor::Deflate64(r) => r.read(buf),
            #[cfg(feature = "_bzip2_any")]
            Decompressor::Bzip2(r) => r.read(buf),
            #[cfg(feature = "zstd")]
            Decompressor::Zstd(r) => r.read(buf),
            #[cfg(feature = "lzma")]
            Decompressor::Lzma(r) => match r {
                Lzma::Uninitialized {
                    reader,
                    uncompressed_size,
                } => {
                    let mut reader = reader.take().ok_or_else(|| {
                        io::Error::other("Reader was not set while reading LZMA data")
                    })?;

                    // 5.8.8.1 LZMA Version Information & 5.8.8.2 LZMA Properties Size
                    let mut header = [0; 4];
                    reader.read_exact(&mut header)?;
                    let _version_information = u16::from_le_bytes(header[0..2].try_into().unwrap());
                    let properties_size = u16::from_le_bytes(header[2..4].try_into().unwrap());
                    if properties_size != 5 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("unexpected LZMA properties size of {properties_size}"),
                        ));
                    }

                    let mut props_data = [0; 5];
                    reader.read_exact(&mut props_data)?;
                    let props = props_data[0];
                    let dict_size = u32::from_le_bytes(props_data[1..5].try_into().unwrap());

                    // We don't need to handle the end-of-stream marker here, since the LZMA reader
                    // stops at the end-of-stream marker OR when it has decoded uncompressed_size bytes, whichever comes first.
                    let mut decompressor = lzma_rust2::LzmaReader::new_with_props(
                        reader,
                        *uncompressed_size,
                        props,
                        dict_size,
                        None,
                    )?;

                    let read = decompressor.read(buf)?;

                    *r = Lzma::Initialized(Box::new(decompressor));

                    Ok(read)
                }
                Lzma::Initialized(decompressor) => decompressor.read(buf),
            },
            #[cfg(feature = "xz")]
            Decompressor::Xz(r) => r.read(buf),
            #[cfg(feature = "ppmd")]
            Decompressor::Ppmd(r) => match r {
                Ppmd::Uninitialized(reader) => {
                    let mut reader = reader.take().ok_or_else(|| {
                        io::Error::other("Reader was not set while reading PPMd data")
                    })?;

                    let mut buffer = [0; 2];
                    reader.read_exact(&mut buffer)?;
                    let parameters = u16::from_le_bytes(buffer);

                    let order = ((parameters & 0x0F) + 1) as u32;
                    let memory_size = 1024 * 1024 * (((parameters >> 4) & 0xFF) + 1) as u32;
                    let restoration_method = (parameters >> 12) & 0x0F;

                    let mut decompressor = ppmd_rust::Ppmd8Decoder::new(
                        reader,
                        order,
                        memory_size,
                        restoration_method.into(),
                    )
                    .map_err(|error| match error {
                        ppmd_rust::Error::RangeDecoderInitialization => io::Error::new(
                            io::ErrorKind::InvalidData,
                            "PPMd range coder initialization failed",
                        ),
                        ppmd_rust::Error::InvalidParameter => {
                            io::Error::new(io::ErrorKind::InvalidInput, "Invalid PPMd parameter")
                        }
                        ppmd_rust::Error::IoError(io_error) => io_error,
                        ppmd_rust::Error::MemoryAllocation => {
                            io::Error::new(io::ErrorKind::OutOfMemory, "Memory allocation failed")
                        }
                    })?;

                    let read = decompressor.read(buf)?;

                    *r = Ppmd::Initialized(Box::new(decompressor));

                    Ok(read)
                }
                Ppmd::Initialized(decompressor) => decompressor.read(buf),
            },
            #[cfg(feature = "legacy-zip")]
            Decompressor::Shrink(r) => r.read(buf),
            #[cfg(feature = "legacy-zip")]
            Decompressor::Reduce(r) => r.read(buf),
            #[cfg(feature = "legacy-zip")]
            Decompressor::Implode(r) => r.read(buf),
        }
    }
}

impl<R: io::BufRead> Decompressor<R> {
    pub fn new(
        reader: R,
        compression_method: CompressionMethod,
        #[cfg(any(feature = "lzma", feature = "legacy-zip"))] uncompressed_size: u64,
        #[cfg(not(any(feature = "lzma", feature = "legacy-zip")))] _uncompressed_size: u64,
        #[cfg(feature = "legacy-zip")] flags: u16,
        #[cfg(not(feature = "legacy-zip"))] _flags: u16,
    ) -> crate::result::ZipResult<Self> {
        Ok(match compression_method {
            CompressionMethod::Stored => Decompressor::Stored(reader),
            #[cfg(feature = "deflate-flate2")]
            CompressionMethod::Deflated => {
                Decompressor::Deflated(flate2::bufread::DeflateDecoder::new(reader))
            }
            #[cfg(feature = "deflate64")]
            CompressionMethod::Deflate64 => {
                Decompressor::Deflate64(deflate64::Deflate64Decoder::with_buffer(reader))
            }
            #[cfg(feature = "_bzip2_any")]
            CompressionMethod::Bzip2 => Decompressor::Bzip2(bzip2::bufread::BzDecoder::new(reader)),
            #[cfg(feature = "zstd")]
            CompressionMethod::Zstd => Decompressor::Zstd(zstd::Decoder::with_buffer(reader)?),
            #[cfg(feature = "lzma")]
            CompressionMethod::Lzma => Decompressor::Lzma(Lzma::Uninitialized {
                reader: Some(reader),
                uncompressed_size,
            }),
            #[cfg(feature = "xz")]
            CompressionMethod::Xz => {
                Decompressor::Xz(Box::new(lzma_rust2::XzReader::new(reader, false)))
            }
            #[cfg(feature = "ppmd")]
            CompressionMethod::Ppmd => Decompressor::Ppmd(Ppmd::Uninitialized(Some(reader))),
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Shrink => Decompressor::Shrink(
                crate::legacy::shrink::ShrinkDecoder::new(reader, uncompressed_size),
            ),
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Reduce(n) => Decompressor::Reduce(
                crate::legacy::reduce::ReduceDecoder::new(reader, uncompressed_size, n),
            ),
            #[cfg(feature = "legacy-zip")]
            CompressionMethod::Implode => Decompressor::Implode(
                crate::legacy::implode::ImplodeDecoder::new(reader, uncompressed_size, flags),
            ),
            _ => {
                return Err(crate::result::ZipError::UnsupportedArchive(
                    "Compression method not supported",
                ))
            }
        })
    }

    /// Consumes this decoder, returning the underlying reader.
    #[allow(clippy::infallible_destructuring_match)]
    pub fn into_inner(self) -> io::Result<R> {
        let inner = match self {
            Decompressor::Stored(r) => r,
            #[cfg(feature = "deflate-flate2")]
            Decompressor::Deflated(r) => r.into_inner(),
            #[cfg(feature = "deflate64")]
            Decompressor::Deflate64(r) => r.into_inner(),
            #[cfg(feature = "_bzip2_any")]
            Decompressor::Bzip2(r) => r.into_inner(),
            #[cfg(feature = "zstd")]
            Decompressor::Zstd(r) => r.finish(),
            #[cfg(feature = "lzma")]
            Decompressor::Lzma(r) => match r {
                Lzma::Uninitialized { mut reader, .. } => reader
                    .take()
                    .ok_or_else(|| io::Error::other("Reader was not set"))?,
                Lzma::Initialized(decoder) => decoder.into_inner(),
            },
            #[cfg(feature = "legacy-zip")]
            Decompressor::Shrink(r) => r.into_inner(),
            #[cfg(feature = "legacy-zip")]
            Decompressor::Reduce(r) => r.into_inner(),
            #[cfg(feature = "legacy-zip")]
            Decompressor::Implode(r) => r.into_inner(),
            #[cfg(feature = "xz")]
            Decompressor::Xz(r) => r.into_inner(),
            #[cfg(feature = "ppmd")]
            Decompressor::Ppmd(r) => match r {
                Ppmd::Uninitialized(mut reader) => reader
                    .take()
                    .ok_or_else(|| io::Error::other("Reader was not set"))?,
                Ppmd::Initialized(decoder) => decoder.into_inner(),
            },
        };
        Ok(inner)
    }
}

#[cfg(test)]
mod test {
    use super::{CompressionMethod, SUPPORTED_COMPRESSION_METHODS};

    #[test]
    fn from_eq_to() {
        for v in 0..(u16::MAX as u32 + 1) {
            let from = CompressionMethod::parse_from_u16(v as u16);
            let to = from.serialize_to_u16() as u32;
            assert_eq!(v, to);
        }
    }

    #[test]
    fn to_eq_from() {
        fn check_match(method: CompressionMethod) {
            let to = method.serialize_to_u16();
            let from = CompressionMethod::parse_from_u16(to);
            let back = from.serialize_to_u16();
            assert_eq!(to, back);
        }

        for &method in SUPPORTED_COMPRESSION_METHODS {
            check_match(method);
        }
    }

    #[test]
    fn to_display_fmt() {
        fn check_match(method: CompressionMethod) {
            let debug_str = format!("{method:?}");
            let display_str = format!("{method}");
            assert_eq!(debug_str, display_str);
        }

        for &method in SUPPORTED_COMPRESSION_METHODS {
            check_match(method);
        }
    }
}
