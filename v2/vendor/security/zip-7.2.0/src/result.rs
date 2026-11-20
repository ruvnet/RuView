#![allow(unknown_lints)] // non_local_definitions isn't in Rust 1.70
#![allow(non_local_definitions)]
//! Error types that can be emitted from this library

use core::error::Error;
use core::fmt::{self, Display, Formatter};
use core::num::TryFromIntError;
use std::borrow::Cow;
use std::io;

/// Generic result type with ZipError as its error variant
pub type ZipResult<T> = Result<T, ZipError>;

/// Error type for Zip
#[derive(Debug)]
#[non_exhaustive]
pub enum ZipError {
    /// i/o error
    Io(io::Error),

    /// invalid Zip archive
    InvalidArchive(Cow<'static, str>),

    /// unsupported Zip archive
    UnsupportedArchive(&'static str),

    /// specified file not found in archive
    FileNotFound,

    /// provided password is incorrect
    InvalidPassword,
}

impl ZipError {
    /// The text used as an error when a password is required and not supplied
    ///
    /// ```rust,no_run
    /// # use zip::result::ZipError;
    /// # let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&[])).unwrap();
    /// match archive.by_index(1) {
    ///     Err(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED)) => eprintln!("a password is needed to unzip this file"),
    ///     _ => (),
    /// }
    /// # ()
    /// ```
    pub const PASSWORD_REQUIRED: &'static str = "Password required to decrypt file";
}

impl Display for ZipError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::InvalidArchive(e) => write!(f, "invalid Zip archive: {e}"),
            Self::UnsupportedArchive(e) => write!(f, "unsupported Zip archive: {e}"),
            Self::FileNotFound => f.write_str("specified file not found in archive"),
            Self::InvalidPassword => f.write_str("provided password is incorrect"),
        }
    }
}

impl Error for ZipError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::InvalidArchive(_)
            | Self::UnsupportedArchive(_)
            | Self::FileNotFound
            | Self::InvalidPassword => None,
        }
    }
}

impl From<ZipError> for io::Error {
    fn from(err: ZipError) -> io::Error {
        let kind = match &err {
            ZipError::Io(err) => err.kind(),
            ZipError::InvalidArchive(_) => io::ErrorKind::InvalidData,
            ZipError::UnsupportedArchive(_) => io::ErrorKind::Unsupported,
            ZipError::FileNotFound => io::ErrorKind::NotFound,
            ZipError::InvalidPassword => io::ErrorKind::InvalidInput,
        };

        io::Error::new(kind, err)
    }
}

impl From<io::Error> for ZipError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<DateTimeRangeError> for ZipError {
    fn from(_: DateTimeRangeError) -> Self {
        invalid!("Invalid date or time")
    }
}

impl From<std::string::FromUtf8Error> for ZipError {
    fn from(_: std::string::FromUtf8Error) -> Self {
        invalid!("Invalid UTF-8")
    }
}

/// Error type for time parsing
#[derive(Debug)]
pub struct DateTimeRangeError;

// TryFromIntError is also an out-of-range error.
impl From<TryFromIntError> for DateTimeRangeError {
    fn from(_value: TryFromIntError) -> Self {
        DateTimeRangeError
    }
}

impl fmt::Display for DateTimeRangeError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "a date could not be represented within the bounds the MS-DOS date range (1980-2107)"
        )
    }
}

impl Error for DateTimeRangeError {}

pub(crate) fn invalid_archive<M: Into<Cow<'static, str>>>(message: M) -> ZipError {
    ZipError::InvalidArchive(message.into())
}

pub(crate) const fn invalid_archive_const(message: &'static str) -> ZipError {
    ZipError::InvalidArchive(Cow::Borrowed(message))
}

macro_rules! invalid {
    ($message:literal) => {
        crate::result::invalid_archive_const($message)
    };
    ($($arg:tt)*) => {
        crate::result::invalid_archive(format!($($arg)*))
    };
}
pub(crate) use invalid;
